//! Source encoded RGBA upload directly into optimal images. No full VRAM
//! temporary buffer. A lazily grown <=8MiB host staging allocation and one
//! command/fence pair are reused between cold asset uploads, never per frame.
use super::{Resources, UPLOAD_CHUNK};
use crate::native_vulkan::VulkanContext;
use ash::{vk, Device};
pub(super) struct Transfer {
    lifetime: std::sync::Arc<super::super::lifetime::Lifetime>,
    device: Device,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    fence: vk::Fence,
}
impl Transfer {
    pub(super) fn handles(&self) -> (vk::CommandBuffer,vk::Fence) {(self.command,self.fence)}
    pub(super) fn new(context: &VulkanContext) -> Result<Self, String> {
        let device = context.device.clone();
        let command = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(context.command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|e| e.to_string())?[0];
        let fence = match unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) } {
            Ok(f) => f,
            Err(e) => {
                unsafe { device.free_command_buffers(context.command_pool, &[command]) };
                return Err(e.to_string());
            }
        };
        Ok(Self {
            lifetime:context.lifetime.clone(),
            device,
            pool: context.command_pool,
            command,
            fence,
        })
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        if self.lifetime.uncertain(){return;}
        unsafe {
            self.device.destroy_fence(self.fence, None);
            self.device.free_command_buffers(self.pool, &[self.command]);
        }
    }
}
impl Resources {
    /// Fresh RGBA8_UNORM image must have TRANSFER_DST|SAMPLED usage, and
    /// TRANSFER_SRC when mip_levels>1. Failure leaves only this new image
    /// partially prepared; callers must drop it and retain prior cached images.
    fn upload_rgba8_image(
        &mut self,
        context: &VulkanContext,
        image: vk::Image,
        extent: vk::Extent2D,
        data: &[u8],
        mip_levels: u32,
    ) -> Result<(), String> {
        self.lifetime.check()?;
        let layout = ingetrazo_render::texture_layout::Layout::new(
            ingetrazo_render::texture_layout::Format::Rgba8,
            extent.width,
            extent.height,
            mip_levels,
        )?;
        let bytes = layout.mips[0].bytes;
        if bytes != data.len() as u64 {
            return Err("RGBA image bytes do not match dimensions".into());
        }
        let row = extent.width as u64 * 4;
        if row > UPLOAD_CHUNK {
            return Err(
                "RGBA image row exceeds bounded Vulkan staging; tile upload required".into(),
            );
        }
        if mip_levels > 1 {
            let props = unsafe {
                context.instance.get_physical_device_format_properties(
                    context.physical_device,
                    vk::Format::R8G8B8A8_UNORM,
                )
            };
            let required = vk::FormatFeatureFlags::BLIT_SRC
                | vk::FormatFeatureFlags::BLIT_DST
                | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR;
            if !props.optimal_tiling_features.contains(required) {
                return Err(
                    "Device RGBA8 format does not support source linear GPU mip generation".into(),
                );
            }
        }
        let capacity = (UPLOAD_CHUNK / row * row).min(bytes);
        if self
            .image_staging
            .and_then(|id| self.logical_bytes(id))
            .is_none_or(|n| n < capacity)
        {
            // Every upload waits its transfer fence, so obsolete staging is idle.
            // Free before growth: total retained staging never temporarily doubles.
            if let Some(old) = self.image_staging.take() {
                self.destroy(old)
            }
            let id = self.allocate(
                capacity,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE,
            )?;
            let buffer = &self.buffers[&id];
            let mapped = match unsafe {self.device.map_memory(buffer.memory,0,buffer.allocation_bytes,vk::MemoryMapFlags::empty())} {
                Ok(ptr)=>ptr,
                Err(error)=>{self.destroy(id);return Err(error.to_string())}
            };
            self.buffers.get_mut(&id).unwrap().persistent_map=Some(mapped as usize);
            self.stats.image_staging_maps+=1;
            self.stats.staging_allocations += 1;
            self.stats.staging_peak = self
                .stats
                .staging_peak
                .max(self.allocation_bytes(id).unwrap());
            self.image_staging = Some(id);
        }
        let staging = self.image_staging.unwrap();
        let (command,fence)=self.transfer_handles(context)?;
        let mut submitted = false;
        let result: Result<(), String> = (|| {
            let mut offset = 0u64;
            while offset < bytes {
                let length = capacity.min(bytes - offset);
                self.write(staging, &data[offset as usize..(offset + length) as usize])?;
                unsafe {
                    self.device
                        .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
                        .map_err(|e| e.to_string())?;
                    self.device
                        .begin_command_buffer(
                            command,
                            &vk::CommandBufferBeginInfo::default()
                                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                        )
                        .map_err(|e| e.to_string())?;
                    if offset == 0 {
                        transition(
                            &self.device,
                            command,
                            image,
                            0,
                            mip_levels,
                            vk::ImageLayout::UNDEFINED,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            vk::AccessFlags::empty(),
                            vk::AccessFlags::TRANSFER_WRITE,
                            vk::PipelineStageFlags::TOP_OF_PIPE,
                            vk::PipelineStageFlags::TRANSFER,
                        )
                    }
                    let copy = vk::BufferImageCopy::default()
                        .image_subresource(
                            vk::ImageSubresourceLayers::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .layer_count(1),
                        )
                        .image_offset(vk::Offset3D {
                            x: 0,
                            y: (offset / row) as i32,
                            z: 0,
                        })
                        .image_extent(vk::Extent3D {
                            width: extent.width,
                            height: (length / row) as u32,
                            depth: 1,
                        });
                    self.device.cmd_copy_buffer_to_image(
                        command,
                        self.buffer(staging).unwrap(),
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[copy],
                    );
                    if offset + length == bytes {
                        finish_mips(&self.device, command, image, &layout);
                    }
                    self.device
                        .end_command_buffer(command)
                        .map_err(|e| e.to_string())?;
                    self.device
                        .reset_fences(&[fence])
                        .map_err(|e| e.to_string())?;
                    self.device
                        .queue_submit(
                            context.graphics_queue,
                            &[vk::SubmitInfo::default().command_buffers(&[command])],
                            fence,
                        )
                        .map_err(|e| {self.lifetime.quarantine();e.to_string()})?;
                    submitted = true;
                    self.stats.transfer_submissions += 1;
                }
                context.wait_transfer(fence).map_err(|e| e.to_string())?;
                submitted = false;
                self.stats.chunks += 1;
                offset += length;
            }
            Ok(())
        })();
        if submitted {
            self.lifetime.quarantine();
        }
        result?;
        self.stats.image_upload_calls += 1;
        self.stats.image_bytes += bytes;
        self.stats.image_mips_generated += (mip_levels - 1) as u64;
        Ok(())
    }
    pub fn release_image_staging(&mut self) {
        if let Some(id) = self.image_staging.take() {
            self.destroy(id)
        }
    }
    pub fn image_staging_bytes(&self) -> u64 {
        self.image_staging
            .and_then(|id| self.allocation_bytes(id))
            .unwrap_or(0)
    }
}
fn transition(
    device: &Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    base: u32,
    count: u32,
    old: vk::ImageLayout,
    new: vk::ImageLayout,
    src: vk::AccessFlags,
    dst: vk::AccessFlags,
    src_stage: vk::PipelineStageFlags,
    dst_stage: vk::PipelineStageFlags,
) {
    unsafe {
        let barrier = vk::ImageMemoryBarrier::default()
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(base)
                    .level_count(count)
                    .layer_count(1),
            )
            .old_layout(old)
            .new_layout(new)
            .src_access_mask(src)
            .dst_access_mask(dst)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED);
        device.cmd_pipeline_barrier(
            command,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }
}

