//! Persistent application annotation producer. Depth sample policy remains explicit
//! until the paired source MSAA edge policy is independently admitted.
use std::sync::Arc;
use super::{annotation_visibility::{prepare_query,Query},cad2d::{Factory,Handle}};
use ingetrazo_render::annotations::{self,Builder,DimensionInput,LeaderInput};
use crate::annotation_overlay_geometry::{Cache,Limits};
#[derive(PartialEq)]struct Key{revision:u64,view:u64,matrix:[f32;16],rect:[f32;4],density:f32,selected:Option<usize>,bypass_depth:bool}
pub(crate) struct Prepared{pub handle:Handle,pub queries:Arc<[Query]>,pub inverse:[f32;16],pub eye:[f32;3],pub forward:[f32;3]}
pub(crate) struct ViewportAnnotations{cache:Cache,current:Option<(Key,Prepared)>,empty:Option<Key>}
impl ViewportAnnotations{
 pub fn new()->Result<Self,String>{Ok(Self{cache:Cache::new(ingetrazo_formats::annotation_font::Limits{faces:32,retained_font_bytes:32*1024*1024},Limits{cells:2_000_000,spans:200_000,geometry_bytes:32*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024})?,current:None,empty:None})}
 pub fn prepare<'a>(&'a mut self,app:&crate::IngeTrazoApp,factory:&Factory,rect:egui::Rect,density:f32)->Result<Option<&'a Prepared>,String>{
  if !density.is_finite()||density<=0.||!rect.min.is_finite()||!rect.max.is_finite()||rect.width()<=0.||rect.height()<=0.{return Err("Annotation viewport/density outside admission".into());}
  let bypass_depth=matches!(app.style.visual_style,ingetrazo_render::style::VisualStyle::XRay|ingetrazo_render::style::VisualStyle::Wireframe);
  let mut matrix=(app.camera.projection_matrix()*app.camera.view_matrix()).as_mat4().to_cols_array();
  for c in 0..4{matrix[c*4+2]=2.*matrix[c*4+2]-matrix[c*4+3];}
  let key=Key{revision:app.native_geometry_revision,view:app.scene.view_version,matrix,rect:[rect.min.x,rect.min.y,rect.max.x,rect.max.y],density,selected:app.selected_dimension,bypass_depth};
  if self.empty.as_ref()==Some(&key){return Ok(None);}
  if self.current.as_ref().is_some_and(|(old,_)|*old==key){return Ok(self.current.as_ref().map(|(_,ready)|ready));}
  let project=|point:glam::Vec3|{
   let q=super::annotation_visibility::map(&matrix,[point.x,point.y,point.z,1.]);
   if !q.iter().all(|v|v.is_finite())||q[3] as f64<=1e-9{return None;}
   Some([rect.min.x as f64+(q[0] as f64/q[3] as f64+1.)*0.5*rect.width() as f64,
    rect.min.y as f64+(1.-q[1] as f64/q[3] as f64)*0.5*rect.height() as f64])
  };
  let mut builder=Builder::new(project,annotations::Limits{queries:200_000,operations:200_000,text_bytes:16*1024*1024});
  let raw=&app.scene.native_document["dimension_style"];
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
   let(height,_)=self.cache.leader_metrics("",leader_style.font_size,96.)?;
   builder.leader(LeaderInput{anchor:label.anchor.as_vec3(),offset:label.offset.as_vec3(),text:&label.text,highlighted:false,hidden:!app.scene.is_layer_visible(label.layer.as_deref())},&leader_style,height,|text|self.cache.leader_metrics(text,leader_style.font_size,96.).map(|(_,advance)|advance))?;
  }
  let plan=Arc::new(builder.finish()?);
  if plan.operations.is_empty(){self.current=None;self.empty=Some(key);return Ok(None);}
  self.empty=None;
  let mesh=self.cache.prepare(plan.clone(),density,96.)?;
  let extent=[(rect.width()*density).ceil().max(1.)as u32,(rect.height()*density).ceil().max(1.)as u32];
  let eye=app.camera.eye().as_vec3().to_array();let forward=app.camera.forward().as_vec3().to_array();
  let mut queries=Vec::new();queries.try_reserve_exact(plan.queries.len().max(1)).map_err(|_|"Annotation query CPU admission failed")?;
  for &point in &plan.queries{let mut query=prepare_query(&matrix,point,eye,forward,extent)?;if bypass_depth{query.active=0;}queries.push(query);}
  // Always-visible text with an unprojectable anchor has no source depth query.
  // One inactive output slot satisfies the GPU storage ABI without adding a gate.
  if queries.is_empty(){queries.push(Query{pixel:[0;2],normalized:[0.;2],threshold:0.,active:0,_padding:[0;2]});}
  let handle=if let Some((_,old))=&self.current{
   old.handle.update_annotation(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?;old.handle.clone()
  }else{factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?};
  self.current=Some((key,Prepared{handle,queries:queries.into(),inverse:glam::Mat4::from_cols_array(&matrix).inverse().to_cols_array(),eye,forward}));
  Ok(self.current.as_ref().map(|(_,ready)|ready))
 }
}
