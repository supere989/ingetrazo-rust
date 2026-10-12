//! Sample immutable encoded RGBA in the existing physical paper render pass.
//! Cold uploads share Exporter's Resources and budget; every tile reuses them.
use super::{
    resources::{BufferId, Resources},
    VulkanContext,
};
use ash::{vk, Device};
use ingetrazo_render::paper_picture::{Pixels, Sampling};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
struct Image {
    _owner: Arc<Pixels>,
    buffer: BufferId,
}
struct Prepared {
    sampling: Arc<Sampling>,
    image: BufferId,
    rows: BufferId,
    set: vk::DescriptorSet,
    push: [i32; 24],
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Stats {
    pub image_uploads: u64,
    pub row_uploads: u64,
    pub uploaded_bytes: u64,
    pub warm_hits: u64,
    pub cached_image_bytes: u64,
}
pub(super) struct Renderer {
    device: Device,
    lifetime: Arc<super::lifetime::Lifetime>,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    descriptor_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    images: HashMap<u64, Image>,
    prepared: Vec<Prepared>,
    max_storage_range: u64,
    stats: Stats,
}
impl Renderer {
    pub fn new(context: &VulkanContext, pass: vk::RenderPass) -> Result<Self, String> {
        context.lifetime.check()?;
        let mut out = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            pipeline: vk::Pipeline::null(),
            layout: vk::PipelineLayout::null(),
            descriptor_layout: vk::DescriptorSetLayout::null(),
            pool: vk::DescriptorPool::null(),
            images: HashMap::new(),
            prepared: Vec::new(),
            stats: Stats::default(),
            max_storage_range: u64::from(
                unsafe {
                    context
                        .instance
                        .get_physical_device_properties(context.physical_device)
                }
                .limits
                .max_storage_buffer_range,
            ),
        };
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        out.descriptor_layout = out.vk(unsafe {
            out.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        })?;
        let layouts = [out.descriptor_layout];
        let ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 96,
        }];
        out.layout = out.vk(unsafe {
            out.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&ranges),
                None,
            )
        })?;
        out.pipeline = make_pipeline(&out.device, pass, out.layout).map_err(|error| {
            if error.device_lost() {
                out.lifetime.quarantine();
            }
            error.to_string()
        })?;
        Ok(out)
    }
    fn vk<T>(&self, result: Result<T, vk::Result>) -> Result<T, String> {
        result.map_err(|e| {
            if e == vk::Result::ERROR_DEVICE_LOST {
                self.lifetime.quarantine();
            }
            e.to_string()
        })
    }
    /// The prior tile fence is already complete. Empty/off-page pictures never
    /// create descriptors or upload a pixel; their ordered record maps to None.
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        resources: &mut Resources,
        pictures: &[Arc<Sampling>],
        view: vk::ImageView,
        fence: vk::Fence,
        serial: u64,
    ) -> Result<Vec<Option<usize>>, String> {
        if !Arc::ptr_eq(&self.lifetime, &context.lifetime) {
            return Err("Paper picture context owner changed".into());
        }
        self.lifetime.check()?;
        if pictures.len() > 4096 {
            return Err("Paper picture descriptor count exceeds 4096".into());
        }
        // Complete all cold domain/allocation admission before retiring cache entries.
        let mut active = HashSet::new();
        let mut admitted = Vec::new();
        let mut mapping = Vec::new();
        admitted
            .try_reserve_exact(pictures.len())
            .map_err(|_| "Picture admission allocation failed")?;
        mapping
            .try_reserve_exact(pictures.len())
            .map_err(|_| "Picture handle allocation failed")?;
        active
            .try_reserve(pictures.len())
            .map_err(|_| "Picture identity allocation failed")?;
        for sampling in pictures {
            admit(sampling, self.max_storage_range)?;
            if sampling.fy.is_empty()
                || sampling.x[2] < sampling.x[0]
                || sampling.rect_fixed[2] <= 0
                || sampling.rect_fixed[3] <= 0
                || sampling.rect_fixed[0] >= i64::from(sampling.page[0]) * 65536
                || sampling.rect_fixed[1] >= i64::from(sampling.page[1]) * 65536
            {
                mapping.push(None);
            } else {
                mapping.push(Some(admitted.len()));
                active.insert(sampling.image.id());
                admitted.push(sampling.clone());
            }
        }
        let old: Vec<_> = self
            .prepared
            .iter()
            .map(|p| p.rows)
            .chain(
                self.images
                    .iter()
                    .filter(|(id, _)| !active.contains(*id))
                    .map(|(_, image)| image.buffer),
            )
            .collect();
        let completed = resources.publication_preflight(context, fence, serial, &old, &[])?;
        if !admitted.is_empty()
            && admitted.len() == self.prepared.len()
            && admitted
                .iter()
                .zip(&self.prepared)
                .all(|(sampling, old)| Arc::ptr_eq(sampling, &old.sampling))
        {
            self.stats.warm_hits = self.stats.warm_hits.saturating_add(active.len() as u64);
            return Ok(mapping);
        }
        if admitted.is_empty() {
            if self.pool != vk::DescriptorPool::null() {
                unsafe { self.device.destroy_descriptor_pool(self.pool, None) };
                self.pool = vk::DescriptorPool::null();
            }
            self.prepared.clear();
            self.images.clear();
            resources.retire_completed(completed);
            return Ok(mapping);
        }
        self.images
            .try_reserve(admitted.len())
            .map_err(|_| "Picture image cache allocation failed")?;
        let mut candidate = Vec::new();
        candidate
            .try_reserve_exact(admitted.len())
            .map_err(|_| "Picture prepared allocation failed")?;
        let mut missing = Vec::new();
        let mut seen = HashSet::new();
        for sampling in &admitted {
            if !self.images.contains_key(&sampling.image.id()) && seen.insert(sampling.image.id()) {
                missing.push(sampling.image.clone());
            }
        }
        let mut inputs = Vec::new();
        inputs
            .try_reserve_exact(missing.len() + admitted.len())
            .map_err(|_| "Picture upload inventory allocation failed")?;
        for image in &missing {
            inputs.push((image.rgba(), vk::BufferUsageFlags::STORAGE_BUFFER));
        }
        for sampling in &admitted {
            inputs.push((
                bytemuck::cast_slice(sampling.fy.as_slice()),
                vk::BufferUsageFlags::STORAGE_BUFFER,
            ));
        }
        let uploaded_bytes = inputs
            .iter()
            .map(|(bytes, _)| bytes.len() as u64)
            .sum::<u64>();
        let buffers = resources.upload_many(context, &inputs)?;
        // These counters record completed transfers, including a later descriptor refusal.
        self.stats.image_uploads = self
            .stats
            .image_uploads
            .saturating_add(missing.len() as u64);
        self.stats.row_uploads = self.stats.row_uploads.saturating_add(admitted.len() as u64);
        self.stats.uploaded_bytes = self.stats.uploaded_bytes.saturating_add(uploaded_bytes);
        let prepared = (|| -> Result<(vk::DescriptorPool, Vec<vk::DescriptorSet>), String> {
            let count = admitted.len() as u32;
            let sizes = [
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::INPUT_ATTACHMENT,
                    descriptor_count: count,
                },
                vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_BUFFER,
                    descriptor_count: count * 2,
                },
            ];
            let pool = self.vk(unsafe {
                self.device.create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(count)
                        .pool_sizes(&sizes),
                    None,
                )
            })?;
            let layouts = vec![self.descriptor_layout; admitted.len()];
            match self.vk(unsafe {
                self.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&layouts),
                )
            }) {
                Ok(sets) => Ok((pool, sets)),
                Err(error) => {
                    if self.lifetime.uncertain() {
                        self.lifetime.retain(pool);
                    } else {
                        unsafe { self.device.destroy_descriptor_pool(pool, None) }
                    }
                    Err(error)
                }
            }
        })();
        let (pool, sets) = match prepared {
            Ok(value) => value,
            Err(error) => {
                if let Ok(proof) =
                    resources.publication_preflight(context, fence, serial, &buffers, &[])
                {
                    resources.retire_completed(proof);
                }
                return Err(error);
            }
        };
        for (image, buffer) in missing.iter().zip(&buffers) {
            self.images.insert(
                image.id(),
                Image {
                    _owner: image.clone(),
                    buffer: *buffer,
                },
            );
        }
        for ((sampling, rows), set) in admitted
            .into_iter()
            .zip(buffers[missing.len()..].iter().copied())
            .zip(sets)
        {
            let image = self.images[&sampling.image.id()].buffer;
            let target = [vk::DescriptorImageInfo::default()
                .image_view(view)
                .image_layout(vk::ImageLayout::GENERAL)];
            let pixels = [vk::DescriptorBufferInfo::default()
                .buffer(resources.buffer(image).unwrap())
                .range(sampling.image.rgba().len() as u64)];
            let fy = [vk::DescriptorBufferInfo::default()
                .buffer(resources.buffer(rows).unwrap())
                .range((sampling.fy.len() * 4) as u64)];
            let writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT)
                    .image_info(&target),
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&pixels),
                vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&fy),
            ];
            unsafe { self.device.update_descriptor_sets(&writes, &[]) };
            let push = sampling
                .native_push([1, 1], [0, 0])
                .expect("admitted immutable sampling");
            candidate.push(Prepared {
                sampling,
                image,
                rows,
                set,
                push,
            });
        }
        // All fallible candidate work has succeeded. Publish then retire the
        // previous descriptor/row owners under their already completed fence.
        let old_pool = std::mem::replace(&mut self.pool, pool);
        self.prepared = candidate;
        self.images.retain(|id, _| active.contains(id));
        if old_pool != vk::DescriptorPool::null() {
            unsafe { self.device.destroy_descriptor_pool(old_pool, None) };
        }
        resources.retire_completed(completed);
        self.stats.warm_hits = self
            .stats
            .warm_hits
            .saturating_add((active.len() - missing.len()) as u64);
        Ok(mapping)
    }
    pub fn stats(&self) -> Stats {
        Stats {
            cached_image_bytes: self
                .images
                .values()
                .map(|image| image._owner.rgba().len() as u64)
                .sum(),
            ..self.stats
        }
    }
    pub fn mark_used(&self, resources: &mut Resources, serial: u64) -> Result<(), String> {
        for p in &self.prepared {
            resources.mark_used(p.image, serial)?;
            resources.mark_used(p.rows, serial)?;
        }
        Ok(())
    }
    pub fn record(
        &self,
        index: usize,
        command: vk::CommandBuffer,
        extent: vk::Extent2D,
        origin: [u32; 2],
    ) {
        let p = &self.prepared[index];
        let s = &p.sampling;
        let mut push = p.push;
        push[..4].copy_from_slice(&[
            extent.width as i32,
            extent.height as i32,
            origin[0] as i32,
            origin[1] as i32,
        ]);
        let bounds = &push[4..8];
        let x0 = bounds[0].max(origin[0] as i32);
        let y0 = bounds[1].max(origin[1] as i32);
        let x1 = bounds[2]
            .min((origin[0] + extent.width) as i32)
            .min(s.page[0] as i32);
        let y1 = bounds[3]
            .min((origin[1] + extent.height) as i32)
            .min(s.page[1] as i32);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        unsafe {
            let barrier = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::INPUT_ATTACHMENT_READ)];
            self.device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::BY_REGION,
                &barrier,
                &[],
                &[],
            );
            self.device
                .cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            self.device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::GRAPHICS,
                self.layout,
                0,
                &[p.set],
                &[],
            );
            self.device.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport {
                    x: 0.,
                    y: 0.,
                    width: extent.width as f32,
                    height: extent.height as f32,
                    min_depth: 0.,
                    max_depth: 1.,
                }],
            );
            self.device.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D {
                        x: x0 - origin[0] as i32,
                        y: y0 - origin[1] as i32,
                    },
                    extent: vk::Extent2D {
                        width: (x1 - x0) as u32,
                        height: (y1 - y0) as u32,
                    },
                }],
            );
            self.device.cmd_push_constants(
                command,
                self.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
            self.device.cmd_draw(command, 3, 1, 0, 0);
        }
    }
}
fn admit(s: &Sampling, max_storage_range: u64) -> Result<(), String> {
    if s.page.contains(&0)
        || s.page.iter().any(|&n| n > 1_000_000)
        || s.rect_fixed
            .iter()
            .any(|&n| n.unsigned_abs() > 65_536_000_000)
        || s.rect_fixed[2] <= s.rect_fixed[0]
        || s.rect_fixed[3] <= s.rect_fixed[1]
        || s.top < 0
        || (s.top as usize)
            .checked_add(s.fy.len())
            .is_none_or(|n| n > s.page[1] as usize)
        || s.image.rgba().len() as u64 > max_storage_range
        || (s.fy.len() as u64) * 4 > max_storage_range
        || s.x[0] < 0
        || s.x[0] > s.x[1]
        || s.x[1] > s.x[2].saturating_add(1)
        || s.x[2] > s.page[0] as i32
        || s.fdx < 0
    {
        return Err("Paper picture native sampling domain invalid".into());
    }
    for x in [s.x[1], s.x[2]] {
        if i32::try_from(i64::from(s.fx[1]) + i64::from(s.fdx) * i64::from(x - s.x[1])).is_err() {
            return Err("Paper picture fixed sampling range overflow".into());
        }
    }
    s.native_push([1, 1], [0, 0])?;
    Ok(())
}
impl Drop for Renderer {
    fn drop(&mut self) {
        if self.lifetime.uncertain() {
            self.lifetime.retain((
                self.pipeline,
                self.layout,
                self.descriptor_layout,
                self.pool,
                std::mem::take(&mut self.images),
                std::mem::take(&mut self.prepared),
            ));
            return;
        }
        unsafe {
            if self.pool != vk::DescriptorPool::null() {
                self.device.destroy_descriptor_pool(self.pool, None);
            }
            if self.pipeline != vk::Pipeline::null() {
                self.device.destroy_pipeline(self.pipeline, None);
            }
            if self.layout != vk::PipelineLayout::null() {
                self.device.destroy_pipeline_layout(self.layout, None);
            }
            if self.descriptor_layout != vk::DescriptorSetLayout::null() {
                self.device
                    .destroy_descriptor_set_layout(self.descriptor_layout, None);
            }
        }
    }
}
fn make_pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline, super::resize::PrepareError> {
    let mut modules = Vec::new();
    let result = (|| {
        for bytes in [
            include_bytes!("shaders/paper_picture.vert.spv").as_slice(),
            include_bytes!("shaders/paper_picture.frag.spv").as_slice(),
        ] {
            let code =
                ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
            modules.push(
                unsafe {
                    device.create_shader_module(
                        &vk::ShaderModuleCreateInfo::default().code(&code),
                        None,
                    )
                }
                .map_err(super::resize::PrepareError::from)?,
            );
        }
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(modules[0])
                .name(c"main"),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(modules[1])
                .name(c"main"),
        ];
        let input = vk::PipelineVertexInputStateCreateInfo::default();
        let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.)
            .cull_mode(vk::CullModeFlags::NONE);
        let samples = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let attachments = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(false)
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
        let dynamic = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);
        match unsafe {
            device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::default()
                    .stages(&stages)
                    .vertex_input_state(&input)
                    .input_assembly_state(&assembly)
                    .viewport_state(&viewport)
                    .rasterization_state(&raster)
                    .multisample_state(&samples)
                    .color_blend_state(&blend)
                    .dynamic_state(&dynamic)
                    .layout(layout)
                    .render_pass(pass)],
                None,
            )
        } {
            Ok(pipelines) => Ok(pipelines[0]),
            Err((partial, error)) => {
                for pipeline in partial {
                    unsafe { device.destroy_pipeline(pipeline, None) }
                }
                Err(error.into())
            }
        }
    })();
    for module in modules {
        unsafe { device.destroy_shader_module(module, None) }
    }
    result
}