fn finish_mips(
    device: &Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    layout: &ingetrazo_render::texture_layout::Layout,
) {
    let mip_levels = layout.mips.len() as u32;
    unsafe {
        for level in 1..mip_levels {
            transition(
                device,
                command,
                image,
                level - 1,
                1,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
            );
            let src = layout.mips[(level - 1) as usize];
            let dst = layout.mips[level as usize];
            let blit = vk::ImageBlit::default()
                .src_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(level - 1)
                        .layer_count(1),
                )
                .src_offsets([
                    vk::Offset3D::default(),
                    vk::Offset3D {
                        x: src.width as i32,
                        y: src.height as i32,
                        z: 1,
                    },
                ])
                .dst_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(level)
                        .layer_count(1),
                )
                .dst_offsets([
                    vk::Offset3D::default(),
                    vk::Offset3D {
                        x: dst.width as i32,
                        y: dst.height as i32,
                        z: 1,
                    },
                ]);
            device.cmd_blit_image(
                command,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                vk::Filter::LINEAR,
            );
            transition(
                device,
                command,
                image,
                level - 1,
                1,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::ALL_GRAPHICS,
            );
        }
        transition(
            device,
            command,
            image,
            mip_levels - 1,
            1,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
        );
    }
}

/// Fresh device-local sampled image. Owners must retain this resource until
/// their last GPU submission fence signals; construction completes its own
/// transfer fence before publishing the image. No implicit global idle on drop.
pub struct TextureImage {
    lifetime: std::sync::Arc<super::super::lifetime::Lifetime>,
    device: Device,
    budget: std::sync::Arc<super::VulkanBudget>,
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub sampler: vk::Sampler,
    memory: vk::DeviceMemory,
    pub extent: vk::Extent2D,
    pub mip_levels: u32,
    pub allocation_bytes: u64,
    readback_enabled: bool,
}
impl TextureImage {
    /// Decode bytes remain top-row first. Source V reversal belongs to the
    /// source-compatible UV shader, avoiding an extra CPU image flip/copy.
    pub fn rgba8(
        context: &VulkanContext,
        resources: &mut Resources,
        extent: vk::Extent2D,
        data: &[u8],
        mips: bool,
        diagnostic_readback: bool,
    ) -> Result<Self, String> {
        use egui_ash_renderer::MemoryObserver;
        let layout = ingetrazo_render::texture_layout::Layout::full(
            ingetrazo_render::texture_layout::Format::Rgba8,
            extent.width,
            extent.height,
        )?;
        let levels = if mips { layout.mips.len() as u32 } else { 1 };
        if layout.mips[0].bytes != data.len() as u64 {
            return Err("Decoded RGBA bytes differ from dimensions".into());
        }
        let mut usage = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
        if mips || diagnostic_readback {
            usage |= vk::ImageUsageFlags::TRANSFER_SRC;
        }
        let format = vk::Format::R8G8B8A8_UNORM;
        let props = unsafe {
            context
                .instance
                .get_physical_device_image_format_properties(
                    context.physical_device,
                    format,
                    vk::ImageType::TYPE_2D,
                    vk::ImageTiling::OPTIMAL,
                    usage,
                    vk::ImageCreateFlags::empty(),
                )
        }
        .map_err(|e| format!("RGBA texture format/usage unavailable: {e}"))?;
        let logical_bytes: u64 = layout.mips[..levels as usize].iter().map(|m|m.bytes).sum();
        if extent.width > props.max_extent.width
            || extent.height > props.max_extent.height
            || levels > props.max_mip_levels
            || props.max_array_layers < 1
            || !props.sample_counts.contains(vk::SampleCountFlags::TYPE_1)
            || logical_bytes > props.max_resource_size
        {
            return Err("Texture exceeds actual device format limits".into());
        }
        let features = unsafe {
            context
                .instance
                .get_physical_device_format_properties(context.physical_device, format)
        }
        .optimal_tiling_features;
        if !features.contains(
            vk::FormatFeatureFlags::SAMPLED_IMAGE
                | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR,
        ) {
            return Err("RGBA texture requires actual linear sampling support".into());
        }
        let device = context.device.clone();
        let budget = resources.budget.clone();
        let image = unsafe {
            device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(format)
                    .extent(vk::Extent3D {
                        width: extent.width,
                        height: extent.height,
                        depth: 1,
                    })
                    .mip_levels(levels)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(usage),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        let req = unsafe { device.get_image_memory_requirements(image) };
        if req.size > props.max_resource_size {
            unsafe {device.destroy_image(image,None)};
            return Err("Actual texture allocation exceeds queried format max resource size".into());
        }
        let mut memory = None;
        let mut view = vk::ImageView::null();
        let mut sampler = vk::Sampler::null();
        let result = (|| -> Result<(), String> {
            let ty = (0..resources.properties.memory_type_count)
                .find(|&i| {
                    req.memory_type_bits & (1 << i) != 0
                        && resources.properties.memory_types[i as usize]
                            .property_flags
                            .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
                })
                .ok_or("No device-local sampled image memory")?;
            let ticket = budget
                .reserve(
                    resources.properties.memory_types[ty as usize].heap_index,
                    req.size,
                )
                .map_err(|e| e.to_string())?;
            let allocated = match unsafe {
                device.allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(req.size)
                        .memory_type_index(ty),
                    None,
                )
            } {
                Ok(m) => m,
                Err(e) => {
                    budget.cancel(ticket);
                    return Err(e.to_string());
                }
            };
            if let Err(e) = unsafe { device.bind_image_memory(image, allocated, 0) } {
                unsafe { device.free_memory(allocated, None) };
                budget.cancel(ticket);
                return Err(e.to_string());
            }
            budget.commit(ticket, allocated);
            memory = Some(allocated);
            view = unsafe {
                device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(format)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .level_count(levels)
                                .layer_count(1),
                        ),
                    None,
                )
            }
            .map_err(|e| e.to_string())?;
            sampler = unsafe {
                device.create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::LINEAR)
                        .min_filter(vk::Filter::LINEAR)
                        .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                        .address_mode_u(vk::SamplerAddressMode::REPEAT)
                        .address_mode_v(vk::SamplerAddressMode::REPEAT)
                        .address_mode_w(vk::SamplerAddressMode::REPEAT)
                        .min_lod(0.)
                        .max_lod((levels - 1) as f32),
                    None,
                )
            }
            .map_err(|e| e.to_string())?;
            resources.upload_rgba8_image(context, image, extent, data, levels)
        })();
        if let Err(e) = result {
            if context.lifetime.uncertain(){context.lifetime.retain(budget.clone());context.lifetime.retain((image,view,sampler,memory));return Err(e);}
            unsafe {
                if sampler != vk::Sampler::null() {
                    device.destroy_sampler(sampler, None)
                }
                if view != vk::ImageView::null() {
                    device.destroy_image_view(view, None)
                }
                device.destroy_image(image, None);
                if let Some(m) = memory {
                    device.free_memory(m, None);
                    budget.freed(m)
                }
            }
            return Err(e);
        }
        Ok(Self {
            lifetime:context.lifetime.clone(),
            device,
            budget,
            image,
            view,
            sampler,
            memory: memory.unwrap(),
            extent,
            mip_levels: levels,
            allocation_bytes: req.size,
            readback_enabled: diagnostic_readback,
        })
    }
}
impl Drop for TextureImage {
    fn drop(&mut self) {
        use egui_ash_renderer::MemoryObserver;
        if self.lifetime.uncertain(){self.lifetime.retain(self.budget.clone());self.lifetime.retain((self.image,self.view,self.sampler,self.memory));return;}
        unsafe {
            self.device.destroy_sampler(self.sampler, None);
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.memory, None)
        }
        self.budget.freed(self.memory);
    }
}

