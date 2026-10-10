// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable per-plane CPU chunks. GPU arena consumption is a separate stage.
//! This producer never concatenates unchanged geometry or creates GPU resources.
use std::{collections::HashSet,sync::Arc};
use ingetrazo_core::{scene::{Scene,SelectionItem},image_plane::ImagePlane};
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch,EncodedRgba};
#[derive(Clone,PartialEq)]struct ViewKey{view:u64,matrix:[u32;16],size:[u32;2],density:u32}
pub struct PlaneOutlineChunk{pub geometry:Arc<Geometry>,pub batches:Arc<[Batch]>,pub strips:u64}
#[derive(Default,Debug,Clone,serde::Serialize)]pub struct ChunkMetrics{
 pub cold_rebuilds:u64,pub dirty_updates:u64,pub plane_builds:u64,pub scene_planes_scanned:u64,
 pub journal_records_examined:u64,pub changed_slots:u64,pub byte_identical_reuses:u64,pub refusals:u64,
}
/// Borrowed publication; slot order is original scene/plane order. The immutable
/// chunk Arcs survive replacement. No copied outer collection on a pose update.
pub struct ChunkFrame<'a>{pub slots:&'a[Option<Arc<PlaneOutlineChunk>>],pub changed:&'a[usize],pub replacement:bool,pub serial:u64}
#[derive(Default)]pub struct OutlineChunkCache{
 view:Option<ViewKey>,epoch:u64,publication:u64,serial:u64,replacement:bool,slots:Vec<Option<Arc<PlaneOutlineChunk>>>,
 selected:Vec<usize>,hidden_layers:HashSet<String>,changed:Vec<usize>,scratch:Vec<usize>,
 pub metrics:ChunkMetrics,
}
fn make_chunk(image:&ImagePlane,selected:bool,matrix:&[f32;16],size:[u32;2],density:f32,hidden:&HashSet<String>)->Result<Option<Arc<PlaneOutlineChunk>>,String>{
 if hidden.contains(image.layer.as_deref().unwrap_or("Layer 0")){return Ok(None);}
 let o=image.origin.as_vec3();let u=image.u.as_vec3();let v=image.v.as_vec3();let corners=[o,o+u,o+u+v,o+v];
 if corners.iter().any(|p|!p.is_finite()){return Err("Nonfinite image outline frame".into());}
 let(mut vertices,mut indices,mut batches)=(Vec::new(),Vec::new(),Vec::new());let mut count_strips=0u64;
 for edge in 0..4{
  let Some([a,b])=ingetrazo_render::image_outline::projected_segment(matrix,corners[edge].to_array(),corners[(edge+1)%4].to_array(),size)else{continue;};
  let strips=ingetrazo_render::qt_gl_image_line::strips(a,b,selected,size,100_000)?;let first=indices.len()as u32;count_strips+=strips.len()as u64;
  for strip in strips{
   if vertices.len().checked_add(8).and_then(|v|v.checked_mul(12)).and_then(|v|indices.len().checked_add(18).and_then(|i|i.checked_mul(4)).and_then(|i|v.checked_add(i))).is_none_or(|bytes|bytes>8*1024*1024){return Err("Per-plane outline chunk exceeds8MiB admission".into());}
   let base=vertices.len()as u32;for position in strip{vertices.push(Vertex{position:position.map(|v|v/density),coverage:1.});}
   for i in 0..6{indices.extend_from_slice(&[base+i,base+i+1,base+i+2]);}
  }
  let count=indices.len()as u32-first;if count>0{batches.push(Batch{first,count,color:EncodedRgba::from_bytes(if selected{[243,115,41,255]}else{[120,140,165,255]},1.)?});}
 }
 if indices.is_empty(){return Ok(None);}
 Ok(Some(Arc::new(PlaneOutlineChunk{geometry:Geometry::new(vertices,indices)?,batches:batches.into(),strips:count_strips})))
}
fn same_chunk(a:&Option<Arc<PlaneOutlineChunk>>,b:&Option<Arc<PlaneOutlineChunk>>)->bool{match(a,b){
 (None,None)=>true,
 (Some(a),Some(b))=>bytemuck::cast_slice::<Vertex,u8>(a.geometry.vertices())==bytemuck::cast_slice::<Vertex,u8>(b.geometry.vertices())&&a.geometry.indices()==b.geometry.indices()&&a.batches.as_ref()==b.batches.as_ref(),
 _=>false,
}}
impl OutlineChunkCache{
 pub fn prepare(&mut self,scene:&Scene,matrix:[f32;16],size:[u32;2],density:f32)->Result<ChunkFrame<'_>,String>{
  let result=self.update(scene,matrix,size,density);if let Err(error)=result{self.metrics.refusals+=1;return Err(error);}
  Ok(ChunkFrame{slots:&self.slots,changed:&self.changed,replacement:self.replacement,serial:self.serial})
 }
 fn update(&mut self,scene:&Scene,matrix:[f32;16],size:[u32;2],density:f32)->Result<(),String>{
  if !density.is_finite()||density<=0.||matrix.iter().any(|v|!v.is_finite()){return Err("Invalid image outline view".into());}
  let view=ViewKey{view:scene.view_version,matrix:matrix.map(f32::to_bits),size,density:density.to_bits()};
  let count=scene.selection.iter().filter(|v|matches!(v,SelectionItem::ImagePlane(i)if *i<scene.image_planes.len())).count();
  let selection_same=count==self.selected.len()&&self.selected.iter().all(|i|scene.selection.contains(&SelectionItem::ImagePlane(*i)));
  let publication=scene.image_planes.publication_revision();let epoch=scene.image_planes.membership_epoch();
  if self.view.as_ref()==Some(&view)&&self.epoch==epoch&&self.publication==publication&&selection_same{self.changed.clear();self.replacement=false;return Ok(());}
  let next_serial=self.serial.checked_add(1).ok_or("Image outline publication serial exhausted")?;
  let delta=scene.image_planes.pose_changes_since(self.epoch,self.publication);
  let cold=self.view.as_ref()!=Some(&view)||self.epoch!=epoch||self.slots.len()!=scene.image_planes.len()||delta.is_none();
  let new_selected=if selection_same{None}else{Some(scene.selection.iter().filter_map(|v|if let SelectionItem::ImagePlane(i)=v{(*i<scene.image_planes.len()).then_some(*i)}else{None}).collect::<Vec<_>>())};
  if cold{
   let hidden=scene.layers.iter().filter(|l|!l.visible).map(|l|l.name.clone()).collect::<HashSet<_>>();
   let mut slots=Vec::with_capacity(scene.image_planes.len());
   for(index,image)in scene.image_planes.iter().enumerate(){self.metrics.scene_planes_scanned+=1;self.metrics.plane_builds+=1;slots.push(make_chunk(image,scene.selection.contains(&SelectionItem::ImagePlane(index)),&matrix,size,density,&hidden)?);}
   // No publication changes until every candidate chunk has succeeded.
   self.scratch.clear();self.scratch.extend(0..slots.len());self.slots=slots;self.hidden_layers=hidden;
   self.metrics.cold_rebuilds+=1;self.replacement=true;
  }else{
   self.scratch.clear();let mut delta=delta.unwrap();self.scratch.extend(delta.by_ref());self.metrics.journal_records_examined+=delta.records_examined()as u64;
   if let Some(selected)=&new_selected{self.scratch.extend(self.selected.iter().copied());self.scratch.extend(selected.iter().copied());}
   self.scratch.sort_unstable();self.scratch.dedup();
   let mut candidates=Vec::with_capacity(self.scratch.len());
   for &index in &self.scratch{let image=scene.image_planes.get(index).ok_or("Image pose journal index outside current owner")?;self.metrics.plane_builds+=1;let candidate=make_chunk(image,scene.selection.contains(&SelectionItem::ImagePlane(index)),&matrix,size,density,&self.hidden_layers)?;candidates.push((index,candidate));}
   // Transactional publish: a later failed plane cannot replace earlier slots.
   self.scratch.clear();for(index,candidate)in candidates{if same_chunk(&self.slots[index],&candidate){self.metrics.byte_identical_reuses+=1;}else{self.slots[index]=candidate;self.scratch.push(index);}}
   self.metrics.dirty_updates+=1;self.replacement=false;
  }
  self.metrics.changed_slots+=self.scratch.len()as u64;std::mem::swap(&mut self.changed,&mut self.scratch);
  if let Some(selected)=new_selected{self.selected=selected;}
  self.view=Some(view);self.epoch=epoch;self.publication=publication;self.serial=next_serial;Ok(())
 }
}

