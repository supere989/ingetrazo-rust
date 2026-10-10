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
    premultiplied: Arc<[[f32;4]]>,
    pose: Pose,
    ordered_draws:Option<Arc<[std::ops::Range<u32>]>>,
}
struct State {
    geometry: Arc<Geometry>,
    batches: Arc<[Batch]>,
    premultiplied: Arc<[[f32;4]]>,
    pose: Pose,
    ready: Option<Ready>,
}
struct Pipeline {
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    integer:bool,
    descriptor_layout:vk::DescriptorSetLayout,
    descriptor_pool:vk::DescriptorPool,
    descriptor_sets:Vec<vk::DescriptorSet>,
    active_set:Option<vk::DescriptorSet>,
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
        if !Arc::ptr_eq(&s.batches,&batches){s.premultiplied=prepare_palette(&batches)?;}
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
        let screen=[info.screen_size_px[0] as f32 / scale,info.screen_size_px[1] as f32 / scale];
        if screen.iter().any(|v|!v.is_finite()||*v<=0.){return Err(egui_ash_renderer::RendererError::Init("CAD2D logical screen is outside finite shader domain".into()));}
        let push = [
            r.pose.scale[0],
            r.pose.scale[1],
            r.pose.translation[0],
            r.pose.translation[1],
            screen[0],
            screen[1],
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
            if p.integer{let set=p.active_set.ok_or_else(||egui_ash_renderer::RendererError::Init("CAD2D input image not selected".into()))?;device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,p.layout,0,&[set],&[]);}
            device.cmd_push_constants(
                command,
                p.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
            for (batch,rgba) in r.batches.iter().zip(r.premultiplied.iter()).filter(|(b,_)| b.count > 0) {
                device.cmd_push_constants(
                    command,
                    p.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    32,
                    bytemuck::cast_slice(rgba),
                );
                if p.integer{
                    let ranges=r.ordered_draws.as_ref().ok_or_else(||egui_ash_renderer::RendererError::Init("CAD2D disjoint draw metadata absent".into()))?;
                    for range in ranges.iter().filter(|range|range.start>=batch.first&&range.end<=batch.first+batch.count){
                        let barrier=[vk::MemoryBarrier::default().src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::INPUT_ATTACHMENT_READ)];
                        device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,vk::PipelineStageFlags::FRAGMENT_SHADER,vk::DependencyFlags::BY_REGION,&barrier,&[],&[]);
                        device.cmd_draw_indexed(command,range.end-range.start,1,range.start,0,0);self.counters.draws.fetch_add(1,Ordering::Relaxed);
                    }
                }else{device.cmd_draw_indexed(command, batch.count, 1, batch.first, 0, 0);self.counters.draws.fetch_add(1, Ordering::Relaxed);}
            }
        }
        Ok(())
    }
}
fn prepare_palette(batches:&[Batch])->Result<Arc<[[f32;4]]>,String>{batches.iter().map(|b|b.color.qt_premultiplied8()).collect::<Result<Vec<_>,_>>().map(Arc::from)}
struct Cached {
    vertex: BufferId,
    index: BufferId,
    bytes: u64,
}
/// Cheap host-scoped callback factory; GPU ownership stays in Renderer.
#[derive(Clone)]pub struct Factory{device:vk::Device,pipeline:Arc<Mutex<Pipeline>>,counters:Arc<Counters>}
impl Factory{
 pub fn handle(&self,geometry:Arc<Geometry>,batches:Arc<[Batch]>,pose:Pose)->Result<Handle,String>{
  geometry.validate_batches(&batches)?;pose.validate()?;
  let inner=Arc::new(NativePaint{device:self.device,state:Mutex::new(State{geometry,premultiplied:prepare_palette(&batches)?,batches,pose,ready:None}),pipeline:self.pipeline.clone(),counters:self.counters.clone()});
  let callback=Arc::new(VulkanPaintCallback(inner.clone()));Ok(Handle{inner,callback})
 }
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
        views:&[vk::ImageView],
        integer:bool,
    ) -> Result<Self, String> {
        let descriptor_layout=if integer{let bindings=[vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];unsafe{context.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None)}.map_err(|e|e.to_string())?}else{vk::DescriptorSetLayout::null()};
        let set_layouts=if integer{vec![descriptor_layout]}else{Vec::new()};
        let ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: 48,
        }];
        let layout = match unsafe {
            context.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&ranges).set_layouts(&set_layouts),None)
        }{Ok(v)=>v,Err(e)=>{if integer{unsafe{context.device.destroy_descriptor_set_layout(descriptor_layout,None)}}return Err(e.to_string())}};
        let pipeline = match make_pipeline(&context.device, pass, layout,integer) {
            Ok(p) => p,
            Err(e) => {
                unsafe { context.device.destroy_pipeline_layout(layout, None);if integer{context.device.destroy_descriptor_set_layout(descriptor_layout,None)} };
                return Err(e);
            }
        };
        let (descriptor_pool,descriptor_sets)=if integer{match make_inputs(&context.device,descriptor_layout,views){Ok(v)=>v,Err(e)=>{unsafe{context.device.destroy_pipeline(pipeline,None);context.device.destroy_pipeline_layout(layout,None);context.device.destroy_descriptor_set_layout(descriptor_layout,None)}return Err(e)}}}else{(vk::DescriptorPool::null(),Vec::new())};
        Ok(Self {
            device: context.device.clone(),
            budget: budget.clone(),
            pressure_after_vertex: false,
            rolled_back_buffers: 0,
            pipeline: Arc::new(Mutex::new(Pipeline { pipeline, layout,integer,descriptor_layout,descriptor_pool,descriptor_sets,active_set:None })),
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
    pub fn set_render_pass(&mut self,pass:vk::RenderPass,views:&[vk::ImageView])->Result<(),String>{
        let mut p=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
        let replacement=make_pipeline(&self.device,pass,p.layout,p.integer)?;
        let inputs=if p.integer{match make_inputs(&self.device,p.descriptor_layout,views){Ok(v)=>v,Err(e)=>{unsafe{self.device.destroy_pipeline(replacement,None)};return Err(e)}}}else{(vk::DescriptorPool::null(),Vec::new())};
        unsafe{self.device.destroy_pipeline(p.pipeline,None);if p.descriptor_pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(p.descriptor_pool,None)}};
        p.pipeline=replacement;p.descriptor_pool=inputs.0;p.descriptor_sets=inputs.1;p.active_set=None;Ok(())
    }
    /// Called after the previous submitted frame fence, before recording this image.
    pub fn set_input_image(&mut self,index:usize)->Result<(),String>{let mut p=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;if p.integer{p.active_set=Some(*p.descriptor_sets.get(index).ok_or("CAD2D input image index invalid")?);}Ok(())}
    pub fn factory(&self)->Factory{Factory{device:self.device.handle(),pipeline:self.pipeline.clone(),counters:self.counters.clone()}}
    pub fn handle(&self,geometry:Arc<Geometry>,batches:Arc<[Batch]>,pose:Pose)->Result<Handle,String>{self.factory().handle(geometry,batches,pose)}
    /// Previous frame fence has completed. Prepare every requested buffer before
    /// installing any new drawable. On admission failure old ready states survive.
    pub fn prepare(
        &mut self,
        context: &VulkanContext,
        primitives: &[egui::ClippedPrimitive],
        fence: vk::Fence,
        completed: u64,
        next: u64,
        pixels_per_point:f32,
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
                if self.pipeline.lock().unwrap().integer{
                    s.geometry.validate_ordered_pose(s.pose,pixels_per_point)?;
                    let ranges=s.geometry.ordered_draws().ok_or("Exact integer CAD2D requires admitted source-ordered disjoint pixel geometry")?;
                    let mut end=0;for batch in s.batches.iter().filter(|b|b.count>0){if batch.first!=end{return Err("Exact integer CAD2D palette batches must partition ordered geometry".into());}end=batch.first+batch.count;if !ranges.iter().any(|r|r.end==end){return Err("Exact integer CAD2D palette boundary splits a disjoint draw".into());}}
                    if end as usize!=s.geometry.indices().len(){return Err("Exact integer CAD2D palette does not cover ordered geometry".into());}
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
                    premultiplied:s.premultiplied.clone(),
                    pose: s.pose,
                    ordered_draws:s.geometry.ordered_draws(),
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
        let geometry = Geometry::new_with_ordered_draws(
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
            vec![0, 1, 2],vec![0..3],
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
            completed,1.,
        )?;
        let before = self.budget.ledger.lock().unwrap().total_committed();
        let old = handle.inner.state.lock().unwrap().ready.clone().unwrap();
        let original_limit = self.budget.ledger.lock().unwrap().total_limit;
        let replacement = Geometry::new_with_ordered_draws(geometry.vertices().to_vec(), geometry.indices().to_vec(),vec![0..3])?;
        handle.update(replacement, batches.clone(), Pose::default())?;
        self.pressure_after_vertex = true;
        let result = self.prepare(
            context,
            std::slice::from_ref(&primitive),
            fence,
            completed,
            completed,1.,
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
            completed,1.,
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
        self.prepare(context, &[], fence, completed, completed,1.)?;
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
        if !self.resources.drain_graphics(){self.resources.retain_owner(self.pipeline.clone());return;}
        let mut p = self.pipeline.lock().unwrap();
        unsafe {
            self.device.destroy_pipeline(p.pipeline, None);
            self.device.destroy_pipeline_layout(p.layout, None);
            if p.descriptor_pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(p.descriptor_pool,None)}
            if p.descriptor_layout!=vk::DescriptorSetLayout::null(){self.device.destroy_descriptor_set_layout(p.descriptor_layout,None)}
        }
        p.pipeline = vk::Pipeline::null();
        p.layout = vk::PipelineLayout::null();
    }
}
fn make_pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    integer:bool,
) -> Result<vk::Pipeline, String> {
    let mut modules = Vec::new();
    let result = (|| {
        for bytes in [
            include_bytes!("shaders/cad2d.vert.spv").as_slice(),
            if integer{include_bytes!("shaders/cad2d_integer.frag.spv").as_slice()}else{include_bytes!("shaders/cad2d.frag.spv").as_slice()},
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
            .blend_enable(!integer)
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

fn make_inputs(device:&Device,layout:vk::DescriptorSetLayout,views:&[vk::ImageView])->Result<(vk::DescriptorPool,Vec<vk::DescriptorSet>),String>{
 if views.is_empty()||views.len()>16{return Err("CAD2D input image descriptor admission outside1..16".into());}
 let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::INPUT_ATTACHMENT,descriptor_count:views.len()as u32}];
 let pool=unsafe{device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(views.len()as u32).pool_sizes(&sizes),None)}.map_err(|e|e.to_string())?;
 let layouts=vec![layout;views.len()];let sets=match unsafe{device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))}{Ok(v)=>v,Err(e)=>{unsafe{device.destroy_descriptor_pool(pool,None)};return Err(e.to_string())}};
 for (set,view) in sets.iter().zip(views){let images=[vk::DescriptorImageInfo::default().image_view(*view).image_layout(vk::ImageLayout::GENERAL)];let write=[vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT).image_info(&images)];unsafe{device.update_descriptor_sets(&write,&[])}}
 Ok((pool,sets))
}
