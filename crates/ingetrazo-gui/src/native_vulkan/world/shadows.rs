// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 IngeTrazo contributors.

//! Native source shadow map ownership. Disabled scenes allocate no shadow map.
//! GPU completion is owned by the same World graphics Lifetime and ledger.
use super::Image;
use crate::native_vulkan::{budget::VulkanBudget, lifetime::Lifetime, VulkanContext};
use ash::{vk, Device};
use std::sync::Arc;
pub(super) const MAP_SIZE: u32 = 2048;
fn device_error(lifetime: &Lifetime, error: vk::Result) -> String {
    if error == vk::Result::ERROR_DEVICE_LOST {
        lifetime.quarantine();
    }
    error.to_string()
}
pub(super) struct Map {
    device: Device,
    lifetime: Arc<Lifetime>,
    pub render_pass: vk::RenderPass,
    pub framebuffer: vk::Framebuffer,
    color: Option<Image>,
    depth: Option<Image>,
    pub sampler: vk::Sampler,
    readback: bool,
}
impl Map {
    pub fn new(
        context: &VulkanContext,
        budget: Arc<VulkanBudget>,
        readback: bool,
    ) -> Result<Self, String> {
        context.lifetime.check()?;
        let extent = vk::Extent2D {
            width: MAP_SIZE,
            height: MAP_SIZE,
        };
        let color_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT
            | vk::ImageUsageFlags::SAMPLED
            | if readback {
                vk::ImageUsageFlags::TRANSFER_SRC
            } else {
                vk::ImageUsageFlags::empty()
            };
        let depth_usage = vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
        for (format, usage) in [
            (vk::Format::R8G8B8A8_UNORM, color_usage),
            (vk::Format::D24_UNORM_S8_UINT, depth_usage),
        ] {
            let info = unsafe {
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
            .map_err(|e| format!("Source shadow format {format:?} unavailable: {e}"))?;
            if info.max_extent.width < MAP_SIZE
                || info.max_extent.height < MAP_SIZE
                || !info.sample_counts.contains(vk::SampleCountFlags::TYPE_1)
            {
                return Err("Source 2048 single-sample shadow attachment unavailable".into());
            }
        }
        let color = Image::new(
            context,
            budget.clone(),
            extent,
            vk::Format::R8G8B8A8_UNORM,
            color_usage,
            vk::ImageAspectFlags::COLOR,
        )?;
        let depth = Image::new(
            context,
            budget,
            extent,
            vk::Format::D24_UNORM_S8_UINT,
            depth_usage,
            vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
        )?;
        let mut owner = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            render_pass: vk::RenderPass::null(),
            framebuffer: vk::Framebuffer::null(),
            color: Some(color),
            depth: Some(depth),
            sampler: vk::Sampler::null(),
            readback,
        };
        let attachments = [
            vk::AttachmentDescription::default()
                .format(vk::Format::R8G8B8A8_UNORM)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),
            vk::AttachmentDescription::default()
                .format(vk::Format::D24_UNORM_S8_UINT)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];
        let colors = [vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        }];
        let depth = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        let subpasses = [vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&colors)
            .depth_stencil_attachment(&depth)];
        let deps = [
            vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                .dst_stage_mask(
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                )
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                ),
            vk::SubpassDependency::default()
                .src_subpass(0)
                .dst_subpass(vk::SUBPASS_EXTERNAL)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ),
        ];
        owner.render_pass = unsafe {
            owner.device.create_render_pass(
                &vk::RenderPassCreateInfo::default()
                    .attachments(&attachments)
                    .subpasses(&subpasses)
                    .dependencies(&deps),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        let views = [
            owner.color.as_ref().unwrap().view,
            owner.depth.as_ref().unwrap().view,
        ];
        owner.framebuffer = unsafe {
            owner.device.create_framebuffer(
                &vk::FramebufferCreateInfo::default()
                    .render_pass(owner.render_pass)
                    .attachments(&views)
                    .width(MAP_SIZE)
                    .height(MAP_SIZE)
                    .layers(1),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        owner.sampler = unsafe {
            owner.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        Ok(owner)
    }
    pub fn view(&self) -> vk::ImageView {
        self.color.as_ref().unwrap().view
    }
    pub fn begin(&self, command: vk::CommandBuffer) {
        let extent = vk::Extent2D {
            width: MAP_SIZE,
            height: MAP_SIZE,
        };
        let clear = [
            vk::ClearValue {
                color: vk::ClearColorValue { float32: [1.; 4] },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.,
                    stencil: 0,
                },
            },
        ];
        unsafe {
            self.device.cmd_begin_render_pass(
                command,
                &vk::RenderPassBeginInfo::default()
                    .render_pass(self.render_pass)
                    .framebuffer(self.framebuffer)
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D::default(),
                        extent,
                    })
                    .clear_values(&clear),
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport {
                    x: 0.,
                    y: 0.,
                    width: MAP_SIZE as f32,
                    height: MAP_SIZE as f32,
                    min_depth: 0.,
                    max_depth: 1.,
                }],
            );
            self.device.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D::default(),
                    extent,
                }],
            );
        }
    }
}
impl Drop for Map {
    fn drop(&mut self) {
        if self.lifetime.uncertain() {
            self.lifetime.retain((
                self.device.clone(),
                self.render_pass,
                self.framebuffer,
                self.sampler,
                self.color.take(),
                self.depth.take(),
            ));
            return;
        }
        unsafe {
            if self.framebuffer != vk::Framebuffer::null() {
                self.device.destroy_framebuffer(self.framebuffer, None);
            }
            if self.sampler != vk::Sampler::null() {
                self.device.destroy_sampler(self.sampler, None);
            }
            if self.render_pass != vk::RenderPass::null() {
                self.device.destroy_render_pass(self.render_pass, None);
            }
        }
    }
}