#[cfg(test)]mod tests{
 use super::*;use glam::DVec3;
 fn scene()->Scene{let mut s=Scene::new();for x in [-0.5,-0.25,0.]{s.image_planes.push(ImagePlane::new("missing",DVec3::new(x,-0.5,0.),DVec3::new(0.4,0.,0.),DVec3::new(0.,0.4,0.)));}s}
 fn prepare(c:&mut OutlineChunkCache,s:&Scene){c.prepare(s,glam::Mat4::IDENTITY.to_cols_array(),[800,600],1.).unwrap();}
 #[test]fn varying_pose_rebuilds_only_dirty_chunk_and_preserves_unchanged_arcs(){
  let mut s=scene();let mut c=OutlineChunkCache::default();prepare(&mut c,&s);let a=c.slots[0].clone().unwrap();let b=c.slots[2].clone().unwrap();let scans=c.metrics.scene_planes_scanned;let builds=c.metrics.plane_builds;
  for n in 1..=100{s.image_planes.pose_mut(1).unwrap().set_origin(DVec3::new(-0.25+n as f64*0.0001,-0.5,0.));prepare(&mut c,&s);assert_eq!(c.changed,vec![1]);assert!(!c.replacement);assert!(Arc::ptr_eq(&a,c.slots[0].as_ref().unwrap()));assert!(Arc::ptr_eq(&b,c.slots[2].as_ref().unwrap()));}
  assert_eq!(c.metrics.scene_planes_scanned,scans);assert_eq!(c.metrics.plane_builds,builds+100);assert!(c.metrics.journal_records_examined<=6400);
  let old=c.slots[1].clone().unwrap();let origin=s.image_planes[1].origin;s.image_planes.pose_mut(1).unwrap().set_origin(origin+DVec3::new(1e-10,0.,0.));prepare(&mut c,&s);assert!(c.changed.is_empty());assert!(Arc::ptr_eq(&old,c.slots[1].as_ref().unwrap()));
 }
 #[test]fn failed_later_plane_preserves_all_published_slots_and_metadata(){
  let mut s=scene();let mut c=OutlineChunkCache::default();prepare(&mut c,&s);let old=c.slots.iter().map(|v|v.as_ref().unwrap().geometry.id()).collect::<Vec<_>>();let serial=c.serial;let publication=c.publication;let changed=c.changed.clone();
  s.image_planes.pose_mut(0).unwrap().set_origin(DVec3::new(-0.4,-0.5,0.));s.image_planes.pose_mut(2).unwrap().set_origin(DVec3::new(f64::NAN,0.,0.));
  assert!(c.prepare(&s,glam::Mat4::IDENTITY.to_cols_array(),[800,600],1.).is_err());assert_eq!(c.serial,serial);assert_eq!(c.publication,publication);assert_eq!(c.changed,changed);assert_eq!(c.slots.iter().map(|v|v.as_ref().unwrap().geometry.id()).collect::<Vec<_>>(),old);
  s.image_planes.pose_mut(2).unwrap().set_origin(DVec3::new(0.1,-0.5,0.));prepare(&mut c,&s);assert_eq!(c.changed,vec![0,2]);
  c.serial=u64::MAX;s.image_planes.pose_mut(0).unwrap().set_origin(DVec3::new(-0.3,-0.5,0.));let pub_before=c.publication;assert!(c.prepare(&s,glam::Mat4::IDENTITY.to_cols_array(),[800,600],1.).is_err());assert_eq!(c.publication,pub_before);
 }
 #[test]fn overflow_replacement_selection_and_visibility_take_correct_paths(){
  let mut s=scene();let mut c=OutlineChunkCache::default();prepare(&mut c,&s);let old0=c.slots[0].clone().unwrap();let scans=c.metrics.scene_planes_scanned;
  s.image_planes.pose_mut(0).unwrap().set_origin(DVec3::new(-0.4,-0.5,0.));for n in 1..=64{s.image_planes.pose_mut(1).unwrap().set_origin(DVec3::new(-0.25+n as f64*0.0001,-0.5,0.));}prepare(&mut c,&s);assert!(c.replacement);assert_eq!(c.metrics.scene_planes_scanned,scans+3);assert_ne!(old0.geometry.vertices(),c.slots[0].as_ref().unwrap().geometry.vertices());
  let scans=c.metrics.scene_planes_scanned;s.selection.insert(SelectionItem::ImagePlane(1));prepare(&mut c,&s);assert_eq!(c.changed,vec![1]);assert_eq!(c.metrics.scene_planes_scanned,scans);assert_eq!(c.slots[1].as_ref().unwrap().batches[0].color,EncodedRgba::from_bytes([243,115,41,255],1.).unwrap());
  s.image_planes.reverse();prepare(&mut c,&s);assert!(c.replacement);let epoch=c.epoch;s.image_planes=s.image_planes.clone();prepare(&mut c,&s);assert!(c.replacement);assert_ne!(c.epoch,epoch);
  s.layers.push(ingetrazo_core::layers::Layer::new("Hidden"));s.layers.last_mut().unwrap().visible=false;s.image_planes[0].layer=Some("Hidden".into());s.view_version+=1;prepare(&mut c,&s);assert!(c.slots[0].is_none());
 }
 #[test]fn fifty_thousand_planes_varied_drag(){
 let mut s=scene();for _ in 3..50000{s.image_planes.push(ImagePlane::new("missing",DVec3::new(100.,100.,0.),DVec3::X,DVec3::Y));}
 let mut c=OutlineChunkCache::default();prepare(&mut c,&s);let scans=c.metrics.scene_planes_scanned;let builds=c.metrics.plane_builds;let fixed=c.slots[0].clone().unwrap();let start=std::time::Instant::now();
 for n in 1..=2000{s.image_planes.pose_mut(1).unwrap().set_origin(DVec3::new(-0.25+n as f64*0.00001,-0.5,0.));prepare(&mut c,&s);assert_eq!(c.changed,[1]);assert!(Arc::ptr_eq(&fixed,c.slots[0].as_ref().unwrap()));}
 assert_eq!(c.metrics.scene_planes_scanned,scans);assert_eq!(c.metrics.plane_builds,builds+2000);assert!(c.metrics.journal_records_examined<=128000);
 eprintln!("CPU producer only: 50000 planes, 2000 varied updates, {}us, warm scene scans0, chunk builds2000, journal records{}; immutable changed geometry allocates; GPU not integrated",start.elapsed().as_micros(),c.metrics.journal_records_examined);
 }

}
