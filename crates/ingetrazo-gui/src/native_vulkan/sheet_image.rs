//! Cached physical sheet CAD target; shares the host device and allocation ledger.
use super::{
    budget::VulkanBudget,
    cad2d, paper_picture,
    resources::{BufferId, Resources, TextureImage},
    VulkanContext,
};
use ash::{vk, Device};
use ingetrazo_render::cad2d::Pose;
use std::sync::Arc;
enum Handle { Cad(cad2d::Handle), Picture(Option<usize>) }
pub struct Exporter {
    device: Device,
    lifetime: Arc<super::lifetime::Lifetime>,
    resources: Resources,
    target: Option<TextureImage>,
    readback: Option<BufferId>,
    cad: Option<cad2d::Renderer>,
    pictures: Option<paper_picture::Renderer>,
    pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    fence: vk::Fence,
    extent: vk::Extent2D,
    serial: u64,
}
pub use super::paper_picture::Stats as PictureStats;
impl Exporter {
    pub fn picture_stats(&self) -> PictureStats { self.pictures.as_ref().map_or_else(PictureStats::default, |renderer| renderer.stats()) }
    pub fn new(context: &VulkanContext, budget: Arc<VulkanBudget>) -> Result<Self, String> {
        context.lifetime.check()?;
        let limit = unsafe {
            context
                .instance
                .get_physical_device_properties(context.physical_device)
        }
        .limits
        .max_image_dimension2_d;
        let extent = vk::Extent2D {
            width: limit.min(2048),
            height: limit.min(128),
        };
        if extent.width == 0 || extent.height < 8 {
            return Err("Native paper target limits are unusable".into());
        }
        let mut out = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            resources: Resources::new(context, budget.clone()),
            target: None,
            readback: None,
            cad: None,
            pictures: None,
            pass: vk::RenderPass::null(),
            framebuffer: vk::Framebuffer::null(),
            pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
            extent,
            serial: 0,
        };
        out.target = Some(TextureImage::paper_target(
            context,
            &mut out.resources,
            extent,
        )?);
        out.readback = Some(
            out.resources
                .paper_readback_buffer(u64::from(extent.width) * u64::from(extent.height) * 4)?,
        );
        out.pass = super::create_render_pass_with_final_layout(
            &context.device,
            vk::Format::R8G8B8A8_UNORM,
            true,
            None,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        )
        .map_err(|e| e.to_string())?;
        unsafe {
            out.framebuffer = context
                .device
                .create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(out.pass)
                        .attachments(&[out.target.as_ref().unwrap().view])
                        .width(extent.width)
                        .height(extent.height)
                        .layers(1),
                    None,
                )
                .map_err(|e| e.to_string())?;
            out.pool = context
                .device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(context.graphics_q_index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .map_err(|e| e.to_string())?;
            out.command = context
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(out.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .map_err(|e| e.to_string())?[0];
            out.fence = context
                .device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
                .map_err(|e| e.to_string())?;
        }
        out.cad = Some(cad2d::Renderer::new(
            context,
            budget,
            out.pass,
            &[out.target.as_ref().unwrap().view],
            true,
        )?);
        Ok(out)
    }
    fn tile(
        &mut self,
        context: &VulkanContext,
        records: &[crate::sheet_image::Record],
        handles: &[Handle],
        origin: [u32; 2],
    ) -> Result<Vec<u8>, String> {
        if !Arc::ptr_eq(&context.lifetime, &self.lifetime) {
            return Err("Paper exporter differs from native host owner".into());
        }
        self.lifetime.check()?;
        let pose = Pose {
            scale: [1.; 2],
            translation: [-(origin[0] as f32), -(origin[1] as f32)],
        };
        let rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.extent.width as f32, self.extent.height as f32),
        );
        let mut primitives = Vec::new();
        primitives
            .try_reserve_exact(handles.len())
            .map_err(|_| "Paper callback allocation failed")?;
        for (handle, record) in handles.iter().zip(records) {
            let (Handle::Cad(handle), crate::sheet_image::Record::Cad((geometry, batches))) = (handle, record) else { continue; };
            handle.update(geometry.clone(), batches.clone(), pose)?;
            let egui::Shape::Callback(callback) = handle.shape(rect) else {
                unreachable!()
            };
            primitives.push(egui::ClippedPrimitive {
                clip_rect: rect,
                primitive: egui::epaint::Primitive::Callback(callback),
            });
        }
        let next = self
            .serial
            .checked_add(1)
            .ok_or("Paper submission serial overflow")?;
        let cad = self.cad.as_mut().unwrap();
        cad.set_input_image(0)?;
        cad.prepare(context, &primitives, self.fence, self.serial, next, 1.)?;
        unsafe {
            self.device
                .reset_command_buffer(self.command, vk::CommandBufferResetFlags::empty())
                .map_err(|e| e.to_string())?;
            self.device
                .begin_command_buffer(
                    self.command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|e| e.to_string())?;
            let clear = [vk::ClearValue {
                color: vk::ClearColorValue { float32: [1.; 4] },
            }];
            self.device.cmd_begin_render_pass(
                self.command,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.pass)
                    .framebuffer(self.framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: self.extent,
                    })
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
        }
        let info = egui::PaintCallbackInfo {
            viewport: rect,
            clip_rect: rect,
            pixels_per_point: 1.,
            screen_size_px: [self.extent.width, self.extent.height],
        };
        for handle in handles {
            match handle {
                Handle::Cad(handle) => handle.record_paper(&info, &self.device, self.command)?,
                Handle::Picture(Some(index)) => self.pictures.as_ref().unwrap().record(*index, self.command, self.extent, origin),
                Handle::Picture(None) => {},
            }
        }
        unsafe {
            self.device.cmd_end_render_pass(self.command);
            let buffer = self
                .resources
                .buffer(self.readback.unwrap())
                .ok_or("Paper readback buffer absent")?;
            let image_barrier = vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ);
            self.device.cmd_pipeline_barrier(
                self.command,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[image_barrier],
                &[],
                &[],
            );
            let region = vk::BufferImageCopy::default()
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .layer_count(1),
                )
                .image_extent(vk::Extent3D {
                    width: self.extent.width,
                    height: self.extent.height,
                    depth: 1,
                });
            self.device.cmd_copy_image_to_buffer(
                self.command,
                self.target.as_ref().unwrap().image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buffer,
                &[region],
            );
            let host_barrier = vk::BufferMemoryBarrier::default()
                .buffer(buffer)
                .offset(0)
                .size(vk::WHOLE_SIZE)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ);
            self.device.cmd_pipeline_barrier(
                self.command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::HOST,
                vk::DependencyFlags::empty(),
                &[],
                &[host_barrier],
                &[],
            );

            self.device
                .end_command_buffer(self.command)
                .map_err(|e| e.to_string())?;
            if let Some(pictures) = &self.pictures { pictures.mark_used(&mut self.resources, next)?; }
            self.device
                .reset_fences(&[self.fence])
                .map_err(|e| e.to_string())?;
            if let Err(e) = self.device.queue_submit(
                context.graphics_queue,
                &[vk::SubmitInfo::default().command_buffers(&[self.command])],
                self.fence,
            ) {
                self.lifetime.quarantine();
                return Err(e.to_string());
            }
        }
        context
            .wait_transfer(self.fence)
            .map_err(|e| e.to_string())?;
        self.serial = next;
        let bytes = self.extent.width as usize * self.extent.height as usize * 4;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes)
            .map_err(|_| "Paper tile readback allocation failed")?;
        pixels.resize(bytes, 0);
        unsafe {
            self.resources
                .read_completed_paper_buffer(self.readback.unwrap(), &mut pixels)?;
        }
        Ok(pixels)
    }
    pub fn save(
        &mut self,
        context: &VulkanContext,
        plan: &crate::sheet_image::Plan,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let factory = self.cad.as_ref().unwrap().factory();
        let mut pictures = Vec::new();
        for record in &plan.records {
            if let crate::sheet_image::Record::Picture(callback) = record { pictures.push(callback.snapshot()?.sampling); }
        }
        if self.pictures.is_none() && !pictures.is_empty() {
            self.pictures = Some(paper_picture::Renderer::new(context, self.pass)?);
        }
        let picture_handles = if let Some(renderer) = &mut self.pictures {
            renderer.prepare(context, &mut self.resources, &pictures, self.target.as_ref().unwrap().view, self.fence, self.serial)?
        } else { Vec::new() };
        let mut picture_handles = picture_handles.into_iter();
        let mut handles = Vec::new();
        handles.try_reserve_exact(plan.records.len()).map_err(|_| "Paper handle inventory allocation failed")?;
        for record in &plan.records {
            handles.push(match record {
                crate::sheet_image::Record::Cad((geometry, batches)) => Handle::Cad(factory.handle(geometry.clone(), batches.clone(), Pose { scale: [1.; 2], translation: [0.; 2] })?),
                crate::sheet_image::Record::Picture(_) => Handle::Picture(picture_handles.next().ok_or("Paper picture handle inventory mismatch")?),
            });
        }
        let [width, height] = plan.metrics.pixels();
        let band_bytes = (width as usize)
            .checked_mul(self.extent.height as usize)
            .and_then(|n| n.checked_mul(4))
            .filter(|&n| n <= 32 * 1024 * 1024)
            .ok_or("Native paper row band exceeds 32 MiB")?;
        let mut band = Vec::new();
        band.try_reserve_exact(band_bytes)
            .map_err(|_| "Native paper band allocation failed")?;
        let mut first = None;
        let mut band_rows = 0;
        ingetrazo_formats::sheet_image::save_rows(path, plan.metrics, |y, rows| {
            if first.is_none_or(|start| y < start || y + rows > start + band_rows) {
                let start = y / self.extent.height * self.extent.height;
                band_rows = (height - start).min(self.extent.height);
                band.resize(width as usize * band_rows as usize * 4, 255);
                for x in (0..width).step_by(self.extent.width as usize) {
                    let pixels = self.tile(context, &plan.records, &handles, [x, start])?;
                    let columns = (width - x).min(self.extent.width) as usize;
                    for row in 0..band_rows as usize {
                        let src = row * self.extent.width as usize * 4;
                        let dst = (row * width as usize + x as usize) * 4;
                        band[dst..dst + columns * 4]
                            .copy_from_slice(&pixels[src..src + columns * 4]);
                    }
                }
                first = Some(start);
            }
            let offset = (y - first.unwrap()) as usize * width as usize * 4;
            let length = rows as usize * width as usize * 4;
            Ok(band[offset..offset + length].to_vec())
        })
    }
}
impl Drop for Exporter {
    fn drop(&mut self) {
        self.cad.take();
        self.pictures.take();
        if self.lifetime.uncertain() {
            self.lifetime.retain((
                self.pass,
                self.framebuffer,
                self.pool,
                self.command,
                self.fence,
            ));
            return;
        }
        unsafe {
            if self.fence != vk::Fence::null() {
                self.device.destroy_fence(self.fence, None);
            }
            if self.pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.pool, None);
            }
            if self.framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.framebuffer, None);
            }
            if self.pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.pass, None);
            }
        }
    }
}