/// Source `_light_vp`: preserve Python-double bounds operations, QVector3D
/// float construction/operators, and QMatrix4x4 float multiplication order.
pub(super) fn light_matrix(
    direction: [f64; 3],
    lo: [f32; 3],
    hi: [f32; 3],
) -> Result<[f32; 16], String> {
    if !direction.iter().all(|v| v.is_finite())
        || !lo.iter().chain(hi.iter()).all(|v| v.is_finite())
        || lo.iter().zip(hi).any(|(l, h)| *l > h)
    {
        return Err("Shadow light bounds/direction admission".into());
    }
    let lowz = f64::from(lo[2]).min(0.);
    let center = glam::Vec3::new(
        ((f64::from(lo[0]) + f64::from(hi[0])) / 2.) as f32,
        ((f64::from(lo[1]) + f64::from(hi[1])) / 2.) as f32,
        ((lowz + f64::from(hi[2])) / 2.) as f32,
    );
    let radius = (f64::from(hi[0]) - f64::from(lo[0]))
        .max(f64::from(hi[1]) - f64::from(lo[1]))
        .max(f64::from(hi[2]) - lowz)
        .max(2.)
        * 0.87
        * 1.25;
    let eye = center + glam::Vec3::from_array(direction.map(|v| v as f32)) * ((radius * 2.) as f32);
    let normalize = |v: glam::Vec3| {
        let m = v.abs().max_element();
        let length = if m == 0. {
            0.
        } else {
            let s = v / m;
            m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
        };
        if (length - 1.).abs() <= 1e-5 {
            v
        } else if length.abs() <= 1e-5 {
            glam::Vec3::ZERO
        } else {
            v / length
        }
    };
    let forward = normalize(center - eye);
    let up = if direction[2].abs() < 0.95 {
        glam::Vec3::Z
    } else {
        glam::Vec3::Y
    };
    let side = normalize(forward.cross(up));
    let up = side.cross(forward);
    let mut view = [[0.; 4]; 4];
    view[3][3] = 1.;
    for (r, v) in [side, up, -forward].iter().enumerate() {
        view[0][r] = v.x;
        view[1][r] = v.y;
        view[2][r] = v.z;
        view[3][r] = view[0][r] * (-eye.x) + view[1][r] * (-eye.y) + view[2][r] * (-eye.z);
    }
    let left = (-radius) as f32;
    let right = radius as f32;
    let bottom = left;
    let top = right;
    let near = (radius * 0.5) as f32;
    let far = (radius * 3.5) as f32;
    let mut proj = [[0.; 4]; 4];
    proj[0][0] = 2. / (right - left);
    proj[1][1] = 2. / (top - bottom);
    proj[2][2] = -2. / (far - near);
    proj[3][0] = -(left + right) / (right - left);
    proj[3][1] = -(top + bottom) / (top - bottom);
    proj[3][2] = -(near + far) / (far - near);
    proj[3][3] = 1.;
    let matrix = std::array::from_fn(|i| {
        let c = i / 4;
        let r = i % 4;
        proj[0][r] * view[c][0]
            + proj[1][r] * view[c][1]
            + proj[2][r] * view[c][2]
            + proj[3][r] * view[c][3]
    });
    if !matrix.iter().all(|v: &f32| v.is_finite()) {
        return Err("Shadow light float arithmetic overflow".into());
    }
    Ok(matrix)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct CasterParameters {
    pub matrix: [f32; 16],
    pub clip: [f32; 4],
    pub clip_enabled: u32,
    pub use_texture: u32,
}
pub(super) struct Casters {
    device: Device,
    lifetime: Arc<Lifetime>,
    pub plain_layout: vk::PipelineLayout,
    pub texture_layout: vk::PipelineLayout,
    pub opaque: vk::Pipeline,
    pub transparent: vk::Pipeline,
    pub textured: vk::Pipeline,
    pub fallback: vk::Pipeline,
}
impl Casters {
    pub fn new(
        context: &VulkanContext,
        pass: vk::RenderPass,
        texture_layout: vk::DescriptorSetLayout,
    ) -> Result<Self, String> {
        context.lifetime.check()?;
        let clip_distance = unsafe {
            context
                .instance
                .get_physical_device_features(context.physical_device)
        }
        .shader_clip_distance
            != 0;
        let mut owner = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            plain_layout: vk::PipelineLayout::null(),
            texture_layout: vk::PipelineLayout::null(),
            opaque: vk::Pipeline::null(),
            transparent: vk::Pipeline::null(),
            textured: vk::Pipeline::null(),
            fallback: vk::Pipeline::null(),
        };
        let push = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .size(std::mem::size_of::<CasterParameters>() as u32)];
        owner.plain_layout = unsafe {
            owner.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        owner.texture_layout = unsafe {
            owner.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&[texture_layout])
                    .push_constant_ranges(&push),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        owner.opaque = caster_pipeline(
            &owner.device,
            pass,
            owner.plain_layout,
            48,
            false,
            clip_distance,
            &owner.lifetime,
        )?;
        owner.transparent = caster_pipeline(
            &owner.device,
            pass,
            owner.plain_layout,
            24,
            false,
            clip_distance,
            &owner.lifetime,
        )?;
        owner.fallback = caster_pipeline(
            &owner.device,
            pass,
            owner.plain_layout,
            20,
            false,
            clip_distance,
            &owner.lifetime,
        )?;
        owner.textured = caster_pipeline(
            &owner.device,
            pass,
            owner.texture_layout,
            20,
            true,
            clip_distance,
            &owner.lifetime,
        )?;
        Ok(owner)
    }
}
fn caster_pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    stride: u32,
    textured: bool,
    clip_distance: bool,
    lifetime: &Lifetime,
) -> Result<vk::Pipeline, String> {
    let vertex = ash::util::read_spv(&mut std::io::Cursor::new(if clip_distance {
        if textured {
            include_bytes!("../shaders/shadow_clip.vert.spv").as_slice()
        } else {
            include_bytes!("../shaders/shadow_plain_clip.vert.spv").as_slice()
        }
    } else if textured {
        include_bytes!("../shaders/shadow.vert.spv").as_slice()
    } else {
        include_bytes!("../shaders/shadow_plain.vert.spv").as_slice()
    }))
    .map_err(|e| e.to_string())?;
    let fragment = ash::util::read_spv(&mut std::io::Cursor::new(if textured {
        include_bytes!("../shaders/shadow.frag.spv").as_slice()
    } else {
        include_bytes!("../shaders/shadow_plain.frag.spv").as_slice()
    }))
    .map_err(|e| e.to_string())?;
    let vertex = unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&vertex), None)
    }
    .map_err(|e| device_error(lifetime, e))?;
    let fragment = match unsafe {
        device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&fragment), None)
    } {
        Ok(m) => m,
        Err(e) => {
            unsafe {
                device.destroy_shader_module(vertex, None);
            }
            return Err(device_error(lifetime, e));
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
    let binding = [vk::VertexInputBindingDescription {
        binding: 0,
        stride,
        input_rate: vk::VertexInputRate::VERTEX,
    }];
    let attrs = [
        vk::VertexInputAttributeDescription {
            location: 0,
            binding: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        },
        vk::VertexInputAttributeDescription {
            location: 1,
            binding: 0,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12,
        },
    ];
    let input = vk::PipelineVertexInputStateCreateInfo::default()
        .vertex_binding_descriptions(&binding)
        .vertex_attribute_descriptions(if textured { &attrs } else { &attrs[..1] });
    let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = vk::PipelineViewportStateCreateInfo::default()
        .viewport_count(1)
        .scissor_count(1);
    let raster = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::CLOCKWISE);
    let samples = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let depth = vk::PipelineDepthStencilStateCreateInfo::default()
        .depth_test_enable(true)
        .depth_write_enable(true)
        .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);
    let colors = [vk::PipelineColorBlendAttachmentState::default()
        .blend_enable(false)
        .color_write_mask(vk::ColorComponentFlags::RGBA)];
    let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);
    let dynamic = vk::PipelineDynamicStateCreateInfo::default()
        .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);
    let result = unsafe {
        device.create_graphics_pipelines(
            vk::PipelineCache::null(),
            &[vk::GraphicsPipelineCreateInfo::default()
                .stages(&stages)
                .vertex_input_state(&input)
                .input_assembly_state(&assembly)
                .viewport_state(&viewport)
                .rasterization_state(&raster)
                .multisample_state(&samples)
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
                if p != vk::Pipeline::null() {
                    unsafe {
                        device.destroy_pipeline(p, None);
                    }
                }
            }
            Err(device_error(lifetime, e))
        }
    }
}
impl Drop for Casters {
    fn drop(&mut self) {
        if self.lifetime.uncertain() {
            self.lifetime.retain((
                self.device.clone(),
                self.plain_layout,
                self.texture_layout,
                self.opaque,
                self.transparent,
                self.textured,
                self.fallback,
            ));
            return;
        }
        unsafe {
            for p in [self.opaque, self.transparent, self.textured, self.fallback] {
                if p != vk::Pipeline::null() {
                    self.device.destroy_pipeline(p, None);
                }
            }
            for l in [self.plain_layout, self.texture_layout] {
                if l != vk::PipelineLayout::null() {
                    self.device.destroy_pipeline_layout(l, None);
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct ReceiverParameters {
    matrix: [f32; 16],
    sun_bias: [f32; 4],
    parameters: [f32; 4],
}
#[derive(Clone, PartialEq)]
struct Key {
    revision: [u64; 3],
    sun: [u64; 3],
    clip: Option<[u64; 4]>,
    bounds: [u64; 6],
}
fn rounded_bits(v: f64, precision: usize) -> u64 {
    format!("{v:.precision$}").parse::<f64>().unwrap().to_bits()
}
struct Packet {
    revision: [u64; 3],
    vertices: Option<crate::native_vulkan::resources::BufferId>,
    indices: Option<crate::native_vulkan::resources::BufferId>,
    colored: Option<crate::native_vulkan::resources::BufferId>,
    count: u32,
    runs: Vec<super::OpacityRun>,
    textures: ingetrazo_render::buffer::TextureBuffers,
}
impl Packet {
    fn ids(&self) -> [Option<crate::native_vulkan::resources::BufferId>; 3] {
        [self.vertices, self.indices, self.colored]
    }
}
pub(super) struct Owner {
    device: Device,
    lifetime: Arc<Lifetime>,
    resources: crate::native_vulkan::resources::Resources,
    pub map: Map,
    pub casters: Casters,
    pub layout: vk::PipelineLayout,
    pub set_layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    pub set: vk::DescriptorSet,
    pub surface: vk::Pipeline,
    pub transparent: vk::Pipeline,
    pub ground_pipeline: vk::Pipeline,
    pub paint: [vk::Pipeline; 10],
    pub texture_pipelines: [vk::Pipeline; 16],
    packet: Option<Packet>,
    input: Option<([u64; 3], [f64; 3], [f32; 3], [f32; 3], [f32; 4], u32, f32)>,
    uniform: Option<crate::native_vulkan::resources::BufferId>,
    pub ground: Option<crate::native_vulkan::resources::BufferId>,
    parameters: Option<ReceiverParameters>,
    ground_bounds: Option<[u32; 4]>,
    key: Option<Key>,
    pub dirty: bool,
    pub renders: u64,
    pub recorded_serial: u64,
    pub parameter_uploads: u64,
}
type Input = ([u64; 3], [f64; 3], [f32; 3], [f32; 3], [f32; 4], u32, f32);
/// Private staging token: no descriptor or published owner changes until commit.
pub(super) struct Prepared {
    next: [Option<crate::native_vulkan::resources::BufferId>; 5],
    packet: Option<Packet>,
    retirement: Option<crate::native_vulkan::resources::CompletedRetirement>,
    update: Option<(ReceiverParameters, [u32; 4], Key, Input)>,
}
impl Owner {
    pub fn new(
        context: &VulkanContext,
        budget: Arc<VulkanBudget>,
        render_pass: vk::RenderPass,
        samples: vk::SampleCountFlags,
        asset_layout: vk::DescriptorSetLayout,
        clip_distance: bool,
        readback: bool,
    ) -> Result<Self, String> {
        let map = Map::new(context, budget.clone(), readback)?;
        let casters = Casters::new(context, map.render_pass, asset_layout)?;
        let mut owner = Self {
            device: context.device.clone(),
            lifetime: context.lifetime.clone(),
            resources: crate::native_vulkan::resources::Resources::new(context, budget),
            map,
            casters,
            layout: vk::PipelineLayout::null(),
            set_layout: vk::DescriptorSetLayout::null(),
            pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            surface: vk::Pipeline::null(),
            transparent: vk::Pipeline::null(),
            ground_pipeline: vk::Pipeline::null(),
            paint: [vk::Pipeline::null(); 10],
            texture_pipelines: [vk::Pipeline::null(); 16],
            packet: None,
            input: None,
            uniform: None,
            ground: None,
            parameters: None,
            ground_bounds: None,
            key: None,
            dirty: true,
            renders: 0,
            recorded_serial: 0,
            parameter_uploads: 0,
        };
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
            vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        ];
        owner.set_layout = unsafe {
            owner.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        let layouts = [asset_layout, owner.set_layout];
        let push = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
            .size(128)];
        owner.layout = unsafe {
            owner.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default()
                    .set_layouts(&layouts)
                    .push_constant_ranges(&push),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        let sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1,
            },
        ];
        owner.pool = unsafe {
            owner.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&sizes),
                None,
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?;
        owner.set = unsafe {
            owner.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(owner.pool)
                    .set_layouts(&[owner.set_layout]),
            )
        }
        .map_err(|e| device_error(&owner.lifetime, e))?[0];
        owner.surface = super::make_pipeline_source(
            &owner.device,
            render_pass,
            owner.layout,
            super::WorldPass::Surface,
            samples,
            false,
            None,
            true,
        )?;
        owner.transparent = super::make_pipeline_source(
            &owner.device,
            render_pass,
            owner.layout,
            super::WorldPass::Transparent,
            samples,
            false,
            None,
            true,
        )?;
        owner.ground_pipeline = super::make_pipeline_source(
            &owner.device,
            render_pass,
            owner.layout,
            super::WorldPass::ShadowGround,
            samples,
            false,
            None,
            true,
        )?;
        for (i, pass) in [
            super::WorldPass::FrontSurface,
            super::WorldPass::TransparentFront,
            super::WorldPass::TransparentBack,
            super::WorldPass::OpaqueBack,
            super::WorldPass::DefaultBack,
            super::WorldPass::XRaySurface,
            super::WorldPass::XRayFront,
            super::WorldPass::Edge,
            super::WorldPass::DepthOnly,
            super::WorldPass::HiddenEdge,
        ]
        .into_iter()
        .enumerate()
        {
            owner.paint[i] = super::make_pipeline_source(
                &owner.device,
                render_pass,
                owner.layout,
                pass,
                samples,
                clip_distance,
                None,
                true,
            )?;
        }
        for i in 0..16 {
            owner.texture_pipelines[i] = crate::native_vulkan::textures::shadow_pipeline(
                &owner.device,
                render_pass,
                owner.layout,
                samples,
                i,
            )?;
        }
        Ok(owner)
    }
    pub fn stage(
        &mut self,
        context: &VulkanContext,
        direction: [f64; 3],
        darkness: f32,
        lo: [f32; 3],
        hi: [f32; 3],
        clip: [f32; 4],
        clip_enabled: u32,
        revision: [u64; 3],
        fence: vk::Fence,
        completed: u64,
        scene: Option<&ingetrazo_core::scene::Scene>,
    ) -> Result<Prepared, String> {
        if !darkness.is_finite()
            || !(0.0..=1.0).contains(&darkness)
            || !clip.iter().all(|v| v.is_finite())
        {
            return Err("Shadow receiver parameter admission".into());
        }
        self.resources.check_submission()?;
        if self.recorded_serial > completed {
            self.dirty = true;
        }
        let input = (revision, direction, lo, hi, clip, clip_enabled, darkness);
        let packet_needed =
            scene.is_some() && self.packet.as_ref().is_none_or(|p| p.revision != revision);
        if self.input == Some(input) && !packet_needed {
            return Ok(Prepared {
                next: [None; 5],
                packet: None,
                retirement: None,
                update: None,
            });
        }
        let matrix = light_matrix(direction, lo, hi)?;
        let params = ReceiverParameters {
            matrix,
            sun_bias: [
                direction[0] as f32,
                direction[1] as f32,
                direction[2] as f32,
                0.0006,
            ],
            parameters: [darkness, 0., 0., 0.],
        };
        let bounds = [
            lo[0].to_bits(),
            lo[1].to_bits(),
            hi[0].to_bits(),
            hi[1].to_bits(),
        ];
        let key = Key {
            revision,
            sun: direction.map(|v| rounded_bits(v, 4)),
            clip: if clip_enabled == 0 {
                None
            } else {
                Some(clip.map(|v| rounded_bits(f64::from(v), 4)))
            },
            bounds: [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]]
                .map(|v| rounded_bits(f64::from(v), 2)),
        };
        let mut old = [None; 5];
        let mut next = [None; 5];
        let mut count = 0;
        let mut packet = None;
        let prepare = (|| -> Result<(), String> {
            if self.parameters != Some(params) {
                next[0] = Some(self.resources.upload(
                    context,
                    bytemuck::bytes_of(&params),
                    vk::BufferUsageFlags::UNIFORM_BUFFER,
                    false,
                )?);
                old[count] = self.uniform;
                count += 1;
            }
            if self.ground_bounds != Some(bounds) {
                let cx = (f64::from(lo[0]) + f64::from(hi[0])) / 2.;
                let cy = (f64::from(lo[1]) + f64::from(hi[1])) / 2.;
                let radius = (f64::from(hi[0]) - f64::from(lo[0]))
                    .max(f64::from(hi[1]) - f64::from(lo[1]))
                    .max(20.)
                    * 1.5;
                let corners = [
                    [cx - radius, cy - radius, -0.005],
                    [cx + radius, cy - radius, -0.005],
                    [cx + radius, cy + radius, -0.005],
                    [cx - radius, cy + radius, -0.005],
                ];
                let vertices = [0, 1, 2, 0, 2, 3].map(|i| ingetrazo_render::buffer::Vertex {
                    position: corners[i].map(|v| v as f32),
                    ..Default::default()
                });
                if !vertices.iter().flat_map(|v| v.position).all(f32::is_finite) {
                    return Err("Shadow ground float arithmetic admission".into());
                }
                next[1] = Some(self.resources.upload(
                    context,
                    bytemuck::cast_slice(&vertices),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    false,
                )?);
                old[count] = self.ground;
            }
            if packet_needed {
                let mut style = ingetrazo_render::style::StyleConfig::default();
                style.show_textures = true;
                let data = ingetrazo_render::pipeline::build_world_surfaces(
                    scene.unwrap(),
                    &ingetrazo_core::camera::OrbitCamera::default(),
                    &style,
                );
                if data
                    .opaque_mesh
                    .vertices
                    .iter()
                    .any(|v| !v.position.iter().all(|x| x.is_finite()))
                    || data
                        .opaque_mesh
                        .indices
                        .iter()
                        .any(|i| *i as usize >= data.opaque_mesh.vertices.len())
                    || data
                        .transparent_mesh
                        .vertices
                        .iter()
                        .any(|v| !v.position.iter().all(|x| x.is_finite()))
                {
                    return Err("Shadow packet finite/index admission".into());
                }
                let count: u32 = data
                    .opaque_mesh
                    .indices
                    .len()
                    .try_into()
                    .map_err(|_| "Shadow packet index admission")?;
                let (colored, runs, _) = super::pack_transparent_with_textures(
                    &data.transparent_mesh,
                    &data.opaque_mesh,
                    &data.textured_meshes,
                )?;
                if count > 0 {
                    next[2] = Some(self.resources.upload(
                        context,
                        bytemuck::cast_slice(&data.opaque_mesh.vertices),
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        false,
                    )?);
                    next[3] = Some(self.resources.upload(
                        context,
                        bytemuck::cast_slice(&data.opaque_mesh.indices),
                        vk::BufferUsageFlags::INDEX_BUFFER,
                        false,
                    )?);
                }
                if !colored.is_empty() {
                    next[4] = Some(self.resources.upload(
                        context,
                        bytemuck::cast_slice(&colored),
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        false,
                    )?);
                }
                if let Some(previous) = &self.packet {
                    old[2..].copy_from_slice(&previous.ids());
                }
                packet = Some(Packet {
                    revision,
                    vertices: next[2],
                    indices: next[3],
                    colored: next[4],
                    count,
                    runs,
                    textures: data.textured_meshes,
                });
            }
            Ok(())
        })();
        if let Err(error) = prepare {
            for id in next.into_iter().flatten() {
                self.resources.retire(id)?;
            }
            self.resources.collect(fence, completed)?;
            return Err(error);
        }
        let fallback = next[0]
            .or(next[1])
            .or(self.uniform)
            .or(self.ground)
            .ok_or("Shadow publication has no admitted buffer")?;
        let old_count = old.iter().flatten().count();
        let next_count = next.iter().flatten().count();
        let old_values: [_; 5] =
            std::array::from_fn(|i| old.iter().flatten().nth(i).copied().unwrap_or(fallback));
        let next_values: [_; 5] =
            std::array::from_fn(|i| next.iter().flatten().nth(i).copied().unwrap_or(fallback));
        let retirement = match self.resources.publication_preflight(
            context,
            fence,
            completed,
            &old_values[..old_count],
            &next_values[..next_count],
        ) {
            Ok(p) => p,
            Err(error) => {
                for id in next.into_iter().flatten() {
                    self.resources.retire(id)?;
                }
                self.resources.collect(fence, completed)?;
                return Err(error);
            }
        };
        Ok(Prepared {
            next,
            packet,
            retirement: Some(retirement),
            update: Some((params, bounds, key, input)),
        })
    }
    pub fn abort(
        &mut self,
        prepared: Prepared,
        fence: vk::Fence,
        completed: u64,
    ) -> Result<(), String> {
        for id in prepared.next.into_iter().flatten() {
            self.resources.retire(id)?;
        }
        self.resources.collect(fence, completed).map(|_| ())
    }
    pub fn commit(&mut self, prepared: Prepared) {
        let Some((params, bounds, key, input)) = prepared.update else {
            return;
        };
        if let Some(id) = prepared.next[0] {
            let image = [vk::DescriptorImageInfo::default()
                .sampler(self.map.sampler)
                .image_view(self.map.view())
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
            let buffer = [vk::DescriptorBufferInfo::default()
                .buffer(self.resources.buffer(id).unwrap())
                .range(std::mem::size_of::<ReceiverParameters>() as u64)];
            unsafe {
                self.device.update_descriptor_sets(
                    &[
                        vk::WriteDescriptorSet::default()
                            .dst_set(self.set)
                            .dst_binding(0)
                            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                            .image_info(&image),
                        vk::WriteDescriptorSet::default()
                            .dst_set(self.set)
                            .dst_binding(1)
                            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                            .buffer_info(&buffer),
                    ],
                    &[],
                );
            }
            self.uniform = Some(id);
            self.parameter_uploads += 1;
        }
        if let Some(packet) = prepared.packet {
            self.packet = Some(packet);
        }
        if let Some(id) = prepared.next[1] {
            self.ground = Some(id);
        }
        if let Some(retirement) = prepared.retirement {
            self.resources.retire_completed(retirement);
        }
        self.dirty |= self.key.as_ref() != Some(&key);
        self.key = Some(key);
        self.parameters = Some(params);
        self.ground_bounds = Some(bounds);
        self.input = Some(input);
    }
    pub fn staged_textures<'a>(
        &'a self,
        prepared: &'a Prepared,
    ) -> Option<&'a ingetrazo_render::buffer::TextureBuffers> {
        prepared
            .packet
            .as_ref()
            .or(self.packet.as_ref())
            .map(|p| &p.textures)
    }
    pub fn preflight_packet(&mut self, serial: u64) -> Result<(), String> {
        let packet = self
            .packet
            .as_ref()
            .ok_or("Independent shadow caster packet absent")?;
        for id in packet.ids().into_iter().flatten() {
            self.resources.mark_used(id, serial)?;
        }
        Ok(())
    }
    pub fn record_packet(&mut self, command: vk::CommandBuffer, serial: u64) -> Result<(), String> {
        let packet = self
            .packet
            .as_ref()
            .ok_or("Independent shadow caster packet absent")?;
        for id in packet.ids().into_iter().flatten() {
            self.resources.mark_used(id, serial)?;
        }
        unsafe {
            if let (Some(v), Some(i)) = (packet.vertices, packet.indices) {
                self.device.cmd_bind_pipeline(
                    command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.casters.opaque,
                );
                self.device.cmd_bind_vertex_buffers(
                    command,
                    0,
                    &[self.resources.buffer(v).unwrap()],
                    &[0],
                );
                self.device.cmd_bind_index_buffer(
                    command,
                    self.resources.buffer(i).unwrap(),
                    0,
                    vk::IndexType::UINT32,
                );
                self.device
                    .cmd_draw_indexed(command, packet.count, 1, 0, 0, 0);
            }
            if let Some(v) = packet.colored {
                self.device.cmd_bind_pipeline(
                    command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.casters.transparent,
                );
                self.device.cmd_bind_vertex_buffers(
                    command,
                    0,
                    &[self.resources.buffer(v).unwrap()],
                    &[0],
                );
                for run in packet
                    .runs
                    .iter()
                    .filter(|r| r.kind <= 1 && r.opacity >= 0.7)
                {
                    self.device.cmd_draw(command, run.count, 1, run.first, 0);
                }
            }
        }
        Ok(())
    }
    pub fn readback_for_probe(
        &mut self,
        context: &VulkanContext,
        fence: vk::Fence,
        completed: u64,
    ) -> Result<Vec<u8>, String> {
        if !self.map.readback || self.renders == 0 || self.recorded_serial > completed {
            return Err("Shadow diagnostic readback lacks admitted completed map".into());
        }
        let proof = self
            .resources
            .publication_preflight(context, fence, completed, &[], &[])?;
        self.resources.retire_completed(proof);
        let mut rgba = self.resources.readback_image(
            context,
            self.map.color.as_ref().unwrap().handle,
            vk::Extent2D {
                width: MAP_SIZE,
                height: MAP_SIZE,
            },
        )?;
        let row = MAP_SIZE as usize * 4;
        for y in 0..MAP_SIZE as usize / 2 {
            let opposite = MAP_SIZE as usize - 1 - y;
            let (top, bottom) = rgba.split_at_mut(opposite * row);
            top[y * row..(y + 1) * row].swap_with_slice(&mut bottom[..row]);
        }
        Ok(rgba)
    }
    pub fn light_matrix_for_probe(&self) -> Option<[f32; 16]> {
        self.parameters.map(|p| p.matrix)
    }
    pub fn caster_parameters(&self, clip: [f32; 4], clip_enabled: u32) -> CasterParameters {
        CasterParameters {
            matrix: self.parameters.unwrap().matrix,
            clip,
            clip_enabled,
            use_texture: 0,
        }
    }
    pub fn bind(&mut self, command: vk::CommandBuffer, serial: u64) -> Result<(), String> {
        let id = self.uniform.ok_or("Shadow parameters absent")?;
        self.resources.mark_used(id, serial)?;
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::GRAPHICS,
                self.layout,
                1,
                &[self.set],
                &[],
            );
        }
        Ok(())
    }
    pub fn record_ground(&mut self, command: vk::CommandBuffer, serial: u64) -> Result<(), String> {
        let id = self.ground.ok_or("Shadow ground absent")?;
        self.resources.mark_used(id, serial)?;
        unsafe {
            self.device.cmd_bind_pipeline(
                command,
                vk::PipelineBindPoint::GRAPHICS,
                self.ground_pipeline,
            );
            self.device.cmd_bind_vertex_buffers(
                command,
                0,
                &[self.resources.buffer(id).unwrap()],
                &[0],
            );
            self.device.cmd_draw(command, 6, 1, 0, 0);
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if !self.resources.drain_graphics() {
            self.lifetime.retain((
                self.device.clone(),
                self.layout,
                self.set_layout,
                self.pool,
                self.surface,
                self.transparent,
                self.ground_pipeline,
                self.paint,
                self.texture_pipelines,
            ));
            return;
        }
        unsafe {
            for p in self.paint.into_iter().chain(self.texture_pipelines).chain([
                self.surface,
                self.transparent,
                self.ground_pipeline,
            ]) {
                if p != vk::Pipeline::null() {
                    self.device.destroy_pipeline(p, None);
                }
            }
            if self.pool != vk::DescriptorPool::null() {
                self.device.destroy_descriptor_pool(self.pool, None);
            }
            if self.layout != vk::PipelineLayout::null() {
                self.device.destroy_pipeline_layout(self.layout, None);
            }
            if self.set_layout != vk::DescriptorSetLayout::null() {
                self.device
                    .destroy_descriptor_set_layout(self.set_layout, None);
            }
        }
    }
}

