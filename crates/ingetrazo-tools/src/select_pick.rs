//! Select-only edge index. Drawing/snapping references retain their separate policy.
use glam::{DVec2,DVec3};
use ingetrazo_core::{scene::Scene,screen_pick::PixelViewport,math::{Ray},presentation::FaceMode};
#[derive(Default,Clone)]
pub(crate) struct ClickIndex { key:Option<(usize,u64)>, model_revision:u64, visible:Option<std::sync::Arc<ingetrazo_core::visible_ray_index::VisibleRayIndex>>, pub builds:u64 }
impl std::fmt::Debug for ClickIndex {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{f.debug_struct("ClickIndex").field("builds",&self.builds).finish()}}
impl ClickIndex {
 pub fn note_model_revision(&mut self,revision:u64){if self.model_revision!=revision{self.model_revision=revision;self.key=None;self.visible=None;}}
 fn prepare(&mut self,scene:&Scene) {
  let key=(scene as *const Scene as usize,scene.version);
  if self.key==Some(key){return;}
  self.visible=ingetrazo_core::visible_ray_index::VisibleRayIndex::new(scene,64*1024*1024).ok().map(std::sync::Arc::new);
  self.key=Some(key);self.builds+=1;
 }
 pub fn edge(&mut self,scene:&Scene,viewport:PixelViewport,cursor:DVec2,eye:DVec3)->Option<usize> {
  self.prepare(scene);
  // Source examines at most eight candidates, ordered by screen distance.
  let mut near=[(f64::INFINITY,usize::MAX,DVec3::ZERO);8];
  for (id,e) in scene.mesh.edges.iter().enumerate(){
   if (e.hidden&&!scene.show_hidden_geometry)||!scene.is_layer_selectable(e.layer.as_deref())||(e.soft&&!scene.show_hidden_geometry){continue;}
   let a=scene.mesh.vertices[e.v0].position;let b=scene.mesh.vertices[e.v1].position;
   if scene.section_segment(a,b).is_none(){continue;}
   let (Some(pa),Some(pb))=(viewport.project_for_snapping(a),viewport.project_for_snapping(b))else{continue;};
   let ab=pb-pa;let t=if ab.length_squared()>1e-12{((cursor-pa).dot(ab)/ab.length_squared()).clamp(0.,1.)}else{0.};
   let d=(pa+t*ab-cursor).length();if d>=8.{continue;}
   let item=(d,id,a+(b-a)*t);
   if let Some(index)=near.iter().position(|v|d<v.0){near[index..].rotate_right(1);near[index]=item;}
  }
  for (_,id,p) in near {if id==usize::MAX{return None;}
   if matches!(scene.display_style.face_mode,FaceMode::Xray|FaceMode::Wireframe){return Some(id);}
   let delta=p-eye;let dist=delta.length();if dist<1e-9{return Some(id);}let ray=Ray::new(eye,delta/dist);
   let nearest=if let Some(index)=&self.visible{index.distance(scene,&ray)}else{ingetrazo_core::visible_ray_index::VisibleRayIndex::direct_hit(scene,&ray).map(|h|h.2)};
   let blocked=nearest.is_some_and(|t|t<dist-1e-3);
   if !blocked{return Some(id);}
  }None
 }
}


#[cfg(test)]mod tests {
 use super::*;
 #[test]fn cached_click_occlusion_keeps_locked_solids_and_reuses_camera_independent_index(){
  let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::new(2.,0.,0.),DVec3::new(2.,2.,0.),DVec3::new(0.,2.,0.)].map(|p|scene.mesh.add_vertex(p));let face=scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
  let a=scene.mesh.add_vertex(DVec3::new(-1.,1.,-1.));let b=scene.mesh.add_vertex(DVec3::new(3.,1.,-1.));let edge=scene.mesh.add_edge(a,b).unwrap();
  let mut camera=ingetrazo_core::camera::OrbitCamera::new();camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);camera.perspective=false;camera.target=DVec3::new(1.,1.,0.);camera.distance=8.;let viewport=PixelViewport::new(&camera,1000.,600.);let cursor=viewport.project_for_snapping(DVec3::new(1.,1.,-1.)).unwrap();
  let mut index=ClickIndex::default();assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),None);assert_eq!(index.builds,1);
  scene.bump_view_version();assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),None);assert_eq!(index.builds,1);
  // Snapshot publication can replace mesh without changing Scene.version/address.
  let version=scene.version;scene.mesh.faces[face].attrs.insert("hidden".into(),serde_json::json!(true));assert_eq!(scene.version,version);
  index.note_model_revision(1);assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),Some(edge));assert_eq!(index.builds,2);
  scene.mesh.faces[face].attrs.remove("hidden");index.note_model_revision(2);assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),None);assert_eq!(index.builds,3);
  // A locked surface continues to occlude, while a hidden surface does not.
  scene.mesh.faces[face].layer=Some("solid".into());let mut layer=ingetrazo_core::layers::Layer::new("solid");layer.locked=true;scene.layers.push(layer);scene.bump_version();assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),None);
  index.visible=None;assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),None,"direct correctness fallback");
  scene.layers.last_mut().unwrap().visible=false;scene.bump_version();assert_eq!(index.edge(&scene,viewport,cursor,camera.eye()),Some(edge));
 }
}
