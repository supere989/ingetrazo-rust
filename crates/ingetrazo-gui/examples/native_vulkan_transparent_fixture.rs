//! Full-viewport native Vulkan section oracle; uses production world pipelines.
use glam::DVec3;use ingetrazo_core::{camera::{OrbitCamera,StandardView}};use ingetrazo_render::style::StyleConfig;
use ingetrazo_gui::native_vulkan::{App,System};
struct Fixture{app:ingetrazo_gui::IngeTrazoApp}
impl App for Fixture {
 fn title()->&'static str {"IngeTrazo native transparent fixture"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self {
 let input=std::env::var("INGETRAZO_ALPHA_INPUT").expect("original alpha input directory");let case=std::env::var("INGETRAZO_ALPHA_CASE").unwrap_or("red-half-white".into());
 let s=ingetrazo_formats::igz::load_igz(&std::path::Path::new(&input).join(format!("{case}.igz"))).unwrap();
 let original:serde_json::Value=serde_json::from_slice(&std::fs::read(std::path::Path::new(&input).join("original-alpha-results.json")).unwrap()).unwrap();let row=original["cases"].as_array().unwrap().iter().find(|v|v["case"]==case).unwrap();
 let mut camera=OrbitCamera::default();camera.set_standard_view(StandardView::Top);camera.perspective=false;camera.target=DVec3::ZERO;camera.distance=4.;
 let mut style=StyleConfig::default();style.show_edges=false;style.show_profiles=false;style.show_sky=false;style.show_grid=false;style.background_color=[row["background"][0].as_f64().unwrap()as f32,row["background"][1].as_f64().unwrap()as f32,row["background"][2].as_f64().unwrap()as f32,1.];
 let mut app=ingetrazo_gui::IngeTrazoApp::new();app.scene=s;app.camera=camera;app.style=style;app.native_gpu_viewport_enabled=true;app.mark_document_clean();Self{app}
 }
 fn build_ui(&mut self,ctx:&egui::Context){
  egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{let rect=ui.max_rect();self.app.camera.aspect=rect.width()as f64/rect.height()as f64;self.app.native_gpu_viewport_rect=Some(rect);if let Some(texture)=self.app.native_gpu_viewport_texture{ui.painter().image(texture,rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::WHITE);}});
  if ctx.input(|i|i.viewport().close_requested()){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
 }
 fn prepare_gpu(&mut self,system:&mut System,resize:bool){
  if system.world.is_none(){if let Ok(limit)=std::env::var("INGETRAZO_SECTION_STORAGE_LIMIT"){let mut world=ingetrazo_gui::native_vulkan::world::World::new(&system.vulkan_context,system.memory_budget.clone()).unwrap();world.set_contour_storage_limit_for_probe(limit.parse().unwrap()).unwrap();system.world=Some(world);}}
  let before=system.world.as_ref().map(|w|w.uploads).unwrap_or(0);App::prepare_gpu(&mut self.app,system,resize);
  if let Some(world)=&system.world {if world.uploads!=before{eprintln!("Contour metrics: {:?}",world.contour_metrics());}}
 }
 fn gpu_error(&mut self,error:&str){panic!("GPU section fixture failed: {error}");}
}
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Fixture>(None)}
