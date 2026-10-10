use ingetrazo_gui::{IngeTrazoApp,native_vulkan::{App,System},navigation::Mode};
use glam::DVec3;
struct Fixture{app:IngeTrazoApp,dir:std::path::PathBuf,frames:u64}
impl App for Fixture{
 fn title()->&'static str{"IngeTrazo native navigation input"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
  let mut app=IngeTrazoApp::new();app.system_clipboard=None;app.set_language("en");app.show_tray=false;app.show_toolbar=false;
  app.native_composer.factory=Some(system.enable_cad2d().unwrap().factory().into());app.native_gpu_viewport_enabled=true;
  let ids=[DVec3::new(-10.,-10.,0.),DVec3::new(10.,-10.,0.),DVec3::new(10.,10.,0.),DVec3::new(-10.,10.,0.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]);
  let ids=[DVec3::new(-2.,4.,0.),DVec3::new(2.,4.,0.),DVec3::new(2.,4.,3.),DVec3::new(-2.,4.,3.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]);
  app.camera.look_from(DVec3::Z*1.68,DVec3::Y);app.camera.perspective=true;app.activate_navigation(Mode::FirstPerson);
  Self{app,dir:std::env::args().nth(1).unwrap().into(),frames:0}
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){<IngeTrazoApp as App>::prepare_gpu(&mut self.app,system,resize);}
 fn build_ui(&mut self,ctx:&egui::Context){
  self.app.update_ui(ctx);self.frames+=1;
  let rect=self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]);
  let state=serde_json::json!({"frames":self.frames,"eye":self.app.camera.eye().to_array(),"forward":self.app.camera.forward().to_array(),"yaw":self.app.camera.yaw,"pitch":self.app.camera.pitch,"mode":format!("{:?}",self.app.navigation.mode),"tool":format!("{:?}",self.app.active_tool_type),"viewport":rect,"undo":self.app.undo_stack.len(),"version":self.app.scene.version,"faces":self.app.scene.mesh.faces.len(),"focused":ctx.input(|i|i.focused),"status":self.app.status_message});
  std::fs::write(self.dir.join("state.tmp"),state.to_string()).unwrap();std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
  ctx.request_repaint_after(std::time::Duration::from_millis(16));
 }
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
