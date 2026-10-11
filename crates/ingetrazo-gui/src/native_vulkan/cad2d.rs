//! Inline precise encoded CAD paint; immutable buffers uploaded only on cache misses.
pub mod arena;
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
    outline_slots_examined:AtomicU64,
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
    gates: Option<Arc<[crate::annotation_overlay_geometry::Draw]>>,
    id: u64,
    vertex: vk::Buffer,
    index: vk::Buffer,
    first_index:u32,
    base_vertex:i32,
    batches: Arc<[Batch]>,
    premultiplied: Arc<[[f32;4]]>,
    pose: Pose,
    ordered_draws:Option<Arc<[std::ops::Range<u32>]>>,
    opaque:bool,
}
struct AdmittedLayout { geometry:Arc<Geometry>, batches:Arc<[Batch]>, opaque:bool }
struct State {
    gates: Option<Arc<[crate::annotation_overlay_geometry::Draw]>>,
    geometry: Arc<Geometry>,
    batches: Arc<[Batch]>,
    premultiplied: Arc<[[f32;4]]>,
    pose: Pose,
    ready: Option<Ready>,
    admission:Option<AdmittedLayout>,
}
struct Pipeline {
    annotations: bool,
    visibility: Option<(vk::Buffer,u32,Arc<()>)>,
    pipeline: vk::Pipeline,
    lcd_pipeline:vk::Pipeline,
    layout: vk::PipelineLayout,
    integer:bool,
    descriptor_layout:vk::DescriptorSetLayout,
    descriptor_pool:vk::DescriptorPool,
    descriptor_sets:Vec<vk::DescriptorSet>,
    active_set:Option<vk::DescriptorSet>,
}
/// Cold engineering observer; a Weak reference never keeps GPU owners alive.
#[derive(Clone)]
pub struct DescriptorOwnerWeakForProbe(std::sync::Weak<Mutex<Pipeline>>);
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct DescriptorOwnerStateForProbe {
    integer:bool, pipeline:vk::Pipeline, layout:vk::PipelineLayout,
    descriptor_layout:vk::DescriptorSetLayout, descriptor_pool:vk::DescriptorPool,
    descriptor_sets:Vec<vk::DescriptorSet>, active_set:Option<vk::DescriptorSet>,
}
impl DescriptorOwnerStateForProbe {
    pub fn complete_integer(&self)->bool {
        self.integer && self.pipeline!=vk::Pipeline::null() && self.layout!=vk::PipelineLayout::null()
        && self.descriptor_layout!=vk::DescriptorSetLayout::null() && self.descriptor_pool!=vk::DescriptorPool::null()
        && !self.descriptor_sets.is_empty() && self.descriptor_sets.iter().all(|s|*s!=vk::DescriptorSet::null())
        && self.active_set.is_some_and(|s|self.descriptor_sets.contains(&s))
    }
    pub fn descriptor_set_count(&self)->usize{self.descriptor_sets.len()}
}
impl DescriptorOwnerWeakForProbe {
    pub fn state(&self)->Option<DescriptorOwnerStateForProbe>{
        let owner=self.0.upgrade()?;let p=owner.lock().ok()?;
        Some(DescriptorOwnerStateForProbe{integer:p.integer,pipeline:p.pipeline,layout:p.layout,
            descriptor_layout:p.descriptor_layout,descriptor_pool:p.descriptor_pool,
            descriptor_sets:p.descriptor_sets.clone(),active_set:p.active_set})
    }
    pub fn strong_count(&self)->usize{self.0.strong_count()}
}
pub(crate) struct PreparedRenderPass {
    device:Device,pipeline:vk::Pipeline,lcd_pipeline:vk::Pipeline,pool:vk::DescriptorPool,sets:Vec<vk::DescriptorSet>,
}
impl Drop for PreparedRenderPass{fn drop(&mut self){unsafe{
    if self.pipeline!=vk::Pipeline::null(){self.device.destroy_pipeline(self.pipeline,None)}
    if self.lcd_pipeline!=vk::Pipeline::null(){self.device.destroy_pipeline(self.lcd_pipeline,None)}
    if self.pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(self.pool,None)}
}}}
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
    /// Cold producer admission: Rust-visible callback/state records (including
    /// inline Ready storage) and distinct resolved palette payloads. Geometry,
    /// batch/range owners, shared pipeline/counters, Arc allocator headers and
    /// GPU residency are accounted separately; this is not total process RSS.
    pub fn retained_cpu_bytes(&self)->Result<usize,String>{
        let state=self.inner.state.lock().map_err(|_|"CAD2D state poisoned")?;
        let mut bytes=std::mem::size_of::<NativePaint>()+std::mem::size_of::<VulkanPaintCallback>();
        if self.callback.0.as_any().is::<AnnotationPaint>(){bytes+=std::mem::size_of::<AnnotationPaint>();}
        bytes=bytes.checked_add(state.premultiplied.len().checked_mul(std::mem::size_of::<[f32;4]>()).ok_or("CAD palette byte overflow")?).ok_or("CAD retained byte overflow")?;
        if let Some(ready)=&state.ready{if !Arc::ptr_eq(&ready.premultiplied,&state.premultiplied){bytes=bytes.checked_add(ready.premultiplied.len().checked_mul(std::mem::size_of::<[f32;4]>()).ok_or("CAD old palette byte overflow")?).ok_or("CAD retained byte overflow")?;}}
        Ok(bytes)
    }
    pub(crate) fn update_annotation(&self,geometry:Arc<Geometry>,batches:Arc<[Batch]>,pose:Pose,gates:Arc<[crate::annotation_overlay_geometry::Draw]>)->Result<(),String>{
        geometry.validate_batches(&batches)?;pose.validate()?;
        if gates.len()!=batches.len(){return Err("Annotation gate/batch inventory mismatch".into());}
        admit_draw_colors(&gates,&batches)?;
        let palette=prepare_palette(&geometry,&batches)?;
        let mut state=self.inner.state.lock().map_err(|_|"Annotation state poisoned")?;
        state.geometry=geometry;state.batches=batches;state.premultiplied=palette;state.pose=pose;state.gates=Some(gates);Ok(())
    }
    pub fn update(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<(), String> {
        pose.validate()?;
        let mut s = self
            .inner
            .state
            .lock()
            .map_err(|_| "CAD2D state poisoned")?;
        if Arc::ptr_eq(&s.geometry,&geometry)&&Arc::ptr_eq(&s.batches,&batches){s.pose=pose;return Ok(());}
        geometry.validate_batches(&batches)?;
        if !Arc::ptr_eq(&s.batches,&batches) || s.geometry.palette_cache_identity()!=geometry.palette_cache_identity(){s.premultiplied=prepare_palette(&geometry,&batches)?;}
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
        let p=self.pipeline.lock().unwrap();
        record_ready(info,device,command,&p,&self.counters,r.pose,std::iter::once(r))
    }
}
fn record_ready<'a>(info:&egui::PaintCallbackInfo,device:&Device,command:vk::CommandBuffer,p:&Pipeline,counters:&Counters,pose:Pose,ready:impl Iterator<Item=&'a Ready>)->RendererResult<()>{

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
        counters.callbacks.fetch_add(1, Ordering::Relaxed);

        let screen=[info.screen_size_px[0] as f32 / scale,info.screen_size_px[1] as f32 / scale];
        if screen.iter().any(|v|!v.is_finite()||*v<=0.){return Err(egui_ash_renderer::RendererError::Init("CAD2D logical screen is outside finite shader domain".into()));}
        let push = [
            pose.scale[0],
            pose.scale[1],
            pose.translation[0],
            pose.translation[1],
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

            if p.integer{let set=p.active_set.ok_or_else(||egui_ash_renderer::RendererError::Init("CAD2D input image not selected".into()))?;device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,p.layout,0,&[set],&[]);}
            device.cmd_push_constants(
                command,
                p.layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::cast_slice(&push),
            );
        }
        let mut bound=None;let mut bound_pipeline=p.pipeline;
        for r in ready { unsafe {
            let mut ordered_cursor=0;
            if bound!=Some((r.vertex,r.index)) {device.cmd_bind_vertex_buffers(command,0,&[r.vertex],&[0]);device.cmd_bind_index_buffer(command,r.index,0,vk::IndexType::UINT32);bound=Some((r.vertex,r.index));counters.binds.fetch_add(1,Ordering::Relaxed);}
            for (batch_index,(batch,rgba)) in r.batches.iter().zip(r.premultiplied.iter()).enumerate().filter(|(_, (b,_))| b.count > 0) {
                if p.annotations{
                    let count=p.visibility.as_ref().map(|v|v.1).ok_or_else(||egui_ash_renderer::RendererError::Init("Annotation visibility not prepared".into()))?;
                    let gate=r.gates.as_ref().and_then(|g|g.get(batch_index)).copied().ok_or_else(||egui_ash_renderer::RendererError::Init("Annotation draw gate absent".into()))?;
                    let ids=gate_ids(gate.gate,count).map_err(egui_ash_renderer::RendererError::Init)?;
                    device.cmd_push_constants(command,p.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,24,bytemuck::cast_slice(&ids));
                    let lcd=matches!(gate.composition,crate::annotation_overlay_geometry::Composition::GlyphLcd(_));
                    let selected=if lcd{p.lcd_pipeline}else{p.pipeline};
                    if selected==vk::Pipeline::null(){return Err(egui_ash_renderer::RendererError::Init("Annotation LCD pipeline not prepared".into()));}
                    if selected!=bound_pipeline{device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,selected);bound_pipeline=selected;}
                    if lcd{device.cmd_set_blend_constants(command,&gate.straight_rgba.map(|v|f32::from(v)/255.));}
                    let operator=match gate.composition{crate::annotation_overlay_geometry::Composition::SpanByte=>0u32,crate::annotation_overlay_geometry::Composition::GlyphRgba64Opaque=>1u32,crate::annotation_overlay_geometry::Composition::GlyphLcd(mask)=>2u32|(u32::from(mask[0])<<8)|(u32::from(mask[1])<<16)|(u32::from(mask[2])<<24)};
                    device.cmd_push_constants(command,p.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,48,bytemuck::bytes_of(&operator));
                }
                device.cmd_push_constants(
                    command,
                    p.layout,
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    32,
                    bytemuck::cast_slice(&if p.annotations && r.gates.as_ref().unwrap()[batch_index].composition!=crate::annotation_overlay_geometry::Composition::SpanByte{r.gates.as_ref().unwrap()[batch_index].straight_rgba.map(|v|f32::from(v)/255.)}else{*rgba}),
                );
                if p.integer&&!r.opaque{
                    let ranges=r.ordered_draws.as_ref().ok_or_else(||egui_ash_renderer::RendererError::Init("CAD2D disjoint draw metadata absent".into()))?;
                    ordered_intersections(batch.first..batch.first+batch.count,ranges,&mut ordered_cursor,|range,starts_layer|{
                        if starts_layer{let barrier=[vk::MemoryBarrier::default().src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::INPUT_ATTACHMENT_READ)];
                        device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,vk::PipelineStageFlags::FRAGMENT_SHADER,vk::DependencyFlags::BY_REGION,&barrier,&[],&[]);}
                        device.cmd_draw_indexed(command,range.end-range.start,1,r.first_index+range.start,r.base_vertex,0);counters.draws.fetch_add(1,Ordering::Relaxed);
                    });
                }else{device.cmd_draw_indexed(command, batch.count, 1, r.first_index+batch.first, r.base_vertex, 0);counters.draws.fetch_add(1, Ordering::Relaxed);}
            }
        }        }
        Ok(())
}

