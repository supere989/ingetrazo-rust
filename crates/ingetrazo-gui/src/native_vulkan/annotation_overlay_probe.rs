//! Engineering fixture: captured original operations through the production
//! geometry producer, coordinated query/mesh transaction and actual UI pass.
//! Controlled operation inputs do not claim real viewport acquisition/depth parity.
use super::*;
use std::sync::Arc;
use ingetrazo_render::annotations::{Plan,Primitive,Operation,Gate};
struct Probe{plan:Arc<Plan>,cache:crate::annotation_overlay_geometry::Cache,handle:Option<cad2d::Handle>,failure:Option<String>,range_control:bool,byte_glyph_control:bool,queries:Arc<[annotation_visibility::Query]>,pending:Option<(cad2d::Handle,Arc<[annotation_visibility::Query]>)>,control_nonce:u64,cleared:bool,hold:bool}
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
fn range_mesh(density:f32,alternate:bool)->Result<Arc<crate::annotation_overlay_geometry::Mesh>,String>{
 use ingetrazo_render::cad2d::{Vertex,Batch,SourceSpan,EncodedRgba,PremultipliedRgba8,Geometry};
 let rows=[SourceSpan{x:20,y:20,len:10,coverage:128},SourceSpan{x:30,y:20,len:10,coverage:128},SourceSpan{x:25,y:20,len:20,coverage:128}];
 let mut vertices=Vec::new();let mut indices=Vec::new();
 for row in rows{let base=vertices.len()as u32;for p in [[row.x,row.y],[row.x+row.len,row.y],[row.x+row.len,row.y+1],[row.x,row.y+1]]{vertices.push(Vertex{position:[p[0]as f32/density,p[1]as f32/density],coverage:row.coverage as f32/255.});}indices.extend([base,base+1,base+2,base,base+2,base+3]);}
 let batches:Arc<[Batch]>=(0..3).map(|i|Batch{first:i*6,count:6,color:EncodedRgba::from_bytes([0,0,0,255],1.).unwrap()}).collect::<Vec<_>>().into();
 let colors=[[60,10,20,80],[10,50,30,90],if alternate{[0,0,60,60]}else{[0,60,0,60]}].map(PremultipliedRgba8::new).into_iter().collect::<Result<Vec<_>,_>>()?;
 let geometry=Geometry::new_with_premultiplied_source_spans(vertices,indices,vec![0..12,12..18],rows.to_vec(),1.,density,[0.;2],colors,&batches)?;
 Ok(Arc::new(crate::annotation_overlay_geometry::Mesh{geometry,batches,gates:Arc::from([crate::annotation_overlay_geometry::Draw::span(Gate::Always);3])}))
}
impl Probe{
 fn control(&mut self,system:&mut System,factory:&cad2d::Factory)->Result<(),String>{
  let Some(dir)=std::env::var_os("INGETRAZO_ANNOTATION_CONTROL_DIR")else{return Ok(());};let dir=std::path::PathBuf::from(dir);
  let path=dir.join("request.json");let mut request=None;
  if path.exists(){let bytes=std::fs::read(&path).map_err(|e|e.to_string())?;if bytes.len()>256{return Err("Annotation control request admission".into());}request=Some(serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e|e.to_string())?);}
  if let Some(request)=request{let nonce=request["nonce"].as_u64().ok_or("Annotation control nonce")?;if nonce>self.control_nonce{
   match request["action"].as_str().ok_or("Annotation control action")?{
    "refuse"=>{
     let before=system.memory_budget.ledger.lock().map_err(|_|"GPU ledger poisoned")?.total_committed();
     let old=system.annotation_cad.as_ref().ok_or("Annotation renderer absent")?.stats();
     let mesh=range_mesh(system.egui_ctx.pixels_per_point(),true)?;
     let handle=factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),mesh.gates.clone())?;
     let queries:Arc<[annotation_visibility::Query]>=Arc::from([annotation_visibility::Query{pixel:[1,1],normalized:[0.;2],threshold:0.,active:0,_padding:[0;2]}]);
     system.annotation_cad.as_mut().unwrap().force_annotation_pressure_for_probe();
     let result=system.prepare_annotation_overlay(&handle,queries.clone(),glam::Mat4::IDENTITY.to_cols_array(),[0.;3],[0.,0.,-1.],0);
     if result.is_ok(){return Err("Actual annotation pressure unexpectedly admitted index buffer".into());}
     let now=system.annotation_cad.as_ref().unwrap().stats();
     if old.uploads!=now.uploads||old.cached_bytes!=now.cached_bytes||system.memory_budget.ledger.lock().unwrap().total_committed()!=before{return Err("Annotation refusal changed old ready owner or leaked ledger".into());}
     eprintln!("Annotation actual index-admission refusal retained: {}",result.unwrap_err());
     self.pending=Some((handle,queries));self.hold=true;
    },
    "invalid-gate"=>{
     let before=system.memory_budget.ledger.lock().unwrap().total_committed();let old=system.annotation_cad.as_ref().unwrap().stats();
     let mesh=range_mesh(system.egui_ctx.pixels_per_point(),true)?;
     let handle=factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),Arc::from([Gate::Always,Gate::Point(1),Gate::Always].map(crate::annotation_overlay_geometry::Draw::span)))?;
     if system.prepare_annotation_overlay(&handle,self.queries.clone(),glam::Mat4::IDENTITY.to_cols_array(),[0.;3],[0.,0.,-1.],0).is_ok(){return Err("Malformed annotation gate unexpectedly published".into());}
     let now=system.annotation_cad.as_ref().unwrap().stats();if old.uploads!=now.uploads||old.cached_bytes!=now.cached_bytes||system.memory_budget.ledger.lock().unwrap().total_committed()!=before{return Err("Malformed gate changed old ready publication".into());}
     self.hold=true;
    },
    "retry"=>{let(handle,queries)=self.pending.as_ref().ok_or("Annotation retry candidate absent")?;
     system.prepare_annotation_overlay(handle,queries.clone(),glam::Mat4::IDENTITY.to_cols_array(),[0.;3],[0.,0.,-1.],0)?;
     self.handle=Some(handle.clone());self.queries=queries.clone();self.pending=None;self.hold=false;self.cleared=false;},
    "clear"=>{system.clear_annotation_overlay()?;self.handle=None;self.pending=None;self.cleared=true;self.hold=false;},
    "restore"=>{self.cleared=false;self.handle=None;self.hold=false;},
    _=>return Err("Unknown annotation control action".into()),
   };self.control_nonce=nonce;
  }}
  let stats=system.annotation_cad.as_ref().ok_or("Annotation renderer absent")?.stats();
  let row=serde_json::json!({"nonce":self.control_nonce,"submitted_serial":system.draw_serial,"cleared":self.cleared,"held_refusal":self.hold,"geometry_uploads":stats.uploads,"cached_bytes":stats.cached_bytes,"rollback_buffers":stats.rolled_back_buffers,"query_uploads":system.world.as_ref().map_or(0,|world|world.annotation_query_uploads_for_probe()),"committed_bytes":system.memory_budget.ledger.lock().unwrap().total_committed()});
  let temporary=dir.join("state.tmp");std::fs::write(&temporary,serde_json::to_vec(&row).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;std::fs::rename(temporary,dir.join("state.json")).map_err(|e|e.to_string())?;Ok(())
 }
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
  match result{Ok((plan,cache,range_control))=>Self{plan,cache,handle:None,failure:None,range_control,byte_glyph_control:std::env::var_os("INGETRAZO_ANNOTATION_GLYPH_BYTE_CONTROL").is_some(),queries:Arc::from([annotation_visibility::Query{pixel:[0;2],normalized:[0.;2],threshold:0.,active:0,_padding:[0;2]}]),pending:None,control_nonce:0,cleared:false,hold:false},Err(error)=>panic!("Annotation fixture admission: {error}")}
 }
 fn prepare_gpu(&mut self,system:&mut System,_:bool){
  let result=(||{
   let factory=system.annotation_factory()?;
   if self.range_control{self.control(system,&factory)?;}
   if self.hold{return Err("Annotation controlled replacement held after actual admission refusal".into());}
   if self.cleared{system.clear_annotation_overlay()?;return Ok(());}
   if self.plan.operations.is_empty(){system.clear_annotation_overlay()?;return Ok(());}
   if self.handle.is_none(){let mesh=if self.range_control{range_mesh(system.egui_ctx.pixels_per_point(),false)?}else{self.cache.prepare_clipped(self.plan.clone(),system.egui_ctx.pixels_per_point(),96.,[0,0,640,400])?};let draws=if self.byte_glyph_control{mesh.gates.iter().copied().map(|mut draw|{draw.composition=crate::annotation_overlay_geometry::Composition::SpanByte;draw}).collect::<Vec<_>>().into()}else{mesh.gates.clone()};self.handle=Some(factory.annotation_handle(mesh.geometry.clone(),mesh.batches.clone(),ingetrazo_render::cad2d::Pose::default(),draws)?);}
   system.prepare_annotation_overlay(self.handle.as_ref().unwrap(),self.queries.clone(),glam::Mat4::IDENTITY.to_cols_array(),[0.;3],[0.,0.,-1.],0)?;
   Ok::<(),String>(())
  })();self.failure=result.as_ref().err().cloned();system.annotation_prepare_error=self.failure.clone();
  if let Err(error)=result{self.gpu_error(&error);}
 }
 fn build_ui(&mut self,ctx:&Context){
  if std::env::var_os("INGETRAZO_ANNOTATION_CONTROL_DIR").is_some(){ctx.request_repaint_after(std::time::Duration::from_millis(16));}
  let rect=ctx.screen_rect();let painter=ctx.layer_painter(egui::LayerId::background());painter.rect_filled(rect,0.,egui::Color32::WHITE);
  if let Some(handle)=&self.handle{let canvas=egui::Rect::from_min_size(rect.min,egui::vec2(640.,400.));painter.with_clip_rect(canvas).add(handle.shape(canvas));}
 }
 fn gpu_error(&mut self,error:&str){eprintln!("Annotation fixture GPU refusal: {error}");}
}
pub fn run()->Result<(),Box<dyn Error>>{if std::env::var_os("INGETRAZO_ANNOTATION_APP_CONTROL").is_some(){run_app::<ApplicationProbe>(None)}else{run_app::<Probe>(None)}}

