//! Ordered immutable outline chunks in fresh GPU page ranges. One callback.
use super::*;
use crate::{image_outline_arena::{Planner,Ticket},image_outline_chunks::{ChunkFrame,PlaneOutlineChunk}};
use std::{collections::BTreeMap,sync::Weak};
const PAGE:u64=1024*1024;
// Resources uses actual Vulkan requirements and the shared live heap ledger.
fn budget_admission(error:&str)->bool{let error=error.strip_prefix("A error occured when using the allocator: ").unwrap_or(error);(error.starts_with("GPU allocation ")&&error.contains(" exceeds global application budget "))||(error.starts_with("GPU heap ")&&(error.contains(" exceeds application budget ")||error.contains(" exceeds live budget ")))}
struct State{chunks:Vec<Option<Arc<PlaneOutlineChunk>>>,dirty:Vec<usize>,pose:Pose,ready_pose:Pose,ready:Vec<Option<Ready>>,serial:u64,hold_failed_for_probe:bool}
struct Paint{owner:Weak<Paint>,device:vk::Device,pipeline:Arc<Mutex<Pipeline>>,counters:Arc<Counters>,state:Mutex<State>}
#[derive(Clone)]pub struct Handle{inner:Arc<Paint>,callback:Arc<VulkanPaintCallback>}
impl Factory{pub fn outline_arena(&self)->Handle{let inner=Arc::new_cyclic(|owner|Paint{owner:owner.clone(),device:self.device,pipeline:self.pipeline.clone(),counters:self.counters.clone(),state:Mutex::new(State{chunks:Vec::new(),dirty:Vec::new(),pose:Pose::default(),ready_pose:Pose::default(),ready:Vec::new(),serial:0,hold_failed_for_probe:false})});let callback=Arc::new(VulkanPaintCallback(inner.clone()));Handle{inner,callback}}}
impl Handle{
 pub fn update(&self,frame:ChunkFrame<'_>,pose:Pose)->Result<(),String>{pose.validate()?;let mut s=self.inner.state.lock().map_err(|_|"Outline arena state poisoned")?;
  for &index in frame.changed{if index>=frame.slots.len(){return Err("Outline changed slot outside publication".into());}}
  if frame.serial!=s.serial{s.hold_failed_for_probe=false;if frame.replacement||s.chunks.len()!=frame.slots.len(){s.chunks=frame.slots.to_vec();s.dirty.clear();s.dirty.extend(0..frame.slots.len());}else{for &i in frame.changed{s.chunks[i]=frame.slots[i].clone();s.dirty.push(i);}s.dirty.sort_unstable();s.dirty.dedup();}s.serial=frame.serial;}s.pose=pose;Ok(())
 }
 pub fn shape(&self,rect:egui::Rect)->egui::Shape{egui::PaintCallback{rect,callback:self.callback.clone()}.into()}
}
impl VulkanPaint for Paint{fn as_any(&self)->&dyn Any{self}fn paint(&self,info:&egui::PaintCallbackInfo,device:&Device,command:vk::CommandBuffer)->RendererResult<()>{if device.handle()!=self.device{return Err(egui_ash_renderer::RendererError::Init("Outline arena device mismatch".into()));}let s=self.state.lock().unwrap();self.counters.outline_slots_examined.fetch_add(s.ready.len()as u64,Ordering::Relaxed);let p=self.pipeline.lock().unwrap();record_ready(info,device,command,&p,&self.counters,s.ready_pose,s.ready.iter().flatten())}}
struct Slot{vertex:Ticket,index:Ticket}
#[derive(Default,Clone,Debug,serde::Serialize)]pub struct Metrics{pub pages:u64,pub page_bytes:u64,pub uploads:u64,pub upload_bytes:u64,pub candidate_rollbacks:u64,pub preserved_failures:u64,pub live_slots:u64,pub retired_bytes:u64,pub prepare_slots_examined:u64,pub page_use_marks:u64,pub reclaimed_pages:u64,pub reclaimed_bytes:u64,pub exact_size_retries:u64}
#[derive(Default)]pub(super)struct Gpu{vertex:Planner,index:Planner,vpages:BTreeMap<u64,BufferId>,ipages:BTreeMap<u64,BufferId>,slots:Vec<Option<Slot>>,paint:Option<Weak<Paint>>,next_page:u64,page_bytes:u64,pressure_after:Option<usize>,metrics:Metrics}
impl Gpu{
 pub(super)fn allocation_bytes(&self,resources:&Resources)->u64{self.vpages.values().chain(self.ipages.values()).filter_map(|id|resources.allocation_bytes(*id)).sum()}
 pub(super)fn metrics(&self)->Metrics{let mut m=self.metrics.clone();m.pages=(self.vpages.len()+self.ipages.len())as u64;m.page_bytes=self.vertex.metrics().page_bytes+self.index.metrics().page_bytes;m.retired_bytes=self.vertex.metrics().retired_bytes+self.index.metrics().retired_bytes;m.live_slots=self.slots.iter().flatten().count()as u64;m}
 fn reserve(planner:&mut Planner,pages:&mut BTreeMap<u64,BufferId>,next:&mut u64,resources:&mut Resources,bytes:u64,usage:vk::BufferUsageFlags,new_pages:&mut Vec<(bool,u64)>,vertex:bool,page_bytes:u64,context:&VulkanContext,other_planner:&mut Planner,other_pages:&mut BTreeMap<u64,BufferId>,fence:vk::Fence,completed:u64,metrics:&mut Metrics)->Result<Ticket,String>{
  if let Some(t)=planner.reserve(bytes,if vertex{12}else{4})?{return Ok(t);}
  let capacity=bytes.checked_add(page_bytes-1).map(|v|v/page_bytes*page_bytes).ok_or("Outline page size overflow")?;
  let id=next.checked_add(1).ok_or("Outline page identity exhausted")?;let max_bytes=if vertex{(i32::MAX as u64)*12}else{(u32::MAX as u64)*4};
  if bytes>max_bytes||capacity>max_bytes{return Err("Outline page exceeds indexed draw address range".into());}
  let (buffer,capacity)=match resources.allocate_buffer(capacity,usage){Ok(b)=>(b,capacity),Err(error)=>{
   if !budget_admission(&error){return Err(error);}
   resources.check_submission()?;
   if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Outline pressure reclaim requires an acknowledged signaled fence".into());}
   for (p,map) in [(&mut *planner,&mut *pages),(other_planner,other_pages)]{
    let empty=p.empty_pages().collect::<Vec<_>>();
    for (page,size) in empty{resources.retire(map[&page])?;p.remove_empty_page(page)?;map.remove(&page);metrics.reclaimed_pages+=1;metrics.reclaimed_bytes+=size;}
   }
   resources.collect(fence,completed)?;
   // Retry only genuine ledger admission, never device errors or uncertain work.
   let exact=bytes.checked_add(3).map(|b|b/4*4).ok_or("Outline exact page overflow")?;
   metrics.exact_size_retries+=1;(resources.allocate_buffer(exact,usage)?,exact)
  }};if let Err(e)=planner.add_page(id,capacity){resources.retire(buffer)?;return Err(e);}pages.insert(id,buffer);*next=id;new_pages.push((vertex,id));planner.reserve(bytes,if vertex{12}else{4})?.ok_or_else(||"New outline page reservation failed".into())
 }
 pub(super)fn prepare(&mut self,context:&VulkanContext,resources:&mut Resources,budget:&Arc<VulkanBudget>,pipeline:&Arc<Mutex<Pipeline>>,primitives:&[egui::ClippedPrimitive],completed:u64,next:u64,fence:vk::Fence)->Result<(),String>{
  self.vertex.collect(completed)?;self.index.collect(completed)?;
  let mut active=None;
  for primitive in primitives{let egui::epaint::Primitive::Callback(c)=&primitive.primitive else{continue;};let Some(cb)=c.callback.downcast_ref::<VulkanPaintCallback>()else{continue;};let Some(p)=cb.0.as_any().downcast_ref::<Paint>()else{continue;};if !Arc::ptr_eq(&p.pipeline,pipeline){return Err("Outline arena belongs to different host".into());}if active.is_some(){return Err("One outline arena callback per frame is required".into());}active=Some(p);}
  let changed_owner=match(&self.paint,active){(Some(old),Some(p))=>old.as_ptr()!=p as *const Paint,(None,None)=>false,_=>true};
  let Some(paint)=active else{for slot in self.slots.iter_mut().filter_map(Option::take){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}self.slots.clear();if let Some(old)=self.paint.take().and_then(|p|p.upgrade()){old.state.lock().unwrap().ready.clear();}return Ok(());};
  let mut s=paint.state.lock().map_err(|_|"Outline arena state poisoned")?;
  if changed_owner{s.dirty.clear();let len=s.chunks.len();s.dirty.extend(0..len);}
  if s.hold_failed_for_probe{for slot in self.slots.iter().flatten(){self.metrics.prepare_slots_examined+=1;self.vertex.mark_used(slot.vertex,next)?;self.index.mark_used(slot.index,next)?;resources.mark_used(self.vpages[&slot.vertex.range.page],next)?;resources.mark_used(self.ipages[&slot.index.range.page],next)?;self.metrics.page_use_marks+=2;}return Ok(());}
  let mut candidates=Vec::new();let mut new_pages=Vec::new();let page_bytes=if self.page_bytes==0{PAGE}else{self.page_bytes};let pressure_after=self.pressure_after.take();let mut restore=None;
  let result=(||{
   for &i in &s.dirty{let Some(chunk)=s.chunks[i].as_ref()else{candidates.push((i,None,None));continue;};chunk.geometry.validate_batches(&chunk.batches)?;if !chunk.geometry.is_opaque(&chunk.batches){return Err("Outline opaque admission changed".into());}
    let vertex=Self::reserve(&mut self.vertex,&mut self.vpages,&mut self.next_page,resources,(chunk.geometry.vertices().len()*std::mem::size_of::<ingetrazo_render::cad2d::Vertex>())as u64,vk::BufferUsageFlags::VERTEX_BUFFER,&mut new_pages,true,page_bytes,context,&mut self.index,&mut self.ipages,fence,completed,&mut self.metrics)?;
    let index=match Self::reserve(&mut self.index,&mut self.ipages,&mut self.next_page,resources,(chunk.geometry.indices().len()*4)as u64,vk::BufferUsageFlags::INDEX_BUFFER,&mut new_pages,false,page_bytes,context,&mut self.vertex,&mut self.vpages,fence,completed,&mut self.metrics){Ok(v)=>v,Err(e)=>{self.vertex.rollback(vertex)?;return Err(e);}};
    // Tickets are Pending in the exclusively owned planner, with disjoint fresh
    // ranges. No publication/mark_used occurs until every transfer succeeds.
    candidates.push((i,Some(Slot{vertex,index}),None));
    unsafe{resources.upload_fresh_range(context,self.vpages[&vertex.range.page],vertex.range.offset,bytemuck::cast_slice(chunk.geometry.vertices()))?;}self.metrics.uploads+=1;self.metrics.upload_bytes+=vertex.range.bytes;
    unsafe{resources.upload_fresh_range(context,self.ipages[&index.range.page],index.range.offset,bytemuck::cast_slice(chunk.geometry.indices()))?;}self.metrics.uploads+=1;self.metrics.upload_bytes+=index.range.bytes;
    let ready=Ready{marker_offset:None,marker_groups:None,gates:None,id:chunk.geometry.id(),vertex:resources.buffer(self.vpages[&vertex.range.page]).ok_or("Missing outline vertex page")?,index:resources.buffer(self.ipages[&index.range.page]).ok_or("Missing outline index page")?,first_index:u32::try_from(index.range.offset/4).map_err(|_|"Outline index offset overflow")?,base_vertex:i32::try_from(vertex.range.offset/std::mem::size_of::<ingetrazo_render::cad2d::Vertex>()as u64).map_err(|_|"Outline vertex offset overflow")?,batches:chunk.batches.clone(),premultiplied:prepare_palette(&chunk.geometry,&chunk.batches)?,pose:s.pose,ordered_draws:None,opaque:true};
    candidates.last_mut().unwrap().2=Some(ready);
    if pressure_after==Some(candidates.len()){let mut ledger=budget.ledger.lock().unwrap();let previous=ledger.total_limit;ledger.total_limit=Some(ledger.total_committed());restore=Some(PressureRestore{budget:budget.clone(),previous});}
   }Ok(())
  })();
  drop(restore);
  if let Err(error)=result{self.metrics.preserved_failures+=1;if resources.check_submission().is_ok(){for(_,slot,_)in candidates{if let Some(slot)=slot{self.vertex.rollback(slot.vertex)?;self.index.rollback(slot.index)?;self.metrics.candidate_rollbacks+=1;}}for(v,id)in new_pages{let(planner,pages)=if v{(&mut self.vertex,&mut self.vpages)}else{(&mut self.index,&mut self.ipages)};planner.remove_empty_page(id)?;resources.retire(pages.remove(&id).unwrap())?;}}return Err(error);}
  if changed_owner{for slot in self.slots.iter_mut().filter_map(Option::take){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}self.slots.clear();if let Some(old)=self.paint.take().and_then(|p|p.upgrade()){old.state.lock().unwrap().ready.clear();}}
  if self.slots.len()>s.chunks.len(){for slot in self.slots.drain(s.chunks.len()..).flatten(){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}}
  let len=s.chunks.len();self.slots.resize_with(len,||None);s.ready.resize_with(len,||None);
  for(i,slot,ready)in candidates{if let Some(old)=self.slots[i].take(){self.vertex.retire(old.vertex)?;self.index.retire(old.index)?;}if let Some(slot)=&slot{self.vertex.publish(slot.vertex)?;self.index.publish(slot.index)?;}self.slots[i]=slot;s.ready[i]=ready;}
  s.dirty.clear();s.ready_pose=s.pose;
  for slot in self.slots.iter().flatten(){self.metrics.prepare_slots_examined+=1;self.vertex.mark_used(slot.vertex,next)?;self.index.mark_used(slot.index,next)?;resources.mark_used(self.vpages[&slot.vertex.range.page],next)?;resources.mark_used(self.ipages[&slot.index.range.page],next)?;self.metrics.page_use_marks+=2;}
  self.paint=Some(paint.owner.clone());
  self.metrics.pages=(self.vpages.len()+self.ipages.len())as u64;self.metrics.page_bytes=self.vertex.metrics().page_bytes+self.index.metrics().page_bytes;self.metrics.live_slots=self.slots.iter().flatten().count()as u64;self.metrics.retired_bytes=self.vertex.metrics().retired_bytes+self.index.metrics().retired_bytes;Ok(())
 }
}