/// Palette records may split a disjoint source layer. Index partitions retain
/// source order; synchronization is required once on entry to each new layer,
/// never between its nonoverlapping color runs. The cursor advances monotonically.
fn ordered_intersections(batch:std::ops::Range<u32>,ranges:&[std::ops::Range<u32>],cursor:&mut usize,mut draw:impl FnMut(std::ops::Range<u32>,bool)){
 while let Some(range)=ranges.get(*cursor){
  if range.start>=batch.end{break;}
  let start=range.start.max(batch.start);let end=range.end.min(batch.end);
  if start<end{draw(start..end,start==range.start);}
  if range.end<=batch.end{*cursor+=1;}else{break;}
 }
}
fn gate_ids(gate:ingetrazo_render::annotations::Gate,count:u32)->Result<[u32;2],String>{
 use ingetrazo_render::annotations::Gate;
 let ids=match gate {
  Gate::Always=>[u32::MAX;2],
  Gate::Point(a) if a<count=>[a,u32::MAX],
  Gate::Both(a,b) if a<count&&b<count=>[a,b],
  _=>return Err("Annotation query gate outside retained output".into()),
 };
 Ok(ids)
}
fn admit_gates(gates:Option<&[crate::annotation_overlay_geometry::Draw]>,batches:usize,count:u32)->Result<(),String>{
 let gates=gates.ok_or("Annotation draw gates absent")?;
 if gates.len()!=batches{return Err("Annotation gate/batch inventory mismatch".into());}
 for draw in gates{gate_ids(draw.gate,count)?;}
 Ok(())
}
fn admit_draw_colors(draws:&[crate::annotation_overlay_geometry::Draw],batches:&[Batch])->Result<(),String>{
 if draws.len()!=batches.len(){return Err("Annotation material/batch inventory mismatch".into());}
 for (draw,batch) in draws.iter().zip(batches){
  if draw.composition!=crate::annotation_overlay_geometry::Composition::SpanByte
   &&batch.color.0!=draw.straight_rgba.map(|v|f32::from(v)/255.){
   return Err("Annotation straight glyph palette differs from admitted batch".into());
  }
 }
 Ok(())
}
fn prepare_palette(geometry:&Geometry,batches:&[Batch])->Result<Arc<[[f32;4]]>,String>{geometry.resolve_batch_colors(batches).map(Arc::from)}
/// A distinct callback type prevents the ordinary CAD preparation cache from
/// consuming or publishing annotation geometry.
struct AnnotationPaint(Arc<NativePaint>);
impl VulkanPaint for AnnotationPaint{
 fn as_any(&self)->&dyn Any{self}
 fn paint(&self,info:&egui::PaintCallbackInfo,device:&Device,command:vk::CommandBuffer)->RendererResult<()>{
  self.0.paint(info,device,command)
 }
}
struct AnnotationAdmission {
 geometry:Arc<Geometry>,batches:Arc<[Batch]>,gates:Arc<[crate::annotation_overlay_geometry::Draw]>,count:u32,opaque:bool,
}
impl AnnotationAdmission{
 fn matches(&self,state:&State,count:u32)->bool{
  Arc::ptr_eq(&self.geometry,&state.geometry)&&Arc::ptr_eq(&self.batches,&state.batches)
   &&state.gates.as_ref().is_some_and(|gates|Arc::ptr_eq(&self.gates,gates))&&self.count==count
 }
}
struct Cached {
    vertex: BufferId,
    index: BufferId,
    bytes: u64,
}
/// Cheap host-scoped callback factory; GPU ownership stays in Renderer.
#[derive(Clone)]pub struct Factory{device:vk::Device,pipeline:Arc<Mutex<Pipeline>>,counters:Arc<Counters>}
impl Factory{
 /// Retained callback owners keep this pipeline allocation alive across cache hits.
 pub fn cache_identity(&self)->usize{Arc::as_ptr(&self.pipeline)as usize}