struct ApplicationProbe{app:crate::IngeTrazoApp,nonce:u64,held:bool,pressure:bool,texture:Option<egui::TextureHandle>}
impl App for ApplicationProbe{
 fn title()->&'static str{"IngeTrazo actual application annotation probe"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self{
  let mut app=crate::IngeTrazoApp::new();app.native_gpu_viewport_enabled=true;app.show_tray=false;
  app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);app.camera.target=glam::DVec3::ZERO;app.camera.distance=5.;app.camera.perspective=false;app.mark_document_clean();
  if let Some(path)=std::env::var_os("INGETRAZO_ANNOTATION_APP_SCENE"){
   app.scene=ingetrazo_formats::igz::load_igz(&std::path::PathBuf::from(path)).expect("Actual annotation app scene admission");
   let camera_path=std::env::var_os("INGETRAZO_ANNOTATION_APP_CAMERA").expect("Actual scene requires explicit camera");
   use std::io::Read;let mut bytes=Vec::new();std::fs::File::open(camera_path).unwrap().take(16*1024+1).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=16*1024,"Camera input exceeds admission");
   app.camera=serde_json::from_slice(&bytes).expect("Actual annotation app camera schema");
   app.style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;
   app.style.show_textures=false;app.style.background_color=[1.;4];app.style.show_sky=false;app.style.show_profiles=false;
   app.mark_document_clean();
  }
  Self{app,nonce:0,held:false,pressure:false,texture:None}
 }
 fn build_ui(&mut self,ctx:&Context){
  self.app.update_ui(ctx);ctx.request_repaint_after(std::time::Duration::from_millis(16));
  // Mutate AFTER production render_viewport has collected the old membership.
  let dir=std::path::PathBuf::from(std::env::var_os("INGETRAZO_ANNOTATION_CONTROL_DIR").unwrap());
  if let Ok(file)=std::fs::File::open(dir.join("request.json")){use std::io::Read;let mut bytes=Vec::new();file.take(257).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=256);let row:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let nonce=row["nonce"].as_u64().unwrap();if nonce>self.nonce{
   match row["action"].as_str().unwrap(){
    "add"=>{self.app.save_undo_state();self.app.scene.dimensions.push(ingetrazo_core::dimensions::Dimension::new(glam::DVec3::new(-1.,0.,0.),glam::DVec3::new(1.,0.,0.),glam::DVec3::Y));self.app.scene.bump_version();},
    "delete"=>{assert!(self.app.remove_dimension(0));},
    "refuse"=>{assert!(self.app.edit_dimension(0,Some("replacement <>".into()),None,None));self.pressure=true;self.texture=Some(ctx.load_texture("refused-new-managed-image",egui::ColorImage::new([2,2],egui::Color32::RED),egui::TextureOptions::NEAREST));},
    "retry"=>self.held=false,
    _=>panic!("Unknown actual app control"),
   };self.app.mark_document_clean();self.nonce=nonce;
  }}
  if let(Some(texture),Some(rect))=(&self.texture,self.app.native_gpu_viewport_rect){let target=egui::Rect::from_min_size(rect.min+egui::vec2(20.,20.),egui::vec2(20.,20.));ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,egui::Id::new("actual-app-managed-control"))).image(texture.id(),target,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::WHITE);}
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){
  if self.held{system.annotation_prepare_error=Some("Actual app controlled refusal retained".into());}
  else{
   if self.pressure{system.annotation_cad.as_mut().expect("renderer created before controlled replacement").force_annotation_pressure_for_probe();}
   App::prepare_gpu(&mut self.app,system,resize);
   if self.pressure{assert!(system.annotation_prepare_error.is_some(),"Actual app pressure unexpectedly published");self.pressure=false;self.held=true;}
  }
  let row=serde_json::json!({"nonce":self.nonce,"submitted_serial":system.draw_serial,"held_refusal":self.held,"dimensions":self.app.scene.dimensions.len(),"leaders":self.app.scene.text_labels.len(),"camera":self.app.camera,"camera_snapshot":crate::navigation_ray::camera_snapshot(&self.app.camera).map(|s|serde_json::json!({"matrix":s.matrix.concat(),"inverse":s.inverse().map(|m|m.concat()),"eye":s.eye,"forward":s.forward})),"screen_size":[system.egui_ctx.screen_rect().width(),system.egui_ctx.screen_rect().height()],"viewport":self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"cached_bytes":system.annotation_cad.as_ref().map_or(0,|r|r.stats().cached_bytes),"pending_textures":system.pending_texture_updates.len()});
  let dir=std::path::PathBuf::from(std::env::var_os("INGETRAZO_ANNOTATION_CONTROL_DIR").unwrap());let temp=dir.join("state.tmp");std::fs::write(&temp,serde_json::to_vec(&row).unwrap()).unwrap();std::fs::rename(temp,dir.join("state.json")).unwrap();
 }
 fn gpu_error(&mut self,error:&str){eprintln!("Actual annotation app refusal: {error}");}
}
