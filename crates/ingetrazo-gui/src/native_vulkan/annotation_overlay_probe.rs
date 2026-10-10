//! Engineering fixture: captured original operations through the production
//! geometry producer, coordinated query/mesh transaction and actual UI pass.
//! Controlled operation inputs do not claim real viewport acquisition/depth parity.
use super::*;
use std::sync::Arc;
use ingetrazo_render::annotations::{Plan,Primitive,Operation,Gate};
struct Probe{plan:Arc<Plan>,cache:crate::annotation_overlay_geometry::Cache,handle:Option<cad2d::Handle>,failure:Option<String>,range_control:bool}
fn pair(value:&serde_json::Value)->Result<[f64;2],String>{let v=value.as_array().ok_or("Annotation point absent")?;if v.len()!=2{return Err("Annotation point inventory".into());}Ok([v[0].as_f64().ok_or("Annotation point x")?,v[1].as_f64().ok_or("Annotation point y")?])}
fn color(value:&serde_json::Value)->Result<[u8;4],String>{let v=value.as_array().ok_or("Annotation color absent")?;if v.len()!=4{return Err("Annotation color inventory".into());}let mut result=[0;4];for(i,item)in v.iter().enumerate(){result[i]=u8::try_from(item.as_u64().ok_or("Annotation color byte")?).map_err(|_|"Annotation color range")?;}Ok(result)}
fn source_plan(row:&serde_json::Value)->Result<Plan,String>{
 let mut operations=Vec::new();
 for op in row["operations"].as_array().ok_or("Annotation operations absent")?{
  let ink=color(&op["pen"])?;let width=op["width"].as_f64().ok_or("Annotation width absent")?;
  let primitive=match op["kind"].as_str().ok_or("Annotation operation kind")?{
   "line"=>Primitive::Line{a:pair(&op["a"])?,b:pair(&op["b"])?,width,color:ink},
   "polygon"=>{let points=op["points"].as_array().ok_or("Annotation polygon absent")?;if points.len()!=3{return Err("Annotation triangle inventory".into());}Primitive::Polygon{points:[pair(&points[0])?,pair(&points[1])?,pair(&points[2])?],color:color(&op["brush"])?}},
   "ellipse"=>Primitive::Ellipse{center:pair(&op["center"])?,radius:pair(&op["radius"])?,width,color:color(&op["brush"])?},
   "text"=>Primitive::Text{position:pair(&op["position"])?,text:Arc::from(op["text"].as_str().ok_or("Annotation text absent")?),size_pt:op["font"][1].as_f64().ok_or("Annotation font point size")? as i32,color:ink},
   _=>return Err("Unsupported annotation source operation".into()),
  };operations.push(Operation{primitive,gate:Gate::Always});
 }
 Ok(Plan{queries:Vec::new(),operations})
}
fn range_mesh(density:f32)->Result<Arc<crate::annotation_overlay_geometry::Mesh>,String>{
 use ingetrazo_render::cad2d::{Vertex,Batch,SourceSpan,EncodedRgba,PremultipliedRgba8,Geometry};
 let rows=[SourceSpan{x:20,y:20,len:10,coverage:128},SourceSpan{x:30,y:20,len:10,coverage:128},SourceSpan{x:25,y:20,len:20,coverage:128}];
 let mut vertices=Vec::new();let mut indices=Vec::new();
 for row in rows{let base=vertices.len()as u32;for p in [[row.x,row.y],[row.x+row.len,row.y],[row.x+row.len,row.y+1],[row.x,row.y+1]]{vertices.push(Vertex{position:[p[0]as f32/density,p[1]as f32/density],coverage:row.coverage as f32/255.});}indices.extend([base,base+1,base+2,base,base+2,base+3]);}
 let batches:Arc<[Batch]>=(0..3).map(|i|Batch{first:i*6,count:6,color:EncodedRgba::from_bytes([0,0,0,255],1.).unwrap()}).collect::<Vec<_>>().into();
 let colors=[[60,10,20,80],[10,50,30,90],[0,60,0,60]].map(PremultipliedRgba8::new).into_iter().collect::<Result<Vec<_>,_>>()?;
 let geometry=Geometry::new_with_premultiplied_source_spans(vertices,indices,vec![0..12,12..18],rows.to_vec(),1.,density,[0.;2],colors,&batches)?;
 Ok(Arc::new(crate::annotation_overlay_geometry::Mesh{geometry,batches,gates:Arc::from([Gate::Always;3])}))
}
impl App for Probe{
 fn title()->&'static str{"IngeTrazo native annotation transaction probe"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
  let result=(||{
   let path=std::env::var("INGETRAZO_ANNOTATION_ORACLE").map_err(|_|"Annotation oracle path absent")?;
   let file=std::fs::File::open(path).map_err(|e|e.to_string())?;let mut bytes=Vec::new();use std::io::Read;file.take(4*1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;if bytes.len()>4*1024*1024{return Err("Annotation oracle admission exceeded".into());}let oracle:serde_json::Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
   if oracle["source_pin"]!="6be29fe437a7cd992d4e079f71720821b25216b9"{return Err("Annotation original pin mismatch".into());}
   let name=std::env::var("INGETRAZO_ANNOTATION_CASE").map_err(|_|"Annotation case absent")?;
   let range_control=name=="palette-intersection-control";
   let row=oracle["cases"].as_array().ok_or("Annotation case inventory")?.iter().find(|r|r["name"]==if range_control{"arrow"}else{&name}).ok_or("Annotation required case missing")?;
   let plan=Arc::new(source_plan(row)?);
   let cache=crate::annotation_overlay_geometry::Cache::new(ingetrazo_formats::annotation_font::Limits{faces:32,retained_font_bytes:32*1024*1024},crate::annotation_overlay_geometry::Limits{cells:2_000_000,spans:200_000,geometry_bytes:32*1024*1024,glyphs:4096,glyph_bytes:16*1024*1024})?;
   system.world=Some(world::World::new(&system.vulkan_context,system.memory_budget.clone())?);
   system.world.as_mut().unwrap().resize_offscreen(&system.vulkan_context,vk::Extent2D{width:64,height:64})?;
   system.world.as_mut().unwrap().set_scene(&system.vulkan_context,&ingetrazo_core::scene::Scene::new(),&ingetrazo_core::camera::OrbitCamera::default(),&ingetrazo_render::style::StyleConfig::default(),system.fence,system.draw_serial,1)?;
   Ok::<_,String>((plan,cache,range_control))
  })();
  match result{Ok((plan,cache,range_control))=>Self{plan,cache,handle:None,failure:None,range_control},Err(error)=>panic!("Annotation fixture admission: {error}")}
 }
 fn prepare_gpu(&mut self,system:&mut System,_:bool){
  let result=(||{
   let factory=system.annotation_factory()?;
   if self.plan.operations.is_empty(){system.clear_annotation_overlay()?;return Ok(());}
   if self.handle.is_none(){let mesh=if self.range_control{range_mesh(system.egui_ctx.pixels_per_point())?}else{self.cache.prepare(self.plan.clone(),system.egui_ctx.pixels_per_point(),96.)?};self.handle=Some(factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?);}
   let query=annotation_visibility::Query{pixel:[0;2],normalized:[0.;2],threshold:0.,active:0,_padding:[0;2]};
   // Cache this immutable query owner as well as the mesh, rather than allocate
   // or serialize anything on unchanged presentation frames.
   static QUERIES:std::sync::OnceLock<Arc<[annotation_visibility::Query]>>=std::sync::OnceLock::new();
   let queries=QUERIES.get_or_init(||Arc::from([query])).clone();
   system.prepare_annotation_overlay(self.handle.as_ref().unwrap(),queries,glam::Mat4::IDENTITY.to_cols_array(),[0.;3],[0.,0.,-1.],0)?;
   Ok::<(),String>(())
  })();self.failure=result.as_ref().err().cloned();system.annotation_prepare_error=self.failure.clone();
  if let Err(error)=result{self.gpu_error(&error);}
 }
 fn build_ui(&mut self,ctx:&Context){
  let rect=ctx.screen_rect();let painter=ctx.layer_painter(egui::LayerId::background());painter.rect_filled(rect,0.,egui::Color32::WHITE);
  if let Some(handle)=&self.handle{painter.add(handle.shape(rect));}
 }
 fn gpu_error(&mut self,error:&str){eprintln!("Annotation fixture GPU refusal: {error}");}
}
pub fn run()->Result<(),Box<dyn Error>>{run_app::<Probe>(None)}
