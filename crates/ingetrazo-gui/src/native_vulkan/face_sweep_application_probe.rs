//! Engineering-only actual App/Tool dispatcher and native Vulkan submission.
use super::{App,System,run_app};
use std::{path::PathBuf,error::Error,io::Read};
use glam::DVec3;
use ingetrazo_tools::base::{ToolContext,ToolType};
struct Probe{app:crate::IngeTrazoApp,nonce:u64,refuse:bool,dir:PathBuf,anchor:DVec3,normal:DVec3,face:usize}
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
  Self{app,nonce:0,refuse:false,dir:PathBuf::from(std::env::var_os("INGETRAZO_SWEEP_CONTROL").expect("Sweep control")),anchor:vec(&row["anchor"]),normal:vec(&row["normal"]),face:row["face"].as_u64().unwrap()as usize}
 }
 fn build_ui(&mut self,ctx:&egui::Context){
  self.app.update_ui(ctx);ctx.request_repaint_after(std::time::Duration::from_millis(16));
  if let Ok(file)=std::fs::File::open(self.dir.join("request.json")){
   let mut bytes=Vec::new();file.take(1025).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=1024);
   let row:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let nonce=row["nonce"].as_u64().unwrap();
   if nonce>self.nonce{
    match row["action"].as_str().unwrap(){
     "acquire"=>{self.app.set_tool(ToolType::PushPull);let eye=self.app.camera.eye();let context=ToolContext::new(self.anchor,ingetrazo_core::math::Ray::new(eye,(self.anchor-eye).normalize()));self.app.dispatch_tool_click(&context);let preview=self.app.active_tool.face_sweep_preview().expect("Actual Tool acquisition");assert_eq!(preview.face,self.face);assert_eq!(preview.normal,self.normal);},
     "drag"|"refuse"=>{let distance=row["distance"].as_f64().unwrap();assert!(distance.is_finite());let point=self.anchor+self.normal*distance;let eye=self.app.camera.eye();let context=ToolContext::new(point,ingetrazo_core::math::Ray::new(eye,(point-eye).normalize()));self.app.active_tool.on_move(&mut self.app.scene,&context);self.refuse=row["action"]=="refuse";},
     "retry"=>self.refuse=false,
     "cancel"=>{self.refuse=false;self.app.active_tool.on_cancel(&mut self.app.scene);self.app.vcb_text.clear();},
     "commit"=>{self.app.dispatch_tool_vcb(row["text"].as_str().unwrap()).unwrap();self.app.vcb_text.clear();},
     "undo"=>self.app.undo(),
     "typed"=>{self.app.vcb_text=row["text"].as_str().unwrap().into();},
     "save"=>self.app.save_to_path(&self.dir.join("saved.igz")),
     _=>panic!("Unknown sweep operation"),
    }self.nonce=nonce;
   }
  }
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){
  let prior={let mut ledger=system.memory_budget.ledger.lock().unwrap();let prior=ledger.total_limit;if self.refuse{ledger.total_limit=Some(ledger.total_committed());}prior};
  App::prepare_gpu(&mut self.app,system,resize);
  system.memory_budget.ledger.lock().unwrap().total_limit=prior;
  let label=self.app.active_tool.distance_label().map(|(distance,point)|serde_json::json!({"distance":distance,"point":point.to_array()}));
  let row=serde_json::json!({"nonce":self.nonce,"label":label,"dirty":self.app.document_dirty(),"screen_size":[system.egui_ctx.screen_rect().width(),system.egui_ctx.screen_rect().height()],"viewport":self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"gpu":system.face_sweep_observation_for_probe()});
  std::fs::write(self.dir.join("state.tmp"),serde_json::to_vec(&row).unwrap()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
 }
 fn gpu_error(&mut self,error:&str){self.app.gpu_error(error);}
}
pub fn run()->Result<(),Box<dyn Error>>{run_app::<Probe>(None)}
