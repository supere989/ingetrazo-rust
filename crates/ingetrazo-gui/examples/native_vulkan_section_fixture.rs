//! Full-viewport native Vulkan section oracle; uses production world pipelines.
use glam::DVec3;use ingetrazo_core::{scene::Scene,camera::{OrbitCamera,StandardView},section::SectionPlane};use ingetrazo_render::style::StyleConfig;
use ingetrazo_gui::native_vulkan::{App,System};
struct Fixture{app:ingetrazo_gui::IngeTrazoApp}
impl App for Fixture {
 fn title()->&'static str {"IngeTrazo native section fixture"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self {
 let case=std::env::var("INGETRAZO_SECTION_CASE").unwrap_or("closed".into());
let mut s=Scene::new();let mut layers=vec![];for z in [0.,1.,2.]{layers.push([[0.,0.,z],[1.,0.,z],[1.,1.,z],[0.,1.,z]].map(|p|s.mesh.add_vertex(DVec3::from_array(p))));}
s.mesh.add_face(layers[0].into_iter().rev().collect(),vec![]).unwrap();s.mesh.add_face(layers[2].to_vec(),vec![]).unwrap();
for i in 0..4{let j=(i+1)%4;for k in 0..2{s.mesh.add_face(vec![layers[k][i],layers[k][j],layers[k+1][j],layers[k+1][i]],vec![]).unwrap();}}
if case.starts_with("shared-slab"){let mut ring=layers[1].to_vec();if case.contains("reversed"){ring.reverse();}let f=s.mesh.add_face(ring,vec![]).unwrap();s.mesh.faces[f].interior=true;}
if case=="transparent"{for f in &mut s.mesh.faces{f.attrs.insert("opacity".into(),serde_json::json!(0.5));}}
if case=="duplicate"{s.mesh.faces.extend(s.mesh.faces.clone());}
if case=="coplanar-backface"{s.mesh=Default::default();let ids=[[0.,1.,1.5],[1.,1.,1.5],[1.,0.,1.5],[0.,0.,1.5]].map(|p|s.mesh.add_vertex(DVec3::from_array(p)));s.mesh.add_face(ids.to_vec(),vec![]).unwrap();}
s.section_planes.push(SectionPlane::new(DVec3::new(0.,0.,1.5),DVec3::Z,"A","A"));s.show_section_cuts=true;s.show_section_fill=true;
let mut camera=OrbitCamera::default();camera.set_standard_view(if case.ends_with("iso"){StandardView::Iso}else if case.ends_with("front"){StandardView::Front}else{StandardView::Top});camera.perspective=false;camera.target=DVec3::new(0.5,0.5,1.5);camera.distance=5.;
let mut style=StyleConfig::default();style.show_edges=false;style.show_profiles=false;style.show_sky=false;style.show_grid=false;style.section_fill_color=[1.,0.,1.,1.];
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