/// Source Scene.bounds inventory: loose visible entities and every visible
/// placement's welded vertices, including orphaned group vertices.
pub(super) fn scene_bounds(scene: &ingetrazo_core::scene::Scene) -> Option<([f32; 3], [f32; 3])> {
    let mut lo = glam::DVec3::splat(f64::INFINITY);
    let mut hi = glam::DVec3::splat(f64::NEG_INFINITY);
    fn absorb(p: glam::DVec3, lo: &mut glam::DVec3, hi: &mut glam::DVec3) {
        *lo = lo.min(p);
        *hi = hi.max(p);
    }
    for edge in &scene.mesh.edges {
        if scene.is_layer_visible(edge.layer.as_deref()) {
            for id in [edge.v0, edge.v1] {
                absorb(scene.mesh.vertices[id].position, &mut lo, &mut hi);
            }
        }
    }
    for (id, face) in scene.mesh.faces.iter().enumerate() {
        if scene.mesh_face_visible(&scene.mesh, id) {
            for &id in &face.vertices {
                absorb(scene.mesh.vertices[id].position, &mut lo, &mut hi);
            }
        }
    }
    fn groups(
        scene: &ingetrazo_core::scene::Scene,
        group: &ingetrazo_core::groups::Group,
        parent: glam::DMat4,
        lo: &mut glam::DVec3,
        hi: &mut glam::DVec3,
    ) {
        let world = parent * group.transform;
        if !group.hidden && !group.billboard && scene.is_layer_visible(group.layer.as_deref()) {
            for v in &group.mesh.vertices {
                absorb(world.transform_point3(v.position), lo, hi);
            }
        }
        for child in &group.children {
            groups(scene, child, world, lo, hi);
        }
    }
    for group in &scene.groups {
        if !group.hidden && !group.billboard && scene.is_layer_visible(group.layer.as_deref()) {
            groups(scene, group, glam::DMat4::IDENTITY, &mut lo, &mut hi);
        }
    }
    if lo.is_finite() && hi.is_finite() {
        Some((lo.as_vec3().to_array(), hi.as_vec3().to_array()))
    } else {
        None
    }
}