 /// Fresh callback payload bound before allocating a handle. Annotation factories
 /// include their distinct wrapper; shared pipeline/counter owners stay separate.
 pub fn retained_cpu_bytes_bound(&self,palette_len:usize)->Result<usize,String>{
  let annotations=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?.annotations;
  (std::mem::size_of::<NativePaint>()+std::mem::size_of::<VulkanPaintCallback>()+if annotations{std::mem::size_of::<AnnotationPaint>()}else{0})
   .checked_add(palette_len.checked_mul(std::mem::size_of::<[f32;4]>()).ok_or("CAD palette byte overflow")?).ok_or_else(||"CAD retained byte overflow".into())
 }
 pub fn handle(&self,geometry:Arc<Geometry>,batches:Arc<[Batch]>,pose:Pose)->Result<Handle,String>{
  geometry.validate_batches(&batches)?;pose.validate()?;
  let inner=Arc::new(NativePaint{device:self.device,state:Mutex::new(State{gates:None,premultiplied:prepare_palette(&geometry,&batches)?,geometry,batches,pose,ready:None,admission:None}),pipeline:self.pipeline.clone(),counters:self.counters.clone()});
  let callback=Arc::new(VulkanPaintCallback(inner.clone()));Ok(Handle{inner,callback})
 }
 pub(crate) fn annotation_handle(&self,geometry:Arc<Geometry>,batches:Arc<[Batch]>,pose:Pose,gates:Arc<[crate::annotation_overlay_geometry::Draw]>)->Result<Handle,String>{
  if !self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?.annotations {return Err("Annotation factory requires gated pipeline".into());}
  if gates.len()!=batches.len(){return Err("Annotation gate/batch inventory mismatch".into());}
        admit_draw_colors(&gates,&batches)?;
  let mut handle=self.handle(geometry,batches,pose)?;
  handle.inner.state.lock().map_err(|_|"Annotation state poisoned")?.gates=Some(gates);
  handle.callback=Arc::new(VulkanPaintCallback(Arc::new(AnnotationPaint(handle.inner.clone()))));
  Ok(handle)
 }
}
pub struct Renderer {
    device: Device,
    lifetime:Arc<super::lifetime::Lifetime>,
    budget: Arc<VulkanBudget>,
    pressure_after_vertex: bool,
    rolled_back_buffers: u64,
    pipeline: Arc<Mutex<Pipeline>>,
    resources: Resources,
    arena:arena::Gpu,
    annotation_buffers:Option<(u64,Cached)>,
    annotation_owner:Option<std::sync::Weak<NativePaint>>,
    annotation_admission:Option<AnnotationAdmission>,
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
        Self::new_mode(context,budget,pass,views,integer,false)
    }
    pub(crate) fn new_annotations(context:&VulkanContext,budget:Arc<VulkanBudget>,pass:vk::RenderPass,views:&[vk::ImageView])->Result<Self,String>{
        Self::new_mode(context,budget,pass,views,true,true)
    }
    fn new_mode(context:&VulkanContext,budget:Arc<VulkanBudget>,pass:vk::RenderPass,views:&[vk::ImageView],integer:bool,annotations:bool)->Result<Self,String>{
        let descriptor_layout=if integer{let mut bindings=vec![vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];
        if annotations{bindings.push(vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT));}
unsafe{context.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None)}.map_err(|e|{if e==vk::Result::ERROR_DEVICE_LOST{context.lifetime.quarantine();}e.to_string()})?}else{vk::DescriptorSetLayout::null()};
        let set_layouts=if integer{vec![descriptor_layout]}else{Vec::new()};
        let ranges = [vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: if annotations{52}else{48},
        }];
        let layout = match unsafe {
            context.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&ranges).set_layouts(&set_layouts),None)
        }{Ok(v)=>v,Err(e)=>{if integer{unsafe{context.device.destroy_descriptor_set_layout(descriptor_layout,None)}}if e==vk::Result::ERROR_DEVICE_LOST{context.lifetime.quarantine();}return Err(e.to_string())}};
        let pipeline = match make_pipeline(&context.device, pass, layout,integer,annotations,false) {
            Ok(p) => p,
            Err(e) => {
                unsafe { context.device.destroy_pipeline_layout(layout, None);if integer{context.device.destroy_descriptor_set_layout(descriptor_layout,None)} };
                if e.device_lost(){context.lifetime.quarantine();}
                return Err(e.to_string());
            }
        };
        let lcd_pipeline=if annotations{match make_pipeline(&context.device,pass,layout,true,true,true){Ok(p)=>p,Err(e)=>{unsafe{context.device.destroy_pipeline(pipeline,None);context.device.destroy_pipeline_layout(layout,None);context.device.destroy_descriptor_set_layout(descriptor_layout,None)}if e.device_lost(){context.lifetime.quarantine();}return Err(e.to_string());}}}else{vk::Pipeline::null()};
        let (descriptor_pool,descriptor_sets)=if integer{match make_inputs(&context.device,descriptor_layout,views,annotations){Ok(v)=>v,Err(e)=>{unsafe{context.device.destroy_pipeline(pipeline,None);if lcd_pipeline!=vk::Pipeline::null(){context.device.destroy_pipeline(lcd_pipeline,None)}context.device.destroy_pipeline_layout(layout,None);context.device.destroy_descriptor_set_layout(descriptor_layout,None)}if e.device_lost(){context.lifetime.quarantine();}return Err(e.to_string())}}}else{(vk::DescriptorPool::null(),Vec::new())};
        Ok(Self {
            device: context.device.clone(),
            lifetime:context.lifetime.clone(),
            budget: budget.clone(),
            pressure_after_vertex: false,
            rolled_back_buffers: 0,
            pipeline: Arc::new(Mutex::new(Pipeline { annotations,visibility:None,pipeline,lcd_pipeline, layout,integer,descriptor_layout,descriptor_pool,descriptor_sets,active_set:None })),
            resources: Resources::new(context, budget),
            arena:Default::default(),
            annotation_buffers:None,annotation_owner:None,annotation_admission:None,
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
    /// One complete ordered annotation mesh and its staged visibility inventory.
    /// All locks, validation, uploads and retirement proofs precede either live
    /// descriptor/Ready change. The final publication section is infallible.
    pub(crate) fn prepare_annotation(
        &mut self,context:&VulkanContext,handle:&Handle,
        queries:super::annotation_visibility_gpu::PreparedVisibility<'_>,
        fence:vk::Fence,completed:u64,next:u64,pixels_per_point:f32,
    )->Result<(),String>{
        if next<=completed{return Err("Annotation submission serial must advance".into());}
        if !Arc::ptr_eq(&handle.inner.pipeline,&self.pipeline){return Err("Annotation callback belongs to another pipeline".into());}
        self.resources.publication_preflight(context,fence,completed,&[],&[])?;
        let (visibility,count,identity)=queries.output()?;
        if count==0||visibility==vk::Buffer::null(){return Err("Annotation visibility inventory empty".into());}
        let identity=identity.clone();
        let range=u64::from(count)*4;
        if range>u64::from(unsafe{context.instance.get_physical_device_properties(context.physical_device)}.limits.max_storage_buffer_range){return Err("Annotation visibility storage range exceeded".into());}
        let mut pipeline=self.pipeline.lock().map_err(|_|"Annotation pipeline poisoned")?;
        if !pipeline.annotations||!pipeline.integer||pipeline.descriptor_sets.is_empty(){return Err("Annotation integer pipeline not ready".into());}
        let old_owner=self.annotation_owner.as_ref().and_then(std::sync::Weak::upgrade);
        let mut old_state=if let Some(old)=old_owner.as_ref().filter(|old|!Arc::ptr_eq(old,&handle.inner)){
            Some(old.state.lock().map_err(|_|"Previous annotation state poisoned")?)
        }else{None};
        let mut state=handle.inner.state.lock().map_err(|_|"Annotation state poisoned")?;
        state.pose.validate()?;
        state.geometry.validate_ordered_pose(state.pose,pixels_per_point)?;
        state.geometry.source_spans().ok_or("Annotation immutable spans absent")?.translation(state.pose,pixels_per_point)?;
        let already_admitted=self.annotation_admission.as_ref().is_some_and(|key|key.matches(&state,count));
        let opaque=if already_admitted{self.annotation_admission.as_ref().unwrap().opaque}else{
            state.geometry.validate_batches(&state.batches)?;
            admit_gates(state.gates.as_deref(),state.batches.len(),count)?;
            let opaque=state.geometry.is_opaque(&state.batches)&&state.gates.as_ref().is_none_or(|gates|gates.iter().all(|g|!matches!(g.composition,crate::annotation_overlay_geometry::Composition::GlyphLcd(mask) if mask!=[255;3])));
            if !opaque{
                state.geometry.ordered_draws().ok_or("Annotation ordered span metadata absent")?;
                let mut end=0;
                for batch in state.batches.iter().filter(|batch|batch.count>0){
                    if batch.first!=end{return Err("Annotation batches do not partition geometry".into());}
                    end=batch.first+batch.count;
                }
                if end as usize!=state.geometry.indices().len(){return Err("Annotation palette does not cover geometry".into());}
            }
            opaque
        };
        let admitted=AnnotationAdmission{geometry:state.geometry.clone(),batches:state.batches.clone(),
            gates:state.gates.clone().expect("admitted annotation gates"),count,opaque};
        let id=state.geometry.id();
        let warm=self.annotation_buffers.as_ref().is_some_and(|(old,_)|*old==id);
        let mut fresh=None;
        let admission=(||{
            if !warm&&!state.geometry.indices().is_empty(){
                let vertex=self.resources.upload(context,bytemuck::cast_slice(state.geometry.vertices()),vk::BufferUsageFlags::VERTEX_BUFFER,false)?;
                let restore=if self.pressure_after_vertex{
                    self.pressure_after_vertex=false;
                    let mut ledger=self.budget.ledger.lock().unwrap();let previous=ledger.total_limit;
                    ledger.total_limit=Some(ledger.total_committed());
                    Some(PressureRestore{budget:self.budget.clone(),previous})
                }else{None};
                let index=self.resources.upload(context,bytemuck::cast_slice(state.geometry.indices()),vk::BufferUsageFlags::INDEX_BUFFER,false);
                drop(restore);
                let index=match index{Ok(index)=>index,Err(error)=>{self.resources.retire(vertex)?;self.rolled_back_buffers+=1;return Err(error);}};
                fresh=Some(Cached{vertex,index,bytes:state.geometry.logical_bytes()});
            }
            let candidate=if warm{self.annotation_buffers.as_ref().map(|(_,buffers)|buffers)}else{fresh.as_ref()};
            let old_ids=if warm{None}else{self.annotation_buffers.as_ref().map(|(_,buffers)|[buffers.vertex,buffers.index])};
            let next_ids=candidate.map(|buffers|[buffers.vertex,buffers.index]);
            let proof=self.resources.publication_preflight(context,fence,completed,
                old_ids.as_ref().map_or(&[][..],|ids|ids.as_slice()),
                next_ids.as_ref().map_or(&[][..],|ids|ids.as_slice()))?;
            let ready=if let Some(buffers)=candidate{
                let vertex=self.resources.buffer(buffers.vertex).ok_or("Annotation vertex owner missing")?;
                let index=self.resources.buffer(buffers.index).ok_or("Annotation index owner missing")?;
                self.resources.mark_used(buffers.vertex,next)?;self.resources.mark_used(buffers.index,next)?;
                Some(Ready{gates:state.gates.clone(),id,vertex,index,first_index:0,base_vertex:0,
                    batches:state.batches.clone(),premultiplied:state.premultiplied.clone(),pose:state.pose,
                    ordered_draws:state.geometry.ordered_draws(),opaque})
            }else{None};
            Ok((proof,ready))
        })();
        let (proof,ready)=match admission{Ok(value)=>value,Err(error)=>{
            if let Some(buffers)=fresh.take(){
                self.resources.retire(buffers.vertex)?;self.resources.retire(buffers.index)?;self.rolled_back_buffers+=2;
            }
            self.resources.collect(fence,completed)?;return Err(error);
        }};
        // No recoverable operation below. In-flight old uses were acknowledged
        // before staging. Query commit never exposes a new inventory to old Ready.
        queries.commit();
        if !pipeline.visibility.as_ref().is_some_and(|old|old.0==visibility&&old.1==count&&Arc::ptr_eq(&old.2,&identity)){
            let info=[vk::DescriptorBufferInfo::default().buffer(visibility).offset(0).range(range)];
            for set in &pipeline.descriptor_sets{unsafe{self.device.update_descriptor_sets(&[
                vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&info)],&[]);}}
            pipeline.visibility=Some((visibility,count,identity));
        }
        self.resources.retire_completed(proof);
        if !warm{
            self.annotation_buffers=fresh.take().map(|buffers|(id,buffers));
            if let Some((_,buffers))=&self.annotation_buffers{self.uploads+=1;self.upload_bytes+=buffers.bytes;}
        }else{self.cache_hits+=1;}
        if let Some(old)=old_state.as_mut(){old.ready=None;}
        state.ready=ready;self.annotation_owner=Some(Arc::downgrade(&handle.inner));self.annotation_admission=Some(admitted);
        Ok(())
    }
    pub(crate) fn force_annotation_pressure_for_probe(&mut self){self.pressure_after_vertex=true;}
    pub(crate) fn clear_annotation(&mut self,context:&VulkanContext,
        queries:Option<super::annotation_visibility_gpu::PreparedVisibility<'_>>,
        fence:vk::Fence,completed:u64)->Result<(),String>{
        let old=self.annotation_buffers.as_ref().map(|(_,buffers)|[buffers.vertex,buffers.index]);
        let proof=self.resources.publication_preflight(context,fence,completed,
            old.as_ref().map_or(&[][..],|ids|ids.as_slice()),&[])?;
        let mut pipeline=self.pipeline.lock().map_err(|_|"Annotation pipeline poisoned")?;
        if !pipeline.annotations{return Err("Ordinary CAD cannot clear annotation state".into());}
        let owner=self.annotation_owner.as_ref().and_then(std::sync::Weak::upgrade);
        let mut state=owner.as_ref().map(|owner|owner.state.lock().map_err(|_|"Annotation state poisoned")).transpose()?;
        if let Some(queries)=queries{queries.commit();}
        pipeline.visibility=None;
        if let Some(state)=state.as_mut(){state.ready=None;}
        self.resources.retire_completed(proof);self.annotation_buffers=None;
        self.annotation_owner=None;self.annotation_admission=None;Ok(())
    }
    pub(crate) fn bind_annotation_visibility(&mut self,context:&VulkanContext,buffer:vk::Buffer,count:u32,identity:&Arc<()>,fence:vk::Fence,completed:u64)->Result<(),String>{
        self.resources.publication_preflight(context,fence,completed,&[],&[])?;
        if buffer==vk::Buffer::null()||count==0{return Err("Annotation visibility binding empty".into());}
        let range=u64::from(count).checked_mul(4).filter(|&n|n<=u64::from(unsafe{context.instance.get_physical_device_properties(context.physical_device)}.limits.max_storage_buffer_range)).ok_or("Annotation visibility descriptor range exceeded")?;
        let mut pipeline=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
        if !pipeline.annotations{return Err("Ordinary CAD pipeline cannot bind annotation output".into());}
        if pipeline.visibility.as_ref().is_some_and(|old|old.0==buffer&&old.1==count&&Arc::ptr_eq(&old.2,identity)){return Ok(());}
        let info=[vk::DescriptorBufferInfo::default().buffer(buffer).offset(0).range(range)];
        for set in &pipeline.descriptor_sets{
            let writes=[vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&info)];
            unsafe{self.device.update_descriptor_sets(&writes,&[]);}
        }
        pipeline.visibility=Some((buffer,count,identity.clone()));
        Ok(())
    }
    pub fn descriptor_owner_weak_for_probe(&self)->DescriptorOwnerWeakForProbe{
        DescriptorOwnerWeakForProbe(Arc::downgrade(&self.pipeline))
    }
    pub fn set_render_pass(&mut self,pass:vk::RenderPass,views:&[vk::ImageView])->Result<(),String>{
        let prepared=self.prepare_render_pass(pass,views).map_err(|e|{if e.device_lost(){self.lifetime.quarantine();}e.to_string()})?;self.commit_render_pass(prepared)
    }
    pub(crate) fn prepare_render_pass(&self,pass:vk::RenderPass,views:&[vk::ImageView])->Result<PreparedRenderPass,super::resize::PrepareError>{
        self.resources.check_submission()?;
        let p=self.pipeline.lock().map_err(|_|super::resize::PrepareError::Other("CAD2D pipeline poisoned".into()))?;
        let replacement=make_pipeline(&self.device,pass,p.layout,p.integer,p.annotations,false)?;
        let mut prepared=PreparedRenderPass{device:self.device.clone(),pipeline:replacement,lcd_pipeline:vk::Pipeline::null(),pool:vk::DescriptorPool::null(),sets:Vec::new()};
        if p.annotations{prepared.lcd_pipeline=make_pipeline(&self.device,pass,p.layout,true,true,true)?;}
        if p.integer{let inputs=make_inputs(&self.device,p.descriptor_layout,views,p.annotations)?;prepared.pool=inputs.0;prepared.sets=inputs.1;}
        Ok(prepared)
    }
    fn publish_render_pass(device:&Device,p:&mut Pipeline,mut prepared:PreparedRenderPass){
        let previous=std::mem::replace(&mut p.pipeline,std::mem::replace(&mut prepared.pipeline,vk::Pipeline::null()));
        let previous_lcd=std::mem::replace(&mut p.lcd_pipeline,std::mem::replace(&mut prepared.lcd_pipeline,vk::Pipeline::null()));
        let previous_pool=std::mem::replace(&mut p.descriptor_pool,std::mem::replace(&mut prepared.pool,vk::DescriptorPool::null()));
        p.descriptor_sets=std::mem::take(&mut prepared.sets);p.active_set=None;p.visibility=None;
        unsafe{device.destroy_pipeline(previous,None);if previous_lcd!=vk::Pipeline::null(){device.destroy_pipeline(previous_lcd,None)}if previous_pool!=vk::DescriptorPool::null(){device.destroy_descriptor_pool(previous_pool,None)}};
    }
    pub(crate) fn commit_render_pass(&mut self,prepared:PreparedRenderPass)->Result<(),String>{
        let mut p=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
        Self::publish_render_pass(&self.device,&mut p,prepared);Ok(())
    }
    /// Both immutable owners are locked before either pipeline changes.
    pub(crate) fn commit_render_pass_pair(&mut self,prepared:PreparedRenderPass,
        other:&mut Self,other_prepared:PreparedRenderPass)->Result<(),String>{
        if Arc::ptr_eq(&self.pipeline,&other.pipeline){return Err("CAD pipeline pair aliases one owner".into());}
        let mut p=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
        let mut q=other.pipeline.lock().map_err(|_|"Annotation pipeline poisoned")?;
        Self::publish_render_pass(&self.device,&mut p,prepared);
        Self::publish_render_pass(&other.device,&mut q,other_prepared);Ok(())
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
        {
            let pipeline=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?;
            if pipeline.annotations{
                let count=pipeline.visibility.as_ref().map(|v|v.1).ok_or("Annotation visibility not prepared")?;
                for primitive in primitives {
                    if let egui::epaint::Primitive::Callback(callback)=&primitive.primitive {
                        if let Some(callback)=callback.callback.downcast_ref::<VulkanPaintCallback>() {
                            if let Some(paint)=callback.0.as_any().downcast_ref::<NativePaint>() {
                                if !Arc::ptr_eq(&paint.pipeline,&self.pipeline){return Err("Annotation callback belongs to another pipeline".into());}
                                let state=paint.state.lock().map_err(|_|"Annotation state poisoned")?;
                                admit_gates(state.gates.as_deref(),state.batches.len(),count)?;
                            }
                        }
                    }
                }
            }
        }
        self.arena.prepare(context,&mut self.resources,&self.budget,&self.pipeline,primitives,completed,next,fence)?;
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
                let mut s = paint.state.lock().map_err(|_| "CAD2D state poisoned")?;
                s.pose.validate()?;
                if let Some(r) = &s.ready {
                    self.active.insert(r.id);
                }
                let integer=self.pipeline.lock().map_err(|_|"CAD2D pipeline poisoned")?.integer;
                let same=s.admission.as_ref().is_some_and(|a|Arc::ptr_eq(&a.geometry,&s.geometry)&&Arc::ptr_eq(&a.batches,&s.batches));
                if !same {
                    s.geometry.validate_batches(&s.batches)?;
                    let opaque=s.geometry.is_opaque(&s.batches);
                    if integer&&!opaque {
                        s.geometry.ordered_draws().ok_or("Exact integer CAD2D requires admitted source-ordered disjoint pixel geometry")?;
                        let mut end=0;for batch in s.batches.iter().filter(|b|b.count>0){if batch.first!=end{return Err("Exact integer CAD2D palette batches must partition ordered geometry".into());}end=batch.first+batch.count;}
                        if end as usize!=s.geometry.indices().len(){return Err("Exact integer CAD2D palette does not cover ordered geometry".into());}
                    }
                    s.admission=Some(AdmittedLayout{geometry:s.geometry.clone(),batches:s.batches.clone(),opaque});
                }
                if integer&&!s.admission.as_ref().unwrap().opaque {
                    s.geometry.validate_ordered_pose(s.pose,pixels_per_point)?;
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
                    gates:s.gates.clone(),
                    id,
                    vertex: self.resources.buffer(c.vertex).unwrap(),
                    index: self.resources.buffer(c.index).unwrap(),
                    first_index:0,base_vertex:0,
                    batches: s.batches.clone(),
                    premultiplied:s.premultiplied.clone(),
                    pose: s.pose,
                    ordered_draws:s.geometry.ordered_draws(),
                    opaque:s.admission.as_ref().expect("layout admitted before publication").opaque,
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
    pub fn outline_arena_stats(&self)->serde_json::Value{serde_json::json!({"arena":self.arena.metrics(),"actual_live_page_allocation_bytes":self.arena.allocation_bytes(&self.resources),"total_charged_gpu_bytes":self.budget.ledger.lock().unwrap().total_committed(),"transfers":self.resources.stats,"callbacks":self.counters.callbacks.load(Ordering::Relaxed),"binds":self.counters.binds.load(Ordering::Relaxed),"draws":self.counters.draws.load(Ordering::Relaxed),"draw_slots_examined":self.counters.outline_slots_examined.load(Ordering::Relaxed)})}
    pub fn stats(&self) -> Stats {
        Stats {
            callbacks: self.counters.callbacks.load(Ordering::Relaxed),
            binds: self.counters.binds.load(Ordering::Relaxed),
            draws: self.counters.draws.load(Ordering::Relaxed),
            uploads: self.uploads,
            upload_bytes: self.upload_bytes,
            cache_hits: self.cache_hits,
            cached_bytes: self.cache.values().map(|c| c.bytes).sum::<u64>()+self.annotation_buffers.as_ref().map_or(0,|(_,buffers)|buffers.bytes),
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
            if p.lcd_pipeline!=vk::Pipeline::null(){self.device.destroy_pipeline(p.lcd_pipeline,None);}
            self.device.destroy_pipeline_layout(p.layout, None);
            if p.descriptor_pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(p.descriptor_pool,None)}
            if p.descriptor_layout!=vk::DescriptorSetLayout::null(){self.device.destroy_descriptor_set_layout(p.descriptor_layout,None)}
        }
        p.pipeline = vk::Pipeline::null();p.lcd_pipeline=vk::Pipeline::null();
        p.layout = vk::PipelineLayout::null();
    }
}
fn make_pipeline(
    device: &Device,
    pass: vk::RenderPass,
    layout: vk::PipelineLayout,
    integer:bool,
    annotations:bool,
    lcd:bool,
) -> Result<vk::Pipeline, super::resize::PrepareError> {
    let mut modules = Vec::new();
    let result = (|| {
        for bytes in [
            include_bytes!("shaders/cad2d.vert.spv").as_slice(),
            if annotations{include_bytes!("shaders/annotation_overlay.frag.spv").as_slice()}else if integer{include_bytes!("shaders/cad2d_integer.frag.spv").as_slice()}else{include_bytes!("shaders/cad2d.frag.spv").as_slice()},
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
            .blend_enable(!integer||lcd)
            .src_color_blend_factor(if lcd{vk::BlendFactor::CONSTANT_COLOR}else{vk::BlendFactor::ONE})
            .dst_color_blend_factor(if lcd{vk::BlendFactor::ONE_MINUS_SRC_COLOR}else{vk::BlendFactor::ONE_MINUS_SRC_ALPHA})
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(if lcd{vk::ColorComponentFlags::R|vk::ColorComponentFlags::G|vk::ColorComponentFlags::B}else{vk::ColorComponentFlags::RGBA})];
        let blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
        let dynamic = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR,vk::DynamicState::BLEND_CONSTANTS]);
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
                Err(e.into())
            }
        }
    })();
    for m in modules {
        unsafe { device.destroy_shader_module(m, None) }
    }
    result
}

