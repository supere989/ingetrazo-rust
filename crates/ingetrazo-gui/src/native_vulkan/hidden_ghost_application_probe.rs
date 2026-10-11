//! Engineering-only actual application ghost-view workflow; no substitute rendering.
use super::{App,System,run_app};
use std::{path::PathBuf,error::Error,io::Read};
struct Probe {app:crate::IngeTrazoApp,nonce:u64,refuse:bool,dir:PathBuf}
impl App for Probe {
 fn title()->&'static str{"IngeTrazo actual Hidden View workflow"}
 fn new(system:&mut System,_:Option<PathBuf>)->Self {
  let path=PathBuf::from(std::env::var_os("INGETRAZO_GHOST_SCENE").expect("Ghost fixture scene"));
  let mut app=<crate::IngeTrazoApp as App>::new(system,Some(path));
  app.native_gpu_viewport_enabled=true;app.show_tray=false;
  app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);
  app.camera.target=glam::DVec3::ZERO;app.camera.distance=4.;app.camera.perspective=false;
  app.style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;
  app.style.background_color=[1.;4];app.style.show_sky=false;app.style.show_edges=false;app.style.show_profiles=false;
  app.mark_document_clean();
  Self{app,nonce:0,refuse:false,dir:PathBuf::from(std::env::var_os("INGETRAZO_GHOST_CONTROL").expect("Ghost control directory"))}
 }
 fn build_ui(&mut self,ctx:&egui::Context) {
  self.app.update_ui(ctx);ctx.request_repaint_after(std::time::Duration::from_millis(16));
  if let Ok(file)=std::fs::File::open(self.dir.join("request.json")){
   let mut bytes=Vec::new();file.take(1025).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=1024);
   let row:serde_json::Value=serde_json::from_slice(&bytes).unwrap();let nonce=row["nonce"].as_u64().unwrap();
   if nonce>self.nonce {
    match row["action"].as_str().unwrap() {
     "objects-on"=>{self.app.scene.set_hidden_view(true,true);},
     "objects-off"=>{self.app.scene.set_hidden_view(true,false);},
     "geometry-on"=>{self.app.scene.set_hidden_view(false,true);},
     "geometry-off"=>{self.app.scene.set_hidden_view(false,false);},
     "capture"=>self.app.add_saved_view("ghost-view".into()),
     "update"=>self.app.update_selected_saved_view(),
     "recall"=>self.app.apply_saved_view(0),
     "refuse"=>{self.app.scene.set_hidden_view(true,true);self.refuse=true;},
     "retry"=>self.refuse=false,
     "save"=>self.app.save_to_path(&self.dir.join("saved.igz")),
     _=>panic!("Unknown ghost application operation"),
    }
    self.nonce=nonce;
   }
  }
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){
  if let Some(world)=system.world.as_mut(){world.set_hidden_ghost_refusal_for_probe(self.refuse);}
  App::prepare_gpu(&mut self.app,system,resize);
  let row=serde_json::json!({"nonce":self.nonce,"objects":self.app.scene.show_hidden_objects,
   "geometry":self.app.scene.show_hidden_geometry,"dirty":self.app.document_dirty(),
   "scene_version":self.app.scene.version,"view_version":self.app.scene.view_version,
   "views":self.app.scene.saved_views.iter().map(|v|v.hidden_shown.clone()).collect::<Vec<_>>(),
   "screen_size":[system.egui_ctx.screen_rect().width(),system.egui_ctx.screen_rect().height()],"viewport":self.app.native_gpu_viewport_rect.map(|r|[r.min.x,r.min.y,r.max.x,r.max.y]),
   "gpu":system.hidden_ghost_observation_for_probe()});
  std::fs::write(self.dir.join("state.tmp"),serde_json::to_vec(&row).unwrap()).unwrap();
  std::fs::rename(self.dir.join("state.tmp"),self.dir.join("state.json")).unwrap();
 }
 fn gpu_error(&mut self,error:&str){self.app.gpu_error(error);}
}
pub fn run()->Result<(),Box<dyn Error>>{run_app::<Probe>(None)}
