//! Persistent application annotation producer. Depth sample policy remains explicit
//! until the paired source MSAA edge policy is independently admitted.
use std::sync::Arc;
use super::{annotation_visibility::{prepare_query,Query},cad2d::{Factory,Handle}};
use ingetrazo_render::annotations::{self,Builder,DimensionInput,LeaderInput};
use crate::annotation_overlay_geometry::{Cache,Limits};
#[derive(PartialEq)]struct Key{revision:u64,scene_version:u64,factory:usize,view:u64,matrix:[f32;16],rect:[f32;4],density:f32,selected:Option<usize>,bypass_depth:bool,distance:Option<(f64,[f64;3])>,units:(&'static str,i32)}
pub(crate) struct Prepared{pub handle:Handle,pub queries:Arc<[Query]>,pub inverse:[f32;16],pub eye:[f32;3],pub forward:[f32;3]}
pub(crate) struct ViewportAnnotations{cache:Cache,current:Option<(Key,Prepared)>,empty:Option<Key>,style:serde_json::Value,vcb:String}
impl ViewportAnnotations{
 pub fn new(stencil:bool)->Result<Self,String>{let mut result=Self{cache:Cache::new(ingetrazo_formats::annotation_font::Limits{faces:32,retained_font_bytes:32*1024*1024},Limits{cells:2_000_000,spans:200_000,geometry_bytes:32*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024})?,current:None,empty:None,style:serde_json::Value::Null,vcb:String::new()};if stencil{result.cache.enable_source_stencil();}Ok(result)}
 pub fn prepare<'a>(&'a mut self,app:&crate::IngeTrazoApp,factory:&Factory,rect:egui::Rect,density:f32)->Result<Option<&'a Prepared>,String>{
  if !density.is_finite()||density<=0.||!rect.min.is_finite()||!rect.max.is_finite()||rect.width()<=0.||rect.height()<=0.{return Err("Annotation viewport/density outside admission".into());}
  let bypass_depth=matches!(app.style.visual_style,ingetrazo_render::style::VisualStyle::XRay|ingetrazo_render::style::VisualStyle::Wireframe);
  let snapshot=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Annotation camera arithmetic outside admission")?;
  let matrix=std::array::from_fn(|i|snapshot.matrix[i/4][i%4]);
  let units=label_units(&app.scene)?;
  let distance=app.active_tool.distance_label().map(|(value,point)|(value,point.to_array()));
  if distance.is_some_and(|(value,point)|!value.is_finite()||point.iter().any(|p|!p.is_finite())){return Err("Tool label outside finite admission".into());}
  let key=Key{revision:app.native_geometry_revision,scene_version:app.scene.version,factory:factory.cache_identity(),view:app.scene.view_version,matrix,rect:[rect.min.x,rect.min.y,rect.max.x,rect.max.y],density,selected:app.selected_dimension,bypass_depth,distance,units};
  let raw=&app.scene.native_document["dimension_style"];
  let style_unchanged=self.style==*raw&&self.vcb==app.vcb_text;
  if style_unchanged&&self.empty.as_ref()==Some(&key){return Ok(None);}
  if style_unchanged&&self.current.as_ref().is_some_and(|(old,_)|*old==key){return Ok(self.current.as_ref().map(|(_,ready)|ready));}
  // Invert only on a changed immutable camera key, before changing any ready owner.
  let inverse_columns=snapshot.inverse().ok_or("Annotation camera inverse is singular")?;
  let inverse=std::array::from_fn(|i|inverse_columns[i/4][i%4]);
  let plan=Arc::new(build_plan(app,rect,&matrix,&mut self.cache)?);
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
  let handle=if let Some((_old_key,old))=self.current.as_ref().filter(|(old_key,_)|old_key.factory==key.factory){
   old.handle.update_annotation(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?;old.handle.clone()
  }else{factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?};
  self.style=raw.clone();self.vcb=app.vcb_text.clone();
  self.current=Some((key,Prepared{handle,queries:queries.into(),inverse,eye,forward}));
  Ok(self.current.as_ref().map(|(_,ready)|ready))
 }
}