fn make_inputs(device:&Device,layout:vk::DescriptorSetLayout,views:&[vk::ImageView],annotations:bool)->Result<(vk::DescriptorPool,Vec<vk::DescriptorSet>),super::resize::PrepareError>{
 if views.is_empty()||views.len()>16{return Err("CAD2D input image descriptor admission outside1..16".into());}
 let mut sizes=vec![vk::DescriptorPoolSize{ty:vk::DescriptorType::INPUT_ATTACHMENT,descriptor_count:views.len()as u32}];if annotations{sizes.push(vk::DescriptorPoolSize{ty:vk::DescriptorType::STORAGE_BUFFER,descriptor_count:views.len()as u32});}
 let pool=unsafe{device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(views.len()as u32).pool_sizes(&sizes),None)}.map_err(super::resize::PrepareError::from)?;
 let layouts=vec![layout;views.len()];let sets=match unsafe{device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))}{Ok(v)=>v,Err(e)=>{unsafe{device.destroy_descriptor_pool(pool,None)};return Err(e.into())}};
 for (set,view) in sets.iter().zip(views){let images=[vk::DescriptorImageInfo::default().image_view(*view).image_layout(vk::ImageLayout::GENERAL)];let write=[vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::INPUT_ATTACHMENT).image_info(&images)];unsafe{device.update_descriptor_sets(&write,&[])}}
 Ok((pool,sets))
}


