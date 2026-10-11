//! Reference-plane native submission: static six-vertex shader, raw opacity,
//! source list order, no face cutout/shading. Images share the parent texture cache.
use super::{
    resources::Resources,
    texture_cache::{AssetId, PreparedTexture, TextureCache},
    VulkanContext,
};
use ash::{vk, Device};
use ingetrazo_core::{image_plane::ImagePlane, scene::Scene};
use std::collections::HashMap;
struct Pool {
    device: Device,
    handle: vk::DescriptorPool,
}
impl Drop for Pool {
    fn drop(&mut self) {
        unsafe { self.device.destroy_descriptor_pool(self.handle, None) }
    }
}
struct Draw {
    index: usize,
    id: uuid::Uuid,
    asset: AssetId,
    set: vk::DescriptorSet,
    frame: [[f32; 4]; 3],
    opaque: bool,
}
impl Draw {
    fn update(&mut self, p: &ImagePlane) {
        self.frame = [
            [
                p.origin.x as f32,
                p.origin.y as f32,
                p.origin.z as f32,
                p.opacity.clamp(0., 1.) as f32,
            ],
            [p.u.x as f32, p.u.y as f32, p.u.z as f32, 0.],
            [p.v.x as f32, p.v.y as f32, p.v.z as f32, 0.],
        ];
        self.opaque = p.opacity.clamp(0., 1.) >= 0.999;
    }
}
pub struct Candidate {
    pool: Option<Pool>,
    draws: Vec<Draw>,
    images: Vec<PreparedTexture>,
}
impl Candidate {
    pub(crate) fn images(&self)->impl Iterator<Item=&PreparedTexture>{self.images.iter()}
    pub fn sampled(&self)->impl Iterator<Item=(AssetId,(vk::ImageView,vk::Sampler,bool))>+'_ {self.images.iter().map(|p|(p.id(),p.sampled()))}

