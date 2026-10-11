//! Engineering-only actual App/Tool dispatcher and native Vulkan submission.
use super::{App,System,run_app};
use std::{path::PathBuf,error::Error,io::Read};
use glam::DVec3;
use ingetrazo_tools::base::{ToolContext,ToolType};
struct Probe{app:crate::IngeTrazoApp,nonce:u64,refuse:bool,font_refuse:bool,cad_refuse:bool,distance:Option<f64>,dir:PathBuf,anchor:DVec3,normal:DVec3,face:usize,owner_nonce:u64,snapshot:bool,snapshot_nonce:u64,old_owner:Option<std::sync::Weak<dyn std::any::Any+Send+Sync>>}
fn vec(value:&serde_json::Value)->DVec3{DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap())}
impl App for Probe{
 fn title()->&'static str{"IngeTrazo actual FaceSweep workflow"}
 fn new(system:&mut System,_:Option<PathBuf>)->Self{
  let source=PathBuf::from(std::env::var_os("INGETRAZO_SWEEP_SOURCE").expect("Sweep source"));
  let rows:serde_json::Value=serde_json::from_slice(&std::fs::read(source.join("original.json")).unwrap()).unwrap();
  let name=std::env::var("INGETRAZO_SWEEP_CASE").unwrap_or("ordinary-prism".into());
  let row=rows["cases"].as_array().unwrap().iter().find(|row|row["name"]==name).unwrap();
  let mut app=<crate::IngeTrazoApp as App>::new(system,Some(source.join(row["igz"].as_str().unwrap())));
  app.native_gpu_viewport_enabled=true;app.show_tray=false;
  app.camera.target=vec(&row["camera"]["target"]);app.camera.distance=row["camera"]["distance"].as_f64().unwrap();app.camera.yaw=row["camera"]["yaw"].as_f64().unwrap();app.camera.pitch=row["camera"]["pitch"].as_f64().unwrap();app.camera.perspective=false;
  app.style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;app.style.show_textures=true;app.style.background_color=[1.;4];app.style.show_sky=false;app.style.show_edges=true;app.style.show_profiles=false;
  app.mark_document_clean();
  Self{app,nonce:0,refuse:false,font_refuse:false,cad_refuse:false,distance:None,dir:PathBuf::from(std::env::var_os("INGETRAZO_SWEEP_CONTROL").expect("Sweep control")),anchor:vec(&row["anchor"]),normal:vec(&row["normal"]),face:row["face"].as_u64().unwrap()as usize,owner_nonce:0,snapshot:false,snapshot_nonce:0,old_owner:None}
 }
 fn build_ui(&mut self,ctx:&egui::Context){
  if std::env::var_os("INGETRAZO_SWEEP_SOURCE_FRAME").is_some(){
   // Declared 800x600 viewport-only engineering comparison. The production
   // App::prepare_gpu/World/label path is unchanged; document chrome is omitted.
   egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{
    let rect=ui.max_rect();self.app.camera.aspect=f64::from(rect.width())/f64::from(rect.height());
    self.app.native_gpu_viewport_rect=Some(rect);
    if let Some(texture)=self.app.native_gpu_viewport_texture{ui.painter().image(texture,rect,egui::Rect::from_min_max(egui::pos2(0.,0.),egui::pos2(1.,1.)),egui::Color32::WHITE);}
    if let Some(egui::Shape::Callback(callback))=&self.app.native_gpu_annotation_shape{let mut callback=callback.clone();callback.rect=rect;ui.painter().add(egui::Shape::Callback(callback));}
   });
  }else{self.app.update_ui(ctx);}
  ctx.request_repaint_after(std::time::Duration::from_millis(16));
  if let Ok(file)=std::fs::File::open(self.dir.join("request.json")){
   let mut bytes=Vec::new();file.take(1025).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=1024);
   let row:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let nonce=row["nonce"].as_u64().unwrap();
   if nonce>self.nonce{
    match row["action"].as_str().unwrap(){
     "acquire"=>{self.app.set_tool(ToolType::PushPull);let eye=self.app.camera.eye();let context=ToolContext::new(self.anchor,ingetrazo_core::math::Ray::new(eye,(self.anchor-eye).normalize()));self.app.dispatch_tool_click(&context);let preview=self.app.active_tool.face_sweep_preview().expect("Actual Tool acquisition");assert_eq!(preview.face,self.face);assert_eq!(preview.normal,self.normal);self.distance=Some(0.);},
     "drag"|"refuse"=>{let distance=row["distance"].as_f64().unwrap();assert!(distance.is_finite());let point=self.anchor+self.normal*distance;let eye=self.app.camera.eye();let context=ToolContext::new(point,ingetrazo_core::math::Ray::new(eye,(point-eye).normalize()));self.app.active_tool.on_move(&mut self.app.scene,&context);self.refuse=row["action"]=="refuse";self.distance=Some(distance);},
     "refuse-font"|"refuse-cad"=>{self.app.vcb_text=row["text"].as_str().unwrap().into();self.font_refuse=row["action"]=="refuse-font";self.cad_refuse=row["action"]=="refuse-cad";},
     "retry"=>{self.refuse=false;self.font_refuse=false;self.cad_refuse=false;},
     "cancel"=>{self.distance=None;self.refuse=false;self.app.active_tool.on_cancel(&mut self.app.scene);self.app.vcb_text.clear();},
     "commit"=>{self.distance=None;self.app.dispatch_tool_vcb(row["text"].as_str().unwrap()).unwrap();self.app.vcb_text.clear();},
     "snapshot"=>{self.snapshot=true;},
     "undo"=>self.app.undo(),
     "typed"=>{self.app.vcb_text=row["text"].as_str().unwrap().into();},
     "save"=>self.app.save_to_path(&self.dir.join("saved.igz")),
     _=>panic!("Unknown sweep operation"),
    }self.nonce=nonce;
   }
  }
  // This engineering protocol supplies an explicit persistent motion sample
  // through the production Tool handler; it does not claim hardware input.
  // Otherwise the owned X server's unrelated stationary cursor changes the
  // selected sample during each UI pass before presentation acknowledgment.
  if let Some(distance)=self.distance{
   let point=self.anchor+self.normal*distance;let eye=self.app.camera.eye();
   self.app.active_tool.on_move(&mut self.app.scene,&ToolContext::new(point,ingetrazo_core::math::Ray::new(eye,(point-eye).normalize())));
  }
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){
  let prior={let mut ledger=system.memory_budget.ledger.lock().unwrap();let prior=ledger.total_limit;if self.refuse{ledger.total_limit=Some(ledger.total_committed());}prior};
  if (self.font_refuse||self.cad_refuse)&&self.owner_nonce!=self.nonce{
   self.old_owner=system.annotation_ready_shape.as_ref().and_then(|shape|if let egui::Shape::Callback(callback)=shape{Some(std::sync::Arc::downgrade(&callback.callback))}else{None});self.owner_nonce=self.nonce;
  }
  system.annotation_font_refusal_for_probe=self.font_refuse;
  if self.cad_refuse&&!resize{if let Ok(_)=system.annotation_factory(){system.annotation_cad.as_mut().unwrap().force_annotation_pressure_for_probe();}}
  App::prepare_gpu(&mut self.app,system,resize);
  system.annotation_font_refusal_for_probe=false;
  system.memory_budget.ledger.lock().unwrap().total_limit=prior;
  if !resize&&self.snapshot&&self.snapshot_nonce!=self.nonce{
   let bytes=system.world.as_mut().unwrap().readback(&system.vulkan_context).expect("Retained target readback");
   std::fs::write(self.dir.join(format!("target-{}.rgba",self.nonce)),bytes).unwrap();self.snapshot_nonce=self.nonce;self.snapshot=false;
  }
  let label=self.app.active_tool.distance_label().map(|(distance,point)|serde_json::json!({"distance":distance,"point":point.to_array()}));
  let row=serde_json::json!({"nonce":self.nonce,"old_label_owner_alive":self.old_owner.as_ref().map(|owner|owner.strong_count()>0),"label":label,"dirty":self.app.document_dirty(),"screen_size":[system.egui_ctx.screen_rect().width(),system.egui_ctx.screen_rect().height()],"viewport":self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"gpu":system.face_sweep_observation_for_probe()});
  std::fs::write(self.dir.join("state.tmp"),serde_json::to_vec(&row).unwrap()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
 }
 fn gpu_error(&mut self,error:&str){self.app.gpu_error(error);}
}
pub fn run()->Result<(),Box<dyn Error>>{run_app::<Probe>(None)}