impl Resources {
    /// Diagnostic-only transfer from a sampled mip. Image must have TRANSFER_SRC
    /// usage. Restores its sampled layout and waits the real GPU transfer fence.
    pub fn readback_texture_mip(
        &mut self,
        context: &VulkanContext,
        texture: &TextureImage,
        level: u32,
    ) -> Result<Vec<u8>, String> {
        if !texture.readback_enabled {
            return Err("Texture diagnostic readback usage was not requested".into());
        }
        if level >= texture.mip_levels {
            return Err("Texture mip outside allocated image".into());
        }
        let extent = vk::Extent2D {
            width: (texture.extent.width >> level).max(1),
            height: (texture.extent.height >> level).max(1),
        };
        let row = extent.width as u64 * 4;
        let bytes = row * extent.height as u64;
        self.read_chunks(
            context,
            bytes,
            row,
            |device, command, destination, offset, length| unsafe {
                transition(
                    device,
                    command,
                    texture.image,
                    level,
                    1,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::AccessFlags::SHADER_READ,
                    vk::AccessFlags::TRANSFER_READ,
                    vk::PipelineStageFlags::ALL_GRAPHICS,
                    vk::PipelineStageFlags::TRANSFER,
                );
                let region = vk::BufferImageCopy::default()
                    .image_subresource(
                        vk::ImageSubresourceLayers::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .mip_level(level)
                            .layer_count(1),
                    )
                    .image_offset(vk::Offset3D {
                        x: 0,
                        y: (offset / row) as i32,
                        z: 0,
                    })
                    .image_extent(vk::Extent3D {
                        width: extent.width,
                        height: (length / row) as u32,
                        depth: 1,
                    });
                device.cmd_copy_image_to_buffer(
                    command,
                    texture.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    destination,
                    &[region],
                );
                transition(
                    device,
                    command,
                    texture.image,
                    level,
                    1,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::AccessFlags::TRANSFER_READ,
                    vk::AccessFlags::SHADER_READ,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::ALL_GRAPHICS,
                );
            },
        )
    }
}
