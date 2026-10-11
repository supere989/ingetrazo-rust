// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_gui::{IngeTrazoApp,native_vulkan::{App,System,VulkanContext}};
 struct Probe{app:IngeTrazoApp,dir:std::path::PathBuf,nonce:u64,held:bool,budget:Option<u64>,camera:ingetrazo_core::camera::OrbitCamera,start:std::time::Instant,error:bool}
 impl App for Probe{
  fn title()->&'static str{"IngeTrazo billboard application probe"}
  fn new(system:&mut System,path:Option<std::path::PathBuf>)->Self{
   let mut app=<IngeTrazoApp as App>::new(system,path);app.shadows_enabled=false;app.camera.target=glam::DVec3::new(-1.2,-0.6,0.);app.camera.distance=10.;app.camera.yaw=-std::f64::consts::FRAC_PI_2;app.camera.pitch=1.2;app.camera.perspective=false;app.camera.fov_deg=45.;
   let camera=app.camera.clone();let dir=std::env::var("INGETRAZO_BILLBOARD_CONTROL_DIR").expect("owned control dir").into();
   Self{app,dir,nonce:0,held:false,budget:None,camera,start:std::time::Instant::now(),error:false}
  }
  fn build_ui(&mut self,ctx:&egui::Context){
   if let Ok(bytes)=std::fs::read(self.dir.join("request.json")){let request:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let nonce=request["nonce"].as_u64().unwrap();if nonce>self.nonce{
    self.nonce=nonce;match request["action"].as_str().unwrap(){
     "camera"=>self.app.camera.yaw+=0.2,
     "original"=>self.app.camera=self.camera.clone(),
     "hide"=>{self.app.scene.groups[0].hidden=true;self.app.scene.bump_view_version();},
     "restore"=>{self.app.scene.groups[0].hidden=false;self.app.scene.bump_view_version();},
     "refuse"=>{self.held=true;self.app.camera.yaw+=0.1;},
     "retry"=>self.held=false,
     "close"=>ctx.send_viewport_cmd(egui::ViewportCommand::Close),
     _=>panic!("unknown bounded probe action")
    }
   }}
   <IngeTrazoApp as App>::build_ui(&mut self.app,ctx);ctx.request_repaint();if self.error||self.start.elapsed().as_secs()>=15{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
  }
  fn prepare_gpu(&mut self,system:&mut System,resize:bool){
   if self.held&&self.budget.is_none(){let mut ledger=system.memory_budget.ledger.lock().unwrap();self.budget=Some(ledger.total_limit.unwrap_or(u64::MAX));ledger.total_limit=Some(ledger.total_committed());}
   if !self.held{if let Some(limit)=self.budget.take(){system.memory_budget.ledger.lock().unwrap().total_limit=if limit==u64::MAX{None}else{Some(limit)};}}
   <IngeTrazoApp as App>::prepare_gpu(&mut self.app,system,resize);
   if resize{return;}
   let state=serde_json::json!({"nonce":self.nonce,"held":self.held,"screen_size":system.egui_ctx.input(|i|[i.screen_rect.width(),i.screen_rect.height()]),"viewport":self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),"camera_yaw":self.app.camera.yaw,"hidden":self.app.scene.groups[0].hidden,"observation":system.billboard_observation_for_probe()});
   std::fs::write(self.dir.join("state.tmp"),serde_json::to_vec_pretty(&state).unwrap()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
  }
  fn gpu_error(&mut self,error:&str){if self.held{eprintln!("EXPECTED_BILLBOARD_REFUSAL {error}");}else{self.error=true;<IngeTrazoApp as App>::gpu_error(&mut self.app,error);}}
  fn clean(&mut self,context:&VulkanContext){assert!(!self.error&&self.nonce>=7,"actual billboard app incomplete");<IngeTrazoApp as App>::clean(&mut self.app,context);eprintln!("BILLBOARD_APPLICATION_PASS nonce={}",self.nonce);}
 }
 ingetrazo_gui::native_vulkan::run_app::<Probe>(Some(std::env::args().nth(1).ok_or("INPUT.igz")?.into()))
}
#[cfg(not(any(target_os="linux",target_os="windows")))]fn main(){eprintln!("Vulkan diagnostic requires Linux/Windows");}
