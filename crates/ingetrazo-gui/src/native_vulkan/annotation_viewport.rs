//! Persistent application annotation producer. Depth sample policy remains explicit
//! until the paired source MSAA edge policy is independently admitted.
use std::sync::Arc;
use super::{annotation_visibility::{prepare_query,Query},cad2d::{Factory,Handle}};
use ingetrazo_render::annotations;
use crate::annotation_plan::{build_plan,label_units};
#[cfg(test)] use crate::annotation_plan::typed_distance_text;
use crate::annotation_overlay_geometry::{Cache,Limits};
#[derive(PartialEq)]struct Key{hover:Option<ingetrazo_tools::base::AnnotationEditTarget>,revision:u64,scene_version:u64,factory:usize,view:u64,matrix:[f32;16],rect:[f32;4],density:f32,selected:Option<usize>,bypass_depth:bool,include_annotations:bool,presentation:bool,capture_scale:[u32;2],distance:Option<(f64,[f64;3])>,units:(&'static str,i32)}
pub(crate) struct Prepared{pub handle:Handle,pub queries:Arc<[Query]>,pub inverse:[f32;16],pub eye:[f32;3],pub forward:[f32;3]}
pub(crate) struct ViewportAnnotations{cache:Cache,current:Option<(Key,Prepared)>,empty:Option<Key>,style:serde_json::Value,vcb:String}
impl ViewportAnnotations{
 pub fn new(stencil:bool)->Result<Self,String>{Self::with_font_limits(stencil,ingetrazo_formats::annotation_font::Limits{faces:32,retained_font_bytes:32*1024*1024})}
 pub(crate) fn with_font_limits(stencil:bool,fonts:ingetrazo_formats::annotation_font::Limits)->Result<Self,String>{let mut result=Self{cache:Cache::new(fonts,Limits{cells:2_000_000,spans:200_000,geometry_bytes:32*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024})?,current:None,empty:None,style:serde_json::Value::Null,vcb:String::new()};if stencil{result.cache.enable_source_stencil();}Ok(result)}
 pub(crate) fn presentation_text(&mut self,text:&[crate::annotation_overlay_geometry::PresentationText])->Result<Arc<crate::annotation_overlay_geometry::Mesh>,String>{self.cache.prepare_presentation_text(text,1.,96.)}
 pub fn prepare<'a>(&'a mut self,app:&crate::IngeTrazoApp,factory:&Factory,rect:egui::Rect,density:f32,include_annotations:bool)->Result<Option<&'a Prepared>,String>{
  self.prepare_mode(app,factory,rect,density,include_annotations,false,[1.;2])
 }
 pub(crate) fn prepare_presentation<'a>(&'a mut self,app:&crate::IngeTrazoApp,factory:&Factory,rect:egui::Rect,density:f32,scale:[f32;2])->Result<Option<&'a Prepared>,String>{
  self.prepare_mode(app,factory,rect,density,true,true,scale)
 }
 fn prepare_mode<'a>(&'a mut self,app:&crate::IngeTrazoApp,factory:&Factory,rect:egui::Rect,density:f32,include_annotations:bool,presentation:bool,scale:[f32;2])->Result<Option<&'a Prepared>,String>{
  if !density.is_finite()||density<=0.||!rect.min.is_finite()||!rect.max.is_finite()||rect.width()<=0.||rect.height()<=0.{return Err("Annotation viewport/density outside admission".into());}
  let bypass_depth=matches!(app.style.visual_style,ingetrazo_render::style::VisualStyle::XRay|ingetrazo_render::style::VisualStyle::Wireframe);
  let snapshot=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Annotation camera arithmetic outside admission")?;
  let matrix=std::array::from_fn(|i|snapshot.matrix[i/4][i%4]);
  let units=label_units(&app.scene)?;
  let distance=if presentation{None}else{app.active_tool.distance_label()}.map(|(value,point)|(value,point.to_array()));
  if distance.is_some_and(|(value,point)|!value.is_finite()||point.iter().any(|p|!p.is_finite())){return Err("Tool label outside finite admission".into());}
  let key=Key{hover:app.active_tool.annotation_hover_target(&app.scene),revision:app.native_geometry_revision,scene_version:app.scene.version,factory:factory.cache_identity(),view:app.scene.view_version,matrix,rect:[rect.min.x,rect.min.y,rect.max.x,rect.max.y],density,selected:app.selected_dimension,bypass_depth,include_annotations,presentation,capture_scale:scale.map(f32::to_bits),distance,units};
  let raw=app.scene.dimension_style();
  let style_unchanged=self.style==*raw&&self.vcb==app.vcb_text;
  if style_unchanged&&self.empty.as_ref()==Some(&key){return Ok(None);}
  if style_unchanged&&self.current.as_ref().is_some_and(|(old,_)|*old==key){return Ok(self.current.as_ref().map(|(_,ready)|ready));}
  // Invert only on a changed immutable camera key, before changing any ready owner.
  let inverse_columns=snapshot.inverse().ok_or("Annotation camera inverse is singular")?;
  let inverse=std::array::from_fn(|i|inverse_columns[i/4][i%4]);
  let plan=Arc::new(if presentation{crate::annotation_plan::build_presentation_plan(app,rect,&matrix,&mut self.cache)?}else{build_plan(app,rect,&matrix,&mut self.cache,include_annotations)?});
  if plan.operations.is_empty(){self.style=raw.clone();self.vcb=app.vcb_text.clone();self.current=None;self.empty=Some(key);return Ok(None);}
  self.empty=None;
  let clip=[(rect.min.x*density).floor()as i32,(rect.min.y*density).floor()as i32,(rect.max.x*density).ceil()as i32,(rect.max.y*density).ceil()as i32];
  let mesh=self.cache.prepare_widget_clipped(plan.clone(),density,96.,clip)?;
  let extent=crate::viewport_extent::scene_extent([rect.width() as f64,rect.height() as f64],density as f64)?;
  let eye=snapshot.eye;let forward=snapshot.forward;
  let mut queries=Vec::new();queries.try_reserve_exact(plan.queries.len().max(1)).map_err(|_|"Annotation query CPU admission failed")?;
  for &point in &plan.queries{let mut query=prepare_query(&matrix,point,eye,forward,extent)?;if bypass_depth{query.active=0;}queries.push(query);}
  // Always-visible text with an unprojectable anchor has no source depth query.
  // One inactive output slot satisfies the GPU storage ABI without adding a gate.
  if queries.is_empty(){queries.push(Query{pixel:[0;2],normalized:[0.;2],threshold:0.,active:0,_padding:[0;2]});}
  // Changed plans receive a private immutable handle. Updating the accepted
  // handle here would mutate the previous callback before GPU/World admission.
  let handle=factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose{scale,translation:[0.;2]},mesh.gates.clone())?;
  self.style=raw.clone();self.vcb=app.vcb_text.clone();
  self.current=Some((key,Prepared{handle,queries:queries.into(),inverse,eye,forward}));
  Ok(self.current.as_ref().map(|(_,ready)|ready))
 }
}

