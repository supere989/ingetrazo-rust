//! Ordered immutable outline chunks in fresh GPU page ranges. One callback.
use super::*;
use crate::{image_outline_arena::{Planner,Ticket},image_outline_chunks::{ChunkFrame,PlaneOutlineChunk}};
use std::{collections::BTreeMap,sync::Weak};
const PAGE:u64=1024*1024;
const CAP:u64=32*1024*1024;
struct State{chunks:Vec<Option<Arc<PlaneOutlineChunk>>>,dirty:Vec<usize>,pose:Pose,ready:Vec<Option<Ready>>,serial:u64}
struct Paint{owner:Weak<Paint>,device:vk::Device,pipeline:Arc<Mutex<Pipeline>>,counters:Arc<Counters>,state:Mutex<State>}
#[derive(Clone)]pub struct Handle{inner:Arc<Paint>,callback:Arc<VulkanPaintCallback>}
impl Factory{pub fn outline_arena(&self)->Handle{let inner=Arc::new_cyclic(|owner|Paint{owner:owner.clone(),device:self.device,pipeline:self.pipeline.clone(),counters:self.counters.clone(),state:Mutex::new(State{chunks:Vec::new(),dirty:Vec::new(),pose:Pose::default(),ready:Vec::new(),serial:0})});let callback=Arc::new(VulkanPaintCallback(inner.clone()));Handle{inner,callback}}}
impl Handle{
 pub fn update(&self,frame:ChunkFrame<'_>,pose:Pose)->Result<(),String>{pose.validate()?;let mut s=self.inner.state.lock().map_err(|_|"Outline arena state poisoned")?;
  for &index in frame.changed{if index>=frame.slots.len(){return Err("Outline changed slot outside publication".into());}}
  if frame.serial!=s.serial{if frame.replacement||s.chunks.len()!=frame.slots.len(){s.chunks=frame.slots.to_vec();s.dirty.clear();s.dirty.extend(0..frame.slots.len());}else{for &i in frame.changed{s.chunks[i]=frame.slots[i].clone();s.dirty.push(i);}s.dirty.sort_unstable();s.dirty.dedup();}s.serial=frame.serial;}s.pose=pose;Ok(())
 }
 pub fn shape(&self,rect:egui::Rect)->egui::Shape{egui::PaintCallback{rect,callback:self.callback.clone()}.into()}
}
impl VulkanPaint for Paint{fn as_any(&self)->&dyn Any{self}fn paint(&self,info:&egui::PaintCallbackInfo,device:&Device,command:vk::CommandBuffer)->RendererResult<()>{if device.handle()!=self.device{return Err(egui_ash_renderer::RendererError::Init("Outline arena device mismatch".into()));}let s=self.state.lock().unwrap();let p=self.pipeline.lock().unwrap();record_ready(info,device,command,&p,&self.counters,s.pose,s.ready.iter().flatten())}}
struct Slot{vertex:Ticket,index:Ticket}
#[derive(Default,Clone,Debug,serde::Serialize)]pub struct Metrics{pub pages:u64,pub page_bytes:u64,pub uploads:u64,pub upload_bytes:u64,pub candidate_rollbacks:u64,pub preserved_failures:u64,pub live_slots:u64,pub retired_bytes:u64}
#[derive(Default)]pub(super)struct Gpu{vertex:Planner,index:Planner,vpages:BTreeMap<u64,BufferId>,ipages:BTreeMap<u64,BufferId>,slots:Vec<Option<Slot>>,paint:Option<Weak<Paint>>,next_page:u64,metrics:Metrics}
impl Gpu{
 pub(super)fn metrics(&self)->Metrics{let mut m=self.metrics.clone();m.pages=(self.vpages.len()+self.ipages.len())as u64;m.page_bytes=self.vertex.metrics().page_bytes+self.index.metrics().page_bytes;m.retired_bytes=self.vertex.metrics().retired_bytes+self.index.metrics().retired_bytes;m.live_slots=self.slots.iter().flatten().count()as u64;m}
 fn reserve(planner:&mut Planner,pages:&mut BTreeMap<u64,BufferId>,next:&mut u64,resources:&mut Resources,bytes:u64,usage:vk::BufferUsageFlags,new_pages:&mut Vec<(bool,u64)>,vertex:bool)->Result<Ticket,String>{
  if let Some(t)=planner.reserve(bytes,if vertex{12}else{4})?{return Ok(t);}
  let capacity=bytes.checked_add(PAGE-1).map(|v|v/PAGE*PAGE).ok_or("Outline page size overflow")?;
  if planner.metrics().page_bytes.checked_add(capacity).is_none_or(|v|v>CAP){return Err("Outline arena capacity admission exceeded".into());}
  let id=next.checked_add(1).ok_or("Outline page identity exhausted")?;let buffer=resources.allocate_buffer(capacity,usage)?;planner.add_page(id,capacity)?;pages.insert(id,buffer);*next=id;new_pages.push((vertex,id));planner.reserve(bytes,if vertex{12}else{4})?.ok_or_else(||"New outline page reservation failed".into())
 }
 pub(super)fn prepare(&mut self,context:&VulkanContext,resources:&mut Resources,pipeline:&Arc<Mutex<Pipeline>>,primitives:&[egui::ClippedPrimitive],completed:u64,next:u64)->Result<(),String>{
  self.vertex.collect(completed)?;self.index.collect(completed)?;
  let mut active=None;
  for primitive in primitives{let egui::epaint::Primitive::Callback(c)=&primitive.primitive else{continue;};let Some(cb)=c.callback.downcast_ref::<VulkanPaintCallback>()else{continue;};let Some(p)=cb.0.as_any().downcast_ref::<Paint>()else{continue;};if !Arc::ptr_eq(&p.pipeline,pipeline){return Err("Outline arena belongs to different host".into());}if active.is_some(){return Err("One outline arena callback per frame is required".into());}active=Some(p);}
  let changed_owner=match(&self.paint,active){(Some(old),Some(p))=>old.as_ptr()!=p as *const Paint,(None,None)=>false,_=>true};
  let Some(paint)=active else{for slot in self.slots.iter_mut().filter_map(Option::take){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}self.slots.clear();if let Some(old)=self.paint.take().and_then(|p|p.upgrade()){old.state.lock().unwrap().ready.clear();}return Ok(());};
  let mut s=paint.state.lock().map_err(|_|"Outline arena state poisoned")?;
  if changed_owner{s.dirty.clear();let len=s.chunks.len();s.dirty.extend(0..len);}
  let mut candidates=Vec::new();let mut new_pages=Vec::new();
  let result=(||{
   for &i in &s.dirty{let Some(chunk)=s.chunks[i].as_ref()else{candidates.push((i,None,None));continue;};chunk.geometry.validate_batches(&chunk.batches)?;if !chunk.geometry.is_opaque(&chunk.batches){return Err("Outline opaque admission changed".into());}
    let vertex=Self::reserve(&mut self.vertex,&mut self.vpages,&mut self.next_page,resources,(chunk.geometry.vertices().len()*std::mem::size_of::<ingetrazo_render::cad2d::Vertex>())as u64,vk::BufferUsageFlags::VERTEX_BUFFER,&mut new_pages,true)?;
    let index=match Self::reserve(&mut self.index,&mut self.ipages,&mut self.next_page,resources,(chunk.geometry.indices().len()*4)as u64,vk::BufferUsageFlags::INDEX_BUFFER,&mut new_pages,false){Ok(v)=>v,Err(e)=>{self.vertex.rollback(vertex)?;return Err(e);}};
    // Tickets are Pending in the exclusively owned planner, with disjoint fresh
    // ranges. No publication/mark_used occurs until every transfer succeeds.
    candidates.push((i,Some(Slot{vertex,index}),None));
    unsafe{resources.upload_fresh_range(context,self.vpages[&vertex.range.page],vertex.range.offset,bytemuck::cast_slice(chunk.geometry.vertices()))?;resources.upload_fresh_range(context,self.ipages[&index.range.page],index.range.offset,bytemuck::cast_slice(chunk.geometry.indices()))?;}
    self.metrics.uploads+=2;self.metrics.upload_bytes+=vertex.range.bytes+index.range.bytes;
    let ready=Ready{id:chunk.geometry.id(),vertex:resources.buffer(self.vpages[&vertex.range.page]).ok_or("Missing outline vertex page")?,index:resources.buffer(self.ipages[&index.range.page]).ok_or("Missing outline index page")?,first_index:u32::try_from(index.range.offset/4).map_err(|_|"Outline index offset overflow")?,base_vertex:i32::try_from(vertex.range.offset/std::mem::size_of::<ingetrazo_render::cad2d::Vertex>()as u64).map_err(|_|"Outline vertex offset overflow")?,batches:chunk.batches.clone(),premultiplied:prepare_palette(&chunk.batches)?,pose:s.pose,ordered_draws:None,opaque:true};
    candidates.last_mut().unwrap().2=Some(ready);
   }Ok(())
  })();
  if let Err(error)=result{self.metrics.preserved_failures+=1;if resources.check_submission().is_ok(){for(_,slot,_)in candidates{if let Some(slot)=slot{self.vertex.rollback(slot.vertex)?;self.index.rollback(slot.index)?;self.metrics.candidate_rollbacks+=1;}}for(v,id)in new_pages{let(planner,pages)=if v{(&mut self.vertex,&mut self.vpages)}else{(&mut self.index,&mut self.ipages)};planner.remove_empty_page(id)?;resources.retire(pages.remove(&id).unwrap())?;}}return Err(error);}
  if changed_owner{for slot in self.slots.iter_mut().filter_map(Option::take){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}self.slots.clear();if let Some(old)=self.paint.take().and_then(|p|p.upgrade()){old.state.lock().unwrap().ready.clear();}}
  if self.slots.len()>s.chunks.len(){for slot in self.slots.drain(s.chunks.len()..).flatten(){self.vertex.retire(slot.vertex)?;self.index.retire(slot.index)?;}}
  let len=s.chunks.len();self.slots.resize_with(len,||None);s.ready.resize_with(len,||None);
  for(i,slot,ready)in candidates{if let Some(old)=self.slots[i].take(){self.vertex.retire(old.vertex)?;self.index.retire(old.index)?;}if let Some(slot)=&slot{self.vertex.publish(slot.vertex)?;self.index.publish(slot.index)?;}self.slots[i]=slot;s.ready[i]=ready;}
  s.dirty.clear();
  for slot in self.slots.iter().flatten(){self.vertex.mark_used(slot.vertex,next)?;self.index.mark_used(slot.index,next)?;resources.mark_used(self.vpages[&slot.vertex.range.page],next)?;resources.mark_used(self.ipages[&slot.index.range.page],next)?;}
  self.paint=Some(paint.owner.clone());
  self.metrics.pages=(self.vpages.len()+self.ipages.len())as u64;self.metrics.page_bytes=self.vertex.metrics().page_bytes+self.index.metrics().page_bytes;self.metrics.live_slots=self.slots.iter().flatten().count()as u64;self.metrics.retired_bytes=self.vertex.metrics().retired_bytes+self.index.metrics().retired_bytes;Ok(())
 }
}