#[cfg(test)]
mod annotation_gate_tests {
 use super::*;
 use ingetrazo_render::annotations::Gate;
 #[test]fn palette_splits_draw_intersections_without_reordering_or_extra_layer_barriers(){
  let ranges=[0..18,18..30,30..42];let batches=[0..6,6..24,24..42];let mut cursor=0;let mut calls=Vec::new();
  for batch in batches{ordered_intersections(batch,&ranges,&mut cursor,|range,barrier|calls.push((range,barrier)));}
  assert_eq!(calls,vec![(0..6,true),(6..18,false),(18..24,true),(24..30,false),(30..42,true)]);
  assert_eq!(cursor,ranges.len());
  let mut indices=Vec::new();for(range,_)in calls{indices.extend(range);}assert_eq!(indices,(0..42).collect::<Vec<_>>());
 }
 #[test]fn glyph_material_admission_retains_straight_alpha_and_refuses_palette_alias(){
  use crate::annotation_overlay_geometry::{Draw,Composition};
  for alpha in [0,1,127,128,230,255]{
   let raw=[37,93,157,alpha];let batch=Batch{first:0,count:6,color:ingetrazo_render::cad2d::EncodedRgba::from_bytes(raw,1.).unwrap()};
   let mut draw=Draw{gate:Gate::Always,composition:Composition::GlyphRgba64Opaque,straight_rgba:raw};
   assert!(admit_draw_colors(&[draw],&[batch]).is_ok());
   draw.straight_rgba[3]^=1;assert!(admit_draw_colors(&[draw],&[batch]).is_err());
  }
 }
 #[test]fn gates_refuse_sentinel_and_inventory_before_gpu_admission(){
  assert_eq!(gate_ids(Gate::Always,0).unwrap(),[u32::MAX;2]);
  assert_eq!(gate_ids(Gate::Both(0,2),3).unwrap(),[0,2]);
  assert!(gate_ids(Gate::Point(3),3).is_err());
  assert!(gate_ids(Gate::Point(u32::MAX),u32::MAX).is_err());
  assert!(gate_ids(Gate::Both(0,u32::MAX),u32::MAX).is_err());
  assert!(admit_gates(None,1,3).is_err());
  assert!(admit_gates(Some(&[crate::annotation_overlay_geometry::Draw::span(Gate::Point(0))]),2,3).is_err());
  assert!(admit_gates(Some(&[Gate::Always,Gate::Both(0,2)].map(crate::annotation_overlay_geometry::Draw::span)),2,3).is_ok());
 }
}