/// Engineering-only layout inspection; no GPU visibility or image acceptance.
pub fn plan_for_probe(app:&crate::IngeTrazoApp,rect:egui::Rect)->Result<annotations::Plan,String>{
 let snapshot=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Invalid diagnostic camera")?;
 let matrix=std::array::from_fn(|i|snapshot.matrix[i/4][i%4]);
 let mut owner=ViewportAnnotations::new(false)?;
 build_plan(app,rect,&matrix,&mut owner.cache,true)
}

#[cfg(test)]mod label_tests{
 use super::*;
 #[test]fn acquired_pushpull_label_uses_source_layers_and_cancel_clears(){
  use glam::DVec3;
  use ingetrazo_tools::base::{create_tool,ToolContext,ToolType};
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  let ids=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].into_iter().map(|p|app.scene.mesh.add_vertex(p)).collect();
  app.scene.mesh.add_face(ids,vec![]);app.active_tool=create_tool(ToolType::PushPull);app.active_tool_type=ToolType::PushPull;
  app.apply_model_units(serde_json::json!({"length":"mm","precision":2}));
  let ctx=ToolContext::new(DVec3::new(0.5,0.5,0.),ingetrazo_core::math::Ray::new(DVec3::new(0.5,0.5,5.),-DVec3::Z));
  app.dispatch_tool_click(&ctx);assert!(app.active_tool.distance_label().is_some());
  let rect=egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.));
  let plan=plan_for_probe(&app,rect).unwrap();assert_eq!(plan.operations.len(),2);assert!(plan.queries.is_empty());
  let mut positions=Vec::new();
  for (operation,expected)in plan.operations.iter().zip([[255,255,255,220],[15,20,27,255]]){
   assert_eq!(operation.gate,annotations::Gate::Always);
   let annotations::Primitive::Text{position,text,size_pt,color}=&operation.primitive else{panic!("source label text")};
   assert_eq!(text.as_ref(),"0.00 mm");assert_eq!(*size_pt,10);assert_eq!(*color,expected);positions.push(*position);
  }
  assert_eq!([positions[0][0]-positions[1][0],positions[0][1]-positions[1][1]],[1.,1.]);
  app.active_tool.on_cancel(&mut app.scene);assert!(plan_for_probe(&app,rect).unwrap().operations.is_empty());
 }
 #[test]fn source_units_and_typed_label_marks(){
  assert_eq!(typed_distance_text("500","mm"),"500 mm");
  assert_eq!(typed_distance_text(" -2.5; +3 ","in-frac"),"-2.5; +3\"");
  assert_eq!(typed_distance_text("275cm","m"),"275cm");
  assert_eq!(typed_distance_text("2x","m"),"2x");
  assert_eq!(typed_distance_text("","m"),"");
  let mut scene=ingetrazo_core::scene::Scene::new();
  scene.native_document["units"]=serde_json::json!({"length":"ft-in-frac","precision":4});
  assert_eq!(label_units(&scene).unwrap(),("ft-in-frac",4));
 }
}
