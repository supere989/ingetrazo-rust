// SPDX-License-Identifier: GPL-3.0-or-later
//! Cached source image-border CAD overlay. No decoded images or model buffers.
use std::sync::Arc;
use ingetrazo_core::{scene::{Scene,SelectionItem},camera::OrbitCamera};
use ingetrazo_render::cad2d::{Batch,EncodedRgba,Geometry,Pose,Vertex};
#[derive(Clone,PartialEq)]struct Key{publication:u64,view:u64,matrix:[u32;16],size:[u32;2],density:u32}
#[derive(Default)]pub struct ImageOutlineCache{
 key:Option<Key>,selected:Vec<usize>,geometry:Option<Arc<Geometry>>,batches:Option<Arc<[Batch]>>,
 handle:Option<crate::composer_backend::Handle>,pub cold_builds:u64,pub generated_spans:u64,
}
impl ImageOutlineCache{
 pub fn prepare(&mut self,scene:&Scene,matrix:[f32;16],size:[u32;2],density:f32)->Result<Option<(Arc<Geometry>,Arc<[Batch]>)>,String>{
  if !density.is_finite()||density<=0.{return Err("Invalid image outline density".into());}
  let key=Key{publication:scene.image_planes.publication_revision(),view:scene.view_version,matrix:matrix.map(f32::to_bits),size,density:density.to_bits()};
  // Selection is public legacy state. Validate only selected identities without
  // allocating or scanning all image planes on an unchanged frame.
  let count=scene.selection.iter().filter(|v|matches!(v,SelectionItem::ImagePlane(_))).count();
  if self.key.as_ref()==Some(&key)&&count==self.selected.len()&&self.selected.iter().all(|i|scene.selection.contains(&SelectionItem::ImagePlane(*i))){
   return Ok(self.geometry.clone().zip(self.batches.clone()));
  }
  let mut vertices=Vec::new();let mut indices=Vec::new();let mut batches=Vec::new();let mut ordered=Vec::new();let mut selected=Vec::with_capacity(count);let mut generated=0u64;
  for(index,image)in scene.image_planes.iter().enumerate(){
   let is_selected=scene.selection.contains(&SelectionItem::ImagePlane(index));if is_selected{selected.push(index);}
   let layer=image.layer.as_deref().unwrap_or("Layer 0");if scene.layers.iter().any(|l|l.name==layer&&!l.visible){continue;}
   let o=image.origin.as_vec3();let u=image.u.as_vec3();let v=image.v.as_vec3();let corners=[o,o+u,o+u+v,o+v];
   for edge in 0..4{
    let a=corners[edge].to_array();let b=corners[(edge+1)%4].to_array();
    let Some([a,b])=ingetrazo_render::image_outline::projected_segment(&matrix,a,b,size)else{continue;};
    let spans=if is_selected{ingetrazo_render::qt_selected_line::rasterize(a,b,size,100_000)?}else{ingetrazo_render::qt_cosmetic::rasterize_dashed_line_clipped(a,b,size,100_000)?};
    let first=indices.len()as u32;
    let ranges=ingetrazo_render::qt_cosmetic::disjoint_ranges(&spans,100_000)?;
    for range in ranges{ordered.push(first+(range.start as u32)*6..first+(range.end as u32)*6);}
    generated+=spans.len()as u64;
    for span in spans{
     if vertices.len().checked_add(4).and_then(|v|v.checked_mul(12)).and_then(|v|indices.len().checked_add(6).and_then(|i|i.checked_mul(4)).and_then(|i|v.checked_add(i))).is_none_or(|bytes|bytes>8*1024*1024){return Err("Image outline geometry exceeds8MiB admission".into());}
     let base=vertices.len()as u32;let x=span.x as f32;let y=span.y as f32;let end=x+span.len as f32;
     for position in [[x,y],[end,y],[end,y+1.],[x,y+1.]]{vertices.push(Vertex{position:position.map(|v|v/density),coverage:f32::from(span.coverage)/255.});}
     indices.extend_from_slice(&[base,base+1,base+2,base,base+2,base+3]);
    }
    let count=indices.len()as u32-first;
    if count>0{batches.push(Batch{first,count,color:EncodedRgba::from_bytes(if is_selected{[243,115,41,255]}else{[120,140,165,255]},1.)?});}
   }
  }
  let geometry=if indices.is_empty(){None}else{Some(Geometry::new_with_ordered_draws(vertices,indices,ordered)?)};
  let batches:Option<Arc<[Batch]>>=geometry.as_ref().map(|_|batches.into());
  self.key=Some(key);self.selected=selected;self.geometry=geometry;self.batches=batches;self.cold_builds+=1;self.generated_spans+=generated;
  Ok(self.geometry.clone().zip(self.batches.clone()))
 }
 pub fn paint(&mut self,scene:&Scene,camera:&OrbitCamera,painter:&egui::Painter,rect:egui::Rect,factory:Option<&crate::composer_backend::Factory>)->Result<(),String>{
  let Some(factory)=factory else{return Ok(());};
  let density=painter.ctx().pixels_per_point();let size=[(rect.width()*density).round()as u32,(rect.height()*density).round()as u32];
  let matrix=(camera.projection_matrix()*camera.view_matrix()).as_mat4().to_cols_array();
  let Some((geometry,batches))=self.prepare(scene,matrix,size,density)?else{self.handle=None;return Ok(());};
  let pose=Pose{scale:[1.;2],translation:[rect.min.x,rect.min.y]};
  if let Some(handle)=&self.handle{handle.update(geometry,batches,pose)?;}else{self.handle=Some(factory.handle(geometry,batches,pose)?);}
  painter.add(self.handle.as_ref().unwrap().shape(rect));Ok(())
 }
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn cached_overlay_preserves_identity_and_handles_public_selection_changes(){
  let mut scene=Scene::new();
  for x in [-0.5,0.]{scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::new("missing-image",glam::DVec3::new(x,-0.5,0.),glam::DVec3::X,glam::DVec3::Y));}
  let matrix=glam::Mat4::IDENTITY.to_cols_array();let mut cache=ImageOutlineCache::default();
  let(a,paint)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();
  for _ in 0..100{let(b,colors)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();assert!(Arc::ptr_eq(&a,&b));assert!(Arc::ptr_eq(&paint,&colors));}
  assert_eq!(cache.cold_builds,1);
  // Source border is independent of image decode success and opacity.
  scene.selection.insert(SelectionItem::ImagePlane(0));
  let(selected,_)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();assert!(!Arc::ptr_eq(&a,&selected));
  scene.selection.clear();scene.selection.insert(SelectionItem::ImagePlane(1));
  let(other,_)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();assert!(!Arc::ptr_eq(&selected,&other));
  let builds=cache.cold_builds;assert!(cache.prepare(&scene,matrix,[800,600],f32::NAN).is_err());assert_eq!(cache.cold_builds,builds);
  let(still,_)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();assert!(Arc::ptr_eq(&other,&still));
  scene.image_planes.pose_mut(1).unwrap().set_origin(glam::DVec3::new(0.1,-0.5,0.));
  let(moved,_)=cache.prepare(&scene,matrix,[800,600],1.).unwrap().unwrap();assert!(!Arc::ptr_eq(&other,&moved));
 }
}
