//! Two crossing opaque faces: fragment depth must beat centroid painter ordering.
use ingetrazo_gui::native_vulkan::{App,System};
use glam::DVec3;
struct Fixture {app:ingetrazo_gui::IngeTrazoApp}
impl App for Fixture {
 fn title()->&'static str {"IngeTrazo native world depth fixture"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self {
  let mut app=ingetrazo_gui::IngeTrazoApp::new();app.native_gpu_viewport_enabled=std::env::var_os("INGETRAZO_VULKAN_WORLD").is_some();app.show_tray=false;
  for (points,color) in [([DVec3::new(-1.,-1.,1.),DVec3::new(1.,-1.,-1.),DVec3::new(0.,1.,0.)],[1.,0.,0.,1.]),([DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(0.,1.,0.)],[0.,0.,1.,1.])] {
   let vertices=points.map(|p|app.scene.mesh.add_vertex(p));let face=app.scene.mesh.add_face(vertices.to_vec(),vec![]).unwrap();app.scene.mesh.faces[face].attrs.insert("color".into(),serde_json::json!(color));
  }
  if let Ok(mode)=std::env::var("INGETRAZO_WORLD_SECTION") {
   let normal=if mode=="reverse" {-DVec3::X}else{DVec3::X};
   let mut plane=ingetrazo_core::section::SectionPlane::new(DVec3::ZERO,normal,"Cut","A");plane.active=mode!="inactive";
   app.scene.section_planes.push(plane);app.scene.show_section_fill=false;app.scene.show_section_cuts=mode!="disabled";
  }
  app.scene.bump_version();app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);app.camera.target=DVec3::ZERO;app.camera.distance=4.;app.camera.perspective=false;app.mark_document_clean();
  Self{app}
 }
 fn build_ui(&mut self,ctx:&egui::Context){self.app.update_ui(ctx);}
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){App::prepare_gpu(&mut self.app,system,resize);}
 fn gpu_error(&mut self,error:&str){panic!("GPU fixture failed: {error}");}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