    pub fn assets(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.draws.iter().map(|d| d.asset)
    }
}
#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub builds: u64,
    pub pose_refreshes: u64,
    pub draws: u64,
    pub missing: u64,
}
pub struct ReferenceImages {
    device: Device,
    layout: vk::PipelineLayout,
    descriptor_layout: vk::DescriptorSetLayout,
    pipelines: [vk::Pipeline; 2],
    pool: Option<Pool>,
    draws: Vec<Draw>,
    publication: u64,
    pub stats: Stats,
}
impl ReferenceImages {
    pub fn new(
        context: &VulkanContext,
        pass: vk::RenderPass,
        samples: vk::SampleCountFlags,
    ) -> Result<Self, String> {
        let mut r = Self {
            device: context.device.clone(),
            layout: vk::PipelineLayout::null(),
            descriptor_layout: vk::DescriptorSetLayout::null(),
            pipelines: [vk::Pipeline::null(); 2],
            pool: None,
            draws: vec![],
            publication: 0,
            stats: Stats::default(),
        };
        let bindings = [vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        r.descriptor_layout = unsafe {
            r.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        r.layout = unsafe {
            r.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[r.descriptor_layout])
                    .push_constant_ranges(&[vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .size(128)]),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        for i in 0..2 {
            r.pipelines[i] = pipeline(&r.device, pass, r.layout, samples, i, context.source_depth_clip_enabled)?;
        }
        Ok(r)
    }
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        resources: &mut Resources,
        cache: &mut TextureCache,
        scene: &Scene,
        fence: vk::Fence,
    ) -> Result<Candidate, String> {
        if !unsafe { self.device.get_fence_status(fence) }.map_err(|e| e.to_string())? {
            return Err("Image replacement requires completed draw fence".into());
        }
        let mut c = Candidate {
            pool: None,
            draws: vec![],
            images: vec![],
        };
        if scene.image_planes.is_empty() {
            return Ok(c);
        }
        let count = u32::try_from(scene.image_planes.len()).map_err(|_| "Too many image planes")?;
        let pool = unsafe {
            self.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(count)
                    .pool_sizes(&[vk::DescriptorPoolSize {
                        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        descriptor_count: count,
                    }]),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        c.pool = Some(Pool {
            device: self.device.clone(),
            handle: pool,
        });
        let mut sampled = HashMap::new();
        let mut pending_bytes=0usize;let mut pending_start=0usize;
        for (index, p) in scene.image_planes.iter().enumerate() {
            if !image_visible(scene, p) {
                continue;
            }
            let file;
            let bytes = if let Some(b) = scene.embedded_textures.get(&p.path) {
                b.as_slice()
            } else {
                match std::fs::read(&p.path) {
                    Ok(b) => {
                        file = b;
                        file.as_slice()
                    }
                    Err(_) => {
                        self.stats.missing += 1;
                        continue;
                    }
                }
            };
            let encoded = cache.identify(bytes);
            let id = encoded.id();
            let value = if let Some(v) = sampled.get(&id) {
                *v
            } else {
                let image = cache.prepare_deferred(context, resources, &encoded, 256 * 1024 * 1024)?;
                pending_bytes=pending_bytes.checked_add(image.pending_bytes()).ok_or("Reference pending image bytes overflow")?;
                let v = image.sampled();
                sampled.insert(id, v);
                c.images.push(image);
                if pending_bytes>=8*1024*1024{cache.complete_many(context,resources,c.images[pending_start..].iter_mut())?;pending_bytes=0;pending_start=c.images.len();}
                v
            };
            let set = unsafe {
                self.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(pool)
                        .set_layouts(&[self.descriptor_layout]),
                )
            }
            .map_err(|e| e.to_string())?[0];
            let infos = [vk::DescriptorImageInfo::default()
                .image_view(value.0)
                .sampler(value.1)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            unsafe {
                self.device.update_descriptor_sets(
                    &[vk::WriteDescriptorSet::default()
                        .dst_set(set)
                        .dst_binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(&infos)],
                    &[],
                )
            };
            let mut draw = Draw {
                index,
                id: p.id,
                asset: id,
                set,
                frame: [[0.; 4]; 3],
                opaque: false,
            };
            draw.update(p);
            c.draws.push(draw);
        }
        self.stats.builds += 1;
        cache.complete_many(context,resources,c.images[pending_start..].iter_mut())?;
        Ok(c)
    }
    pub fn publish(&mut self,cache:&mut TextureCache,c:Candidate,publication:u64)->Result<(),String>{
        cache.preflight_publication(c.images())?;
        self.publish_admitted(cache,c,publication);Ok(())
    }
    pub(crate) fn publish_admitted(&mut self,cache:&mut TextureCache,mut c:Candidate,publication:u64){
        for image in c.images.drain(..){cache.publish_admitted(image);}
        drop(self.pool.take());self.pool=c.pool.take();self.draws=c.draws;self.publication=publication;
    }
    pub fn assets(&self) -> impl Iterator<Item = AssetId> + '_ {
        self.draws.iter().map(|d| d.asset)
    }
    pub fn refresh_pose(&mut self, scene: &Scene) -> Result<(), String> {
        if self.publication == scene.image_planes.publication_revision() {
            return Ok(());
        }
        for draw in &self.draws {
            scene.image_planes.get(draw.index).filter(|p|p.id==draw.id).ok_or("Image membership changed before descriptor preparation")?;
        }
        for draw in &mut self.draws {
            draw.update(scene.image_planes.get(draw.index).unwrap());
        }
        self.publication = scene.image_planes.publication_revision();
        self.stats.pose_refreshes += 1;
        Ok(())
    }
    pub fn record(
        &mut self,
        cache: &mut TextureCache,
        command: vk::CommandBuffer,
        serial: u64,
        mvp: &[f32; 16],
    ) -> Result<(), String> {
        for d in &self.draws {
            cache.mark_used(d.asset, serial)?;
            let mut push = [0f32; 32];
            push[..16].copy_from_slice(mvp);
            for i in 0..3 {
                push[16 + i * 4..20 + i * 4].copy_from_slice(&d.frame[i]);
            }
            unsafe {
                self.device.cmd_bind_pipeline(
                    command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipelines[if d.opaque { 0 } else { 1 }],
                );
                self.device.cmd_bind_descriptor_sets(
                    command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.layout,
                    0,
                    &[d.set],
                    &[],
                );
                self.device.cmd_push_constants(
                    command,
                    self.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    bytemuck::cast_slice(&push),
                );
                self.device.cmd_draw(command, 6, 1, 0, 0);
            }
            self.stats.draws += 1;
        }
        Ok(())
    }
}
fn image_visible(scene: &Scene, p: &ImagePlane) -> bool {
    let name = ingetrazo_core::layers::entity_layer_name(p.layer.as_deref());
    scene
        .layers
        .iter()
        .find(|l| l.name == name)
        .map_or(true, |l| l.visible)
}
impl Drop for ReferenceImages {
    fn drop(&mut self) {
        drop(self.pool.take());
        unsafe {
            for p in self.pipelines {
                self.device.destroy_pipeline(p, None);
            }
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_layout, None);
        }
    }
}
fn pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    samples: vk::SampleCountFlags,
    index: usize,
    source_depth: bool,
) -> Result<vk::Pipeline, String> {
    let load = |bytes: &[u8]| {
        ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e| e.to_string())
    };
    let v = load(include_bytes!("shaders/image_plane.vert.spv"))?;
    let f = load(include_bytes!("shaders/image_plane.frag.spv"))?;
    let vertex = unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&v), None)
    }
    .map_err(|e| e.to_string())?;
    let fragment = match unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&f), None)
    } {
        Ok(f) => f,
        Err(e) => {
            unsafe { device.destroy_shader_module(vertex, None) };
            return Err(e.to_string());
        }
    };
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex)
            .name(c"main"),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment)
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
        .front_face(vk::FrontFace::CLOCKWISE)
        .cull_mode(vk::CullModeFlags::NONE)
        .depth_bias_enable(index == 0)
        .depth_bias_constant_factor(8.)
        .depth_bias_slope_factor(4.);
    let msaa = vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);
    let depth = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(index == 0)
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
    let colors = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        .src_alpha_blend_factor(vk::BlendFactor::ZERO)
        .dst_alpha_blend_factor(vk::BlendFactor::ONE)
        .alpha_blend_op(vk::BlendOp::ADD)
        .color_write_mask(vk::ColorComponentFlags::RGBA)];
    let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);
    let mut states=vec![vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR];if source_depth{states.push(vk::DynamicState::DEPTH_CLIP_NEGATIVE_ONE_TO_ONE_EXT);}
    let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&states);
    let result = unsafe {
        device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &[vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&input)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&msaa)
                .depth_stencil_state(&depth)
                .color_blend_state(&blend)
                .dynamic_state(&dynamic)
                .layout(layout)
                .render_pass(pass)],
            None,
        )
    };
    unsafe {
        device.destroy_shader_module(vertex, None);
        device.destroy_shader_module(fragment, None);
    }
    match result {
        Ok(p) => Ok(p[0]),
        Err((partial, e)) => {
            for p in partial {
                unsafe { device.destroy_pipeline(p, None) }
            }
            Err(e.to_string())
        }
    }
}
