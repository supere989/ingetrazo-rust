//! Inline precise encoded CAD paint; immutable buffers uploaded only on cache misses.
use super::{
    budget::VulkanBudget,
    resources::{BufferId, Resources},
    VulkanContext,
};
use ash::{vk, Device};
use egui_ash_renderer::{RendererResult, VulkanPaint, VulkanPaintCallback};
use ingetrazo_render::cad2d::{Batch, Geometry, Pose};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
#[derive(Default)]
struct Counters {
    callbacks: AtomicU64,
    binds: AtomicU64,
    draws: AtomicU64,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub callbacks: u64,
    pub binds: u64,
    pub draws: u64,
    pub uploads: u64,
    pub upload_bytes: u64,
    pub cache_hits: u64,
    pub cached_bytes: u64,
    pub rolled_back_buffers: u64,
}
#[derive(Clone)]
struct Ready {
    id: u64,
    vertex: vk::Buffer,
    index: vk::Buffer,
    batches: Arc<[Batch]>,
    pose: Pose,
}
struct State {
    geometry: Arc<Geometry>,
    batches: Arc<[Batch]>,
    pose: Pose,
    ready: Option<Ready>,
}
struct Pipeline {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}
struct NativePaint {
    device: vk::Device,
    state: Mutex<State>,
    pipeline: Arc<Mutex<Pipeline>>,
    counters: Arc<Counters>,
}
/// Clone and reuse this handle. Shape construction clones an Arc, never geometry.
#[derive(Clone)]
pub struct Handle {
    inner: Arc<NativePaint>,
    callback: Arc<VulkanPaintCallback>,
}
impl Handle {
    pub fn shape(&self, rect: egui::Rect) -> egui::Shape {
        egui::PaintCallback {
            rect,
            callback: self.callback.clone(),
        }
        .into()
    }
    pub fn update(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<(), String> {
        geometry.validate_batches(&batches)?;
        pose.validate()?;
        let mut s = self
            .inner
            .state
            .lock()
            .map_err(|_| "CAD2D state poisoned")?;
        s.geometry = geometry;
        s.batches = batches;
        s.pose = pose;
        Ok(())
    }
}
impl VulkanPaint for NativePaint {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn paint(
        &self,
        info: &egui::PaintCallbackInfo,
        device: &Device,
        command: vk::CommandBuffer,
    ) -> RendererResult<()> {
        if device.handle() != self.device {
            return Err(egui_ash_renderer::RendererError::Init(
                "CAD2D callback device mismatch".into(),
            ));
        }
        let s = self.state.lock().unwrap();
        let Some(r) = &s.ready else { return Ok(()) };
        let p = self.pipeline.lock().unwrap();
        if p.pipeline == vk::Pipeline::null() {
            return Err(egui_ash_renderer::RendererError::Init(
                "CAD2D renderer was retired".into(),
            ));
        }
        let clip = info.clip_rect.intersect(info.viewport);
        let scale = info.pixels_per_point;
        let min = (clip.min.to_vec2() * scale).floor().max(egui::Vec2::ZERO);
        let max = (clip.max.to_vec2() * scale).ceil().min(egui::vec2(
            info.screen_size_px[0] as f32,
            info.screen_size_px[1] as f32,
        ));
        if max.x <= min.x || max.y <= min.y {
            return Ok(());
        }
        self.counters.callbacks.fetch_add(1, Ordering::Relaxed);
        self.counters.binds.fetch_add(1, Ordering::Relaxed);
        let push = [
            r.pose.scale[0],
            r.pose.scale[1],
            r.pose.translation[0],
            r.pose.translation[1],
            info.screen_size_px[0] as f32 / scale,
            info.screen_size_px[1] as f32 / scale,
            0.,
            0.,
        ];
        unsafe {
            device.cmd_bind_pipeline(command, vk::PipelineBindPoint::GRAPHICS, p.pipeline);
            device.cmd_set_viewport(
                command,
                0,
                &[vk::Viewport {
                    x: 0.,
                    y: 0.,
                    width: info.screen_size_px[0] as f32,
                    height: info.screen_size_px[1] as f32,
                    min_depth: 0.,
                    max_depth: 1.,
                }],
            );
            device.cmd_set_scissor(
                command,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D {
                        x: min.x as i32,
                        y: min.y as i32,
                    },
                    extent: vk::Extent2D {
                        width: (max.x - min.x) as u32,
                        height: (max.y - min.y) as u32,
                    },
                }],
            );
            device.cmd_bind_vertex_buffers(command, 0, &[r.vertex], &[0]);
            device.cmd_bind_index_buffer(command, r.index, 0, vk::IndexType::UINT32);
            device.cmd_push_constants(
                command,
                p.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
            for batch in r.batches.iter().filter(|b| b.count > 0) {
                device.cmd_push_constants(
                    command,
                    p.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    32,
                    bytemuck::cast_slice(&batch.color.0),
                );
                device.cmd_draw_indexed(command, batch.count, 1, batch.first, 0, 0);
                self.counters.draws.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(())
    }
}
struct Cached {
    vertex: BufferId,
    index: BufferId,
    bytes: u64,
}
pub struct Renderer {
    device: Device,
    budget: Arc<VulkanBudget>,
    pressure_after_vertex: bool,
    rolled_back_buffers: u64,
    pipeline: Arc<Mutex<Pipeline>>,
    resources: Resources,
    cache: HashMap<u64, Cached>,
    active: HashSet<u64>,
    pending: HashMap<u64, Cached>,
    stale: Vec<u64>,
    counters: Arc<Counters>,
    uploads: u64,
    upload_bytes: u64,
    cache_hits: u64,
}
impl Renderer {
    pub fn new(
        context: &VulkanContext,
        budget: Arc<VulkanBudget>,
        pass: vk::RenderPass,
    ) -> Result<Self, String> {
        let ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 48,
        }];
        let layout = unsafe {
            context.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&ranges),
                None,
            )
        }
        .map_err(|e| e.to_string())?;
        let pipeline = match make_pipeline(&context.device, pass, layout) {
            Ok(p) => p,
            Err(e) => {
                unsafe { context.device.destroy_pipeline_layout(layout, None) };
                return Err(e);
            }
        };
        Ok(Self {
            device: context.device.clone(),
            budget: budget.clone(),
            pressure_after_vertex: false,
            rolled_back_buffers: 0,
            pipeline: Arc::new(Mutex::new(Pipeline { pipeline, layout })),
            resources: Resources::new(context, budget),
            cache: HashMap::new(),
            active: HashSet::new(),
            pending: HashMap::new(),
            stale: Vec::new(),
            counters: Default::default(),
            uploads: 0,
            upload_bytes: 0,
            cache_hits: 0,
        })
    }
    pub fn set_render_pass(&mut self,pass:vk::RenderPass)->Result<(),String>{
        let mut p=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
        let replacement=make_pipeline(&self.device,pass,p.layout)?;
        unsafe{self.device.destroy_pipeline(p.pipeline,None)};
        p.pipeline=replacement;Ok(())
    }
    pub fn handle(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<Handle, String> {
        geometry.validate_batches(&batches)?;
        pose.validate()?;
        let inner = Arc::new(NativePaint {
            device: self.device.handle(),
            state: Mutex::new(State {
                geometry,
                batches,
                pose,
                ready: None,
            }),
            pipeline: self.pipeline.clone(),
            counters: self.counters.clone(),
        });
        let callback = Arc::new(VulkanPaintCallback(inner.clone()));
        Ok(Handle { inner, callback })
    }
    /// Previous frame fence has completed. Prepare every requested buffer before
    /// installing any new drawable. On admission failure old ready states survive.
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        primitives: &[egui::ClippedPrimitive],
        fence: vk::Fence,
        completed: u64,
        next: u64,
    ) -> Result<(), String> {
        self.resources.collect(fence, completed)?;
        self.active.clear();
        self.pending.clear();
        let result = (|| {
            for primitive in primitives {
                let egui::epaint::Primitive::Callback(c) = &primitive.primitive else {
                    continue;
                };
                let Some(callback) = c.callback.downcast_ref::<VulkanPaintCallback>() else {
                    continue;
                };
                let Some(paint) = callback.0.as_any().downcast_ref::<NativePaint>() else {
                    continue;
                };
                if !Arc::ptr_eq(&paint.pipeline, &self.pipeline) {
                    return Err("CAD2D callback belongs to a different Vulkan host".into());
                }
                let s = paint.state.lock().map_err(|_| "CAD2D state poisoned")?;
                s.geometry.validate_batches(&s.batches)?;
                s.pose.validate()?;
                if let Some(r) = &s.ready {
                    self.active.insert(r.id);
                }
                let id = s.geometry.id();
                self.active.insert(id);
                if self.cache.contains_key(&id) || self.pending.contains_key(&id) {
                    self.cache_hits += 1;
                    continue;
                }
                if s.geometry.indices().is_empty() {
                    continue;
                }
                let vertex = self.resources.upload(
                    context,
                    bytemuck::cast_slice(s.geometry.vertices()),
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                    false,
                )?;
                // Engineering-only pressure is a real ledger admission cap after a real
                // successful vertex allocation, never a fabricated Vulkan allocation error.
                let restore = if self.pressure_after_vertex {
                    self.pressure_after_vertex = false;
                    let mut ledger = self.budget.ledger.lock().unwrap();
                    let previous = ledger.total_limit;
                    ledger.total_limit = Some(ledger.total_committed());
                    Some(PressureRestore {
                        budget: self.budget.clone(),
                        previous,
                    })
                } else {
                    None
                };
                let result = self.resources.upload(
                    context,
                    bytemuck::cast_slice(s.geometry.indices()),
                    vk::BufferUsageFlags::INDEX_BUFFER,
                    false,
                );
                drop(restore);
                let index = match result {
                    Ok(i) => i,
                    Err(e) => {
                        self.resources.retire(vertex)?;
                        self.rolled_back_buffers += 1;
                        return Err(e);
                    }
                };
                self.pending.insert(
                    id,
                    Cached {
                        vertex,
                        index,
                        bytes: s.geometry.logical_bytes(),
                    },
                );
            }
            Ok(())
        })();
        if let Err(error) = result {
            for (_, c) in self.pending.drain() {
                self.resources.retire(c.vertex)?;
                self.resources.retire(c.index)?;
                self.rolled_back_buffers += 2;
            }
            self.resources.collect(fence, completed)?;
            return Err(error);
        }
        for (id, c) in self.pending.drain() {
            self.uploads += 1;
            self.upload_bytes += c.bytes;
            self.cache.insert(id, c);
        }
        for primitive in primitives {
            let egui::epaint::Primitive::Callback(c) = &primitive.primitive else {
                continue;
            };
            let Some(callback) = c.callback.downcast_ref::<VulkanPaintCallback>() else {
                continue;
            };
            let Some(paint) = callback.0.as_any().downcast_ref::<NativePaint>() else {
                continue;
            };
            let mut s = paint.state.lock().unwrap();
            let id = s.geometry.id();
            if let Some(c) = self.cache.get(&id) {
                self.resources.mark_used(c.vertex, next)?;
                self.resources.mark_used(c.index, next)?;
                s.ready = Some(Ready {
                    id,
                    vertex: self.resources.buffer(c.vertex).unwrap(),
                    index: self.resources.buffer(c.index).unwrap(),
                    batches: s.batches.clone(),
                    pose: s.pose,
                });
            } else {
                s.ready = None;
            }
        }
        // Inactive geometry is fence-retired, not retained as an unbounded cache.
        self.stale.clear();
        self.stale.extend(
            self.cache
                .keys()
                .filter(|id| !self.active.contains(id))
                .copied(),
        );
        for id in self.stale.drain(..) {
            let c = self.cache.remove(&id).unwrap();
            self.resources.retire(c.vertex)?;
            self.resources.retire(c.index)?;
        }
        self.resources.collect(fence, completed)?;
        Ok(())
    }
    /// Isolated acceptance hook: actual allocation, real budget denial, ledger
    /// rollback, prior drawable identity and successful retry through prepare().
    pub fn verify_pressure_rollback_for_probe(
        &mut self,
        context: &VulkanContext,
        fence: vk::Fence,
        completed: u64,
    ) -> Result<(), String> {
        use ingetrazo_render::cad2d::{EncodedRgba, Vertex};
        let geometry = Geometry::new(
            vec![
                Vertex {
                    position: [0., 0.],
                    coverage: 1.,
                },
                Vertex {
                    position: [1., 0.],
                    coverage: 1.,
                },
                Vertex {
                    position: [0., 1.],
                    coverage: 1.,
                },
            ],
            vec![0, 1, 2],
        )?;
        let batches: Arc<[Batch]> = vec![Batch {
            first: 0,
            count: 3,
            color: EncodedRgba::from_bytes([37, 93, 157, 128], 1.)?,
        }]
        .into();
        let handle = self.handle(geometry.clone(), batches.clone(), Pose::default())?;
        let primitive = egui::ClippedPrimitive {
            clip_rect: egui::Rect::EVERYTHING,
            primitive: egui::epaint::Primitive::Callback(egui::PaintCallback {
                rect: egui::Rect::EVERYTHING,
                callback: handle.callback.clone(),
            }),
        };
        self.prepare(
            context,
            std::slice::from_ref(&primitive),
            fence,
            completed,
            completed,
        )?;
        let before = self.budget.ledger.lock().unwrap().total_committed();
        let old = handle.inner.state.lock().unwrap().ready.clone().unwrap();
        let original_limit = self.budget.ledger.lock().unwrap().total_limit;
        let replacement = Geometry::new(geometry.vertices().to_vec(), geometry.indices().to_vec())?;
        handle.update(replacement, batches.clone(), Pose::default())?;
        self.pressure_after_vertex = true;
        let result = self.prepare(
            context,
            std::slice::from_ref(&primitive),
            fence,
            completed,
            completed,
        );
        if result.is_ok() {
            return Err("CAD2D real pressure unexpectedly admitted index allocation".into());
        }
        let ready = handle.inner.state.lock().unwrap().ready.clone().unwrap();
        let ledger = self.budget.ledger.lock().unwrap();
        if ready.id != old.id
            || ready.vertex != old.vertex
            || ready.index != old.index
            || ledger.total_committed() != before
            || ledger.total_limit != original_limit
            || ledger.heaps.iter().any(|h| h.reserved != 0)
        {
            return Err("CAD2D pressure lost old drawable or leaked GPU ledger".into());
        }
        drop(ledger);
        self.prepare(
            context,
            std::slice::from_ref(&primitive),
            fence,
            completed,
            completed,
        )?;
        if handle
            .inner
            .state
            .lock()
            .unwrap()
            .ready
            .as_ref()
            .unwrap()
            .id
            == old.id
        {
            return Err("CAD2D retry did not install replacement".into());
        }
        self.prepare(context, &[], fence, completed, completed)?;
        eprintln!("PASS CAD2D actual vertex allocation/index budget denial/prior drawable/rollback/retry: {:?}",self.stats());
        Ok(())
    }
    pub fn stats(&self) -> Stats {
        Stats {
            callbacks: self.counters.callbacks.load(Ordering::Relaxed),
            binds: self.counters.binds.load(Ordering::Relaxed),
            draws: self.counters.draws.load(Ordering::Relaxed),
            uploads: self.uploads,
            upload_bytes: self.upload_bytes,
            cache_hits: self.cache_hits,
            cached_bytes: self.cache.values().map(|c| c.bytes).sum(),
            rolled_back_buffers: self.rolled_back_buffers,
        }
    }
}
struct PressureRestore {
    budget: Arc<VulkanBudget>,
    previous: Option<u64>,
}
impl Drop for PressureRestore {
    fn drop(&mut self) {
        self.budget.ledger.lock().unwrap().total_limit = self.previous;
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = unsafe { self.device.device_wait_idle() };
        let mut p = self.pipeline.lock().unwrap();
        unsafe {
            self.device.destroy_pipeline(p.pipeline, None);
            self.device.destroy_pipeline_layout(p.layout, None)
        }
        p.pipeline = vk::Pipeline::null();
        p.layout = vk::PipelineLayout::null();
    }
}
fn make_pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline, String> {
    let mut modules = Vec::new();
    let result = (|| {
        for bytes in [
            include_bytes!("shaders/cad2d.vert.spv").as_slice(),
            include_bytes!("shaders/cad2d.frag.spv").as_slice(),
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
                .map_err(|e| e.to_string())?,
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
        let bindings = [vk::VertexInputBindingDescription {
            binding: 0,
            stride: 12,
            input_rate: vk::VertexInputRate::VERTEX,
        }];
        let attrs = [
            vk::VertexInputAttributeDescription {
                location: 0,
                binding: 0,
                format: vk::Format::R32G32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                location: 1,
                binding: 0,
                format: vk::Format::R32_SFLOAT,
                offset: 8,
            },
        ];
        let input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&bindings)
            .vertex_attribute_descriptions(&attrs);
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
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
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
            Ok(p) => Ok(p[0]),
            Err((partial, e)) => {
                for p in partial {
                    unsafe { device.destroy_pipeline(p, None) }
                }
                Err(e.to_string())
            }
        }
    })();
    for m in modules {
        unsafe { device.destroy_shader_module(m, None) }
    }
    result
}