impl Renderer{
 /// Cold engineering admission proof using real budget reservations, never forged Vulkan failures.
 pub fn verify_outline_budget_for_probe(&mut self,context:&VulkanContext,fence:vk::Fence,completed:u64)->Result<serde_json::Value,String>{
  self.resources.check_submission()?;
  if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Budget probe requires completed fence".into());}
  let mut g=Gpu::default();let mut fresh=Vec::new();let base=self.budget.ledger.lock().unwrap().total_committed();
  let result=(||{
   let pending=Gpu::reserve(&mut g.vertex,&mut g.vpages,&mut g.next_page,&mut self.resources,40*1024*1024,vk::BufferUsageFlags::VERTEX_BUFFER,&mut fresh,true,PAGE,context,&mut g.index,&mut g.ipages,fence,completed,&mut g.metrics)?;
   let actual=self.resources.allocation_bytes(g.vpages[&pending.range.page]).ok_or("Missing large page")?;
   let previous=self.budget.ledger.lock().unwrap().total_limit;
   self.budget.ledger.lock().unwrap().total_limit=Some(base.checked_add(actual.checked_mul(2).ok_or("Probe cap overflow")?).ok_or("Probe cap overflow")?);
   let restore=PressureRestore{budget:self.budget.clone(),previous};
   let denied=Gpu::reserve(&mut g.vertex,&mut g.vpages,&mut g.next_page,&mut self.resources,48*1024*1024,vk::BufferUsageFlags::VERTEX_BUFFER,&mut fresh,true,PAGE,context,&mut g.index,&mut g.ipages,fence,completed,&mut g.metrics).err().ok_or("Pending page unexpectedly reclaimed")?;
   if !budget_admission(&denied)||g.metrics.reclaimed_pages!=0{return Err("Pending ownership pressure control failed".into());}
   g.vertex.rollback(pending)?;
   let replacement=Gpu::reserve(&mut g.vertex,&mut g.vpages,&mut g.next_page,&mut self.resources,48*1024*1024,vk::BufferUsageFlags::VERTEX_BUFFER,&mut fresh,true,PAGE,context,&mut g.index,&mut g.ipages,fence,completed,&mut g.metrics)?;
   if g.metrics.reclaimed_pages!=1||g.vertex.metrics().page_bytes!=48*1024*1024{return Err("Completed empty page reclamation failed".into());}
   g.vertex.rollback(replacement)?;drop(restore);
   for(page,_)in g.vertex.empty_pages().collect::<Vec<_>>(){g.vertex.remove_empty_page(page)?;self.resources.retire(g.vpages.remove(&page).unwrap())?;}self.resources.collect(fence,completed)?;
   let sample=self.resources.allocate_buffer(48,vk::BufferUsageFlags::VERTEX_BUFFER)?;let requirement=self.resources.allocation_bytes(sample).ok_or("Missing exact page")?;self.resources.retire(sample)?;self.resources.collect(fence,completed)?;
   let before=self.budget.ledger.lock().unwrap().total_committed();let previous=self.budget.ledger.lock().unwrap().total_limit;self.budget.ledger.lock().unwrap().total_limit=Some(before.checked_add(requirement).ok_or("Exact cap overflow")?);let restore=PressureRestore{budget:self.budget.clone(),previous};
   let exact=Gpu::reserve(&mut g.vertex,&mut g.vpages,&mut g.next_page,&mut self.resources,48,vk::BufferUsageFlags::VERTEX_BUFFER,&mut fresh,true,PAGE,context,&mut g.index,&mut g.ipages,fence,completed,&mut g.metrics)?;
   if exact.range.bytes!=48||g.vertex.metrics().page_bytes!=48{return Err("Exact-size fallback failed".into());}g.vertex.rollback(exact)?;drop(restore);
   Ok(serde_json::json!({"passed":true,"large_request_bytes":40*1024*1024u64,"large_actual_allocation":actual,"replacement_bytes":48*1024*1024u64,"exact_requirement":requirement,"metrics":g.metrics}))
  })();
  // No queue submission occurs in this admission-only probe. All ranges are unpublished.
  for id in g.vpages.values().chain(g.ipages.values()){self.resources.retire(*id)?;}self.resources.collect(fence,completed)?;
  if self.budget.ledger.lock().unwrap().total_committed()!=base{return Err("Budget admission probe leaked charged ownership".into());}result
 }
 /// Cold engineering controls: smaller real pages and a real admission cap after
 /// N fully transferred candidates. Never fabricates a Vulkan result.
 pub fn configure_outline_arena_for_probe(&mut self,page_bytes:u64,pressure_after:Option<usize>)->Result<(),String>{if page_bytes<16||page_bytes>PAGE||page_bytes%4!=0||pressure_after==Some(0){return Err("Invalid outline arena probe controls".into());}if self.arena.metrics().pages>0&&self.arena.page_bytes!=page_bytes{return Err("Cannot change live arena page size".into());}self.arena.page_bytes=page_bytes;self.arena.pressure_after=pressure_after;Ok(())}
 pub fn verify_outline_candidate_rollback_for_probe(&mut self,context:&VulkanContext,fence:vk::Fence,completed:u64,handle:&Handle)->Result<(),String>{
  let (chunks,serial,pose,old_ids)={let s=handle.inner.state.lock().unwrap();(s.chunks.clone(),s.serial,s.pose,s.ready.iter().map(|r|r.as_ref().map(|r|r.id)).collect::<Vec<_>>())};
  if chunks.len()!=2||chunks.iter().any(Option::is_none){return Err("Rollback probe needs two already drawn chunks".into());}
  let before=self.arena.metrics();let charged_before=self.budget.ledger.lock().unwrap().total_committed();
  let mut modified=Vec::new();for chunk in chunks.iter().flatten(){let mut vertices=chunk.geometry.vertices().to_vec();for v in &mut vertices{v.position[0]+=10.;}modified.push(Some(Arc::new(PlaneOutlineChunk{geometry:Geometry::new(vertices,chunk.geometry.indices().to_vec())?,batches:chunk.batches.clone(),strips:chunk.strips})));}
  let modified_pose=Pose{translation:[pose.translation[0]+7.,pose.translation[1]+11.],..pose};handle.update(ChunkFrame{slots:&modified,changed:&[0,1],replacement:false,serial:serial+1},modified_pose)?;
  self.configure_outline_arena_for_probe(64,Some(1))?;
  let primitive=egui::ClippedPrimitive{clip_rect:egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(800.,600.)),primitive:egui::epaint::Primitive::Callback(egui::PaintCallback{rect:egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(800.,600.)),callback:handle.callback.clone()})};
  let error=self.arena.prepare(context,&mut self.resources,&self.budget,&self.pipeline,&[primitive],completed,completed+1,fence).err().ok_or("Later changed chunk budget pressure unexpectedly succeeded")?;
  let after=self.arena.metrics();let actual_ids=handle.inner.state.lock().unwrap().ready.iter().map(|r|r.as_ref().map(|r|r.id)).collect::<Vec<_>>();
  if after.uploads!=before.uploads+2||after.candidate_rollbacks!=before.candidate_rollbacks+1||actual_ids!=old_ids||handle.inner.state.lock().unwrap().ready_pose!=pose{return Err("Pressure did not retain old drawable after exactly one transferred candidate".into());}
  self.resources.collect(fence,completed)?;if self.budget.ledger.lock().unwrap().total_committed()!=charged_before{return Err("Failed candidate ledger not restored".into());}
  // Engineering-only suspend automatic retry so the real previous published
  // callback is drawn while desired geometry AND pose remain the rejected values.
  // A new explicit handle publication clears this hold and retries normally.
  {let mut s=handle.inner.state.lock().unwrap();s.hold_failed_for_probe=true;}
  eprintln!("PASS actual outline later-chunk pressure: {}",serde_json::json!({"error":error,"before":before,"after":after,"old_drawable_ids":old_ids,"ledger_restored":true}));Ok(())
 }
}

#[cfg(test)]mod budget_tests{use super::*;
 #[test]fn only_shared_ledger_refusals_allow_retry(){for e in ["GPU allocation 64 exceeds global application budget 32","GPU heap 0: allocation 64 exceeds application budget 32; reserved 0; committed 0","A error occured when using the allocator: GPU heap 0: allocation 64 exceeds live budget 32; driver usage 0"]{assert!(budget_admission(e));}for e in ["ERROR_DEVICE_LOST","ERROR_OUT_OF_DEVICE_MEMORY","GPU ownership quarantined","Heap usage overflow"]{assert!(!budget_admission(e));}}
}