// One planner shared by production and bounded CPU-only engineering inspection.
fn build_plan(app:&crate::IngeTrazoApp,rect:egui::Rect,matrix:&[f32;16],cache:&mut Cache)->Result<annotations::Plan,String>{
 let raw=&app.scene.native_document["dimension_style"];
  let project=|point:glam::Vec3|{
   let q=super::annotation_visibility::map(&matrix,[point.x,point.y,point.z,1.]);
   if !q.iter().all(|v|v.is_finite())||q[3]<=0.{return None;}
   Some([rect.min.x as f64+(q[0] as f64/q[3] as f64+1.)*0.5*rect.width() as f64,
    rect.min.y as f64+(1.-q[1] as f64/q[3] as f64)*0.5*rect.height() as f64])
  };
  let mut builder=Builder::new(project,annotations::Limits{queries:200_000,operations:200_000,text_bytes:16*1024*1024});
  let mut style=annotations::Style::default();
  if let Some(v)=raw["color"].as_array(){if v.len()>=3{for(i,c)in style.color[..3].iter_mut().enumerate(){*c=v[i].as_u64().and_then(|x|u8::try_from(x).ok()).ok_or("Invalid annotation style color")?;}}}
  if let Some(v)=raw["font_size"].as_i64(){style.font_size=i32::try_from(v).map_err(|_|"Annotation style size overflow")?;}
  if let Some(v)=raw["ends"].as_str(){style.ends=v.into();}
  if let Some(v)=raw["units"].as_str(){style.units=v.into();}
  if let Some(v)=raw["decimals"].as_i64(){style.decimals=i32::try_from(v).map_err(|_|"Annotation decimal overflow")?;}
  for(index,dim)in app.scene.dimensions.iter().enumerate(){
   let frame=app.scene.dimension_frame;
   builder.dimension(DimensionInput{a:frame.transform_point3(dim.start).as_vec3(),b:frame.transform_point3(dim.end).as_vec3(),offset:frame.transform_vector3(dim.offset).as_vec3(),axis:dim.axis.as_deref(),text:dim.text_override.as_deref(),highlighted:app.selected_dimension==Some(index),hidden:dim.native_document["hidden"].as_bool().unwrap_or(false)||!app.scene.is_layer_visible(dim.layer.as_deref())},&style)?;
  }
  let leader_style=style.clone();
  for label in &app.scene.text_labels{
   let(height,_)=cache.leader_metrics("",leader_style.font_size,96.)?;
   builder.leader(LeaderInput{anchor:label.anchor.as_vec3(),offset:label.offset.as_vec3(),text:&label.text,highlighted:false,hidden:!app.scene.is_layer_visible(label.layer.as_deref())},&leader_style,height,|text|cache.leader_metrics(text,leader_style.font_size,96.).map(|(_,advance)|advance))?;
  }
  let mut plan=builder.finish()?;
  if let Some((distance,point))=app.active_tool.distance_label(){
   if !distance.is_finite()||!point.is_finite(){return Err("Tool label outside finite admission".into());}
   if let Some(pixel)=project(point.as_vec3()){
    let(unit,precision)=label_units(&app.scene)?;
    let text:Arc<str>=if app.vcb_text.is_empty(){annotations::format_length(distance,unit,precision)?}else{typed_distance_text(&app.vcb_text,unit)}.into();
    let shadow=if app.vcb_text.is_empty(){[255,255,255,220]}else{[255,220,130,235]};
    for(offset,color)in [([12.,-7.],shadow),([11.,-8.],[15,20,27,255])]{
     let position=[pixel[0]+offset[0],pixel[1]+offset[1]];
     if position.iter().any(|x|!x.is_finite()){return Err("Tool label projection overflow".into());}
     plan.operations.try_reserve(1).map_err(|_|"Tool label operation admission failed")?;
     plan.operations.push(annotations::Operation{primitive:annotations::Primitive::Text{position,text:text.clone(),size_pt:10,color},gate:annotations::Gate::Always});
    }
   }
  }
  Ok(plan)

}
/// Engineering-only layout inspection; no GPU visibility or image acceptance.
pub fn plan_for_probe(app:&crate::IngeTrazoApp,rect:egui::Rect)->Result<annotations::Plan,String>{
 let snapshot=crate::navigation_ray::camera_snapshot(&app.camera).ok_or("Invalid diagnostic camera")?;
 let matrix=std::array::from_fn(|i|snapshot.matrix[i/4][i%4]);
 let mut owner=ViewportAnnotations::new(false)?;
 build_plan(app,rect,&matrix,&mut owner.cache)
}

fn label_units(scene:&ingetrazo_core::scene::Scene)->Result<(&'static str,i32),String>{
 use ingetrazo_core::units::LengthUnit;
 let fallback=match scene.unit{LengthUnit::Meters=>"m",LengthUnit::Centimeters=>"cm",LengthUnit::Millimeters=>"mm",LengthUnit::Inches=>"in",LengthUnit::Feet=>"ft"};
 let unit=match scene.native_document["units"]["length"].as_str().unwrap_or(fallback){
  "m"=>"m","cm"=>"cm","mm"=>"mm","in"=>"in","ft"=>"ft","ft-in"=>"ft-in","in-frac"=>"in-frac","ft-in-frac"=>"ft-in-frac",_=>return Err("Unknown tool label unit".into())};
 let precision=scene.native_document["units"]["precision"].as_i64().unwrap_or(2);
 Ok((unit,i32::try_from(precision).map_err(|_|"Tool label precision overflow")?))
}

// Pinned typed_value_text's unit mark is added only to bare numeric/list input.
fn typed_distance_text(buffer:&str,unit:&str)->String{
 let text=buffer.trim();
 let bare=!text.is_empty()&&text.split([',',';']).all(|part|{
  let p=part.trim();let p=p.strip_prefix(['-','+']).unwrap_or(p);
  !p.is_empty()&&p.chars().all(|c|c.is_ascii_digit()||c=='.')
 });
 if !bare{return text.into();}
 let mark=match unit{"m"=>" m","cm"=>" cm","mm"=>" mm","ft"=>"'",_=>"\""};
 format!("{text}{mark}")
}
#[cfg(test)]mod label_tests{
 use super::*;
 #[test]fn acquired_pushpull_label_uses_source_layers_and_cancel_clears(){
  use glam::DVec3;
  use ingetrazo_tools::base::{create_tool,ToolContext,ToolType};
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  let ids=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].into_iter().map(|p|app.scene.mesh.add_vertex(p)).collect();
  app.scene.mesh.add_face(ids,vec![]);app.active_tool=create_tool(ToolType::PushPull);app.active_tool_type=ToolType::PushPull;
  app.scene.native_document["units"]=serde_json::json!({"length":"mm","precision":2});
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
