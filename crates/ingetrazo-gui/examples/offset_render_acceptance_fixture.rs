use glam::DVec3;use ingetrazo_gui::{IngeTrazoApp,viewport::render_viewport};use ingetrazo_tools::base::ToolType;use ingetrazo_core::camera::StandardView;
struct Harness{app:IngeTrazoApp,dir:std::path::PathBuf,capture:String}
impl eframe::App for Harness{
 fn update(&mut self,ctx:&egui::Context,_frame:&mut eframe::Frame){
  egui::CentralPanel::default().show(ctx,|ui|{
   let rect=ui.available_rect_before_wrap();render_viewport(&mut self.app,ui);
   let matrix=self.app.camera.projection_matrix()*self.app.camera.view_matrix();
   let project=|p:DVec3|{let c=matrix*p.extend(1.);let n=c.truncate()/c.w;[rect.left() as f64+(n.x+1.)*0.5*rect.width()as f64,rect.top()as f64+(1.-n.y)*0.5*rect.height()as f64]};
   let rows=serde_json::json!({"faces":self.app.scene.mesh.faces.len(),"undo":self.app.undo_stack.len(),"preview":self.app.active_tool.rubber_band_lines().iter().map(|(a,b)|[a.to_array(),b.to_array()]).collect::<Vec<_>>(),"click":project(DVec3::new(1.3,0.,0.)),"move":project(DVec3::new(2.7,0.5,0.)),"pointer":ctx.input(|i|i.pointer.hover_pos().map(|p|[p.x,p.y]))});
   if self.app.scene.mesh.faces.len()==2 && !self.dir.join("committed.igz").exists(){ingetrazo_formats::igz::save_igz(&self.app.scene,&self.dir.join("committed.igz")).unwrap();}
   let pending=self.dir.join("state.tmp");
 std::fs::write(&pending,serde_json::to_vec(&rows).unwrap()).unwrap();
 std::fs::rename(pending,self.dir.join("state.json")).unwrap();
  });
  if let Ok(name)=std::fs::read_to_string(self.dir.join("capture.txt")){if !name.is_empty() && name!=self.capture{self.capture=name.clone();ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(name)));}}
  for event in ctx.input(|i|i.events.clone()){if let egui::Event::Screenshot{image,user_data,..}=event{let data=user_data.data.expect("capture identity");let name=data.downcast_ref::<String>().expect("capture request name");let pixels:Vec<u8>=image.pixels.iter().flat_map(|p|p.to_array()).collect();image::save_buffer(self.dir.join(format!("{}.png",name)),&pixels,image.size[0]as u32,image.size[1]as u32,image::ColorType::Rgba8).unwrap();}}
  ctx.request_repaint_after(std::time::Duration::from_millis(30));
 }
}
fn main()->eframe::Result{let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());let mut app=IngeTrazoApp::new();let ids=[DVec3::ZERO,DVec3::X*4.,DVec3::new(4.,4.,0.),DVec3::Y*4.].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]);app.camera.set_standard_view(StandardView::Top);app.camera.perspective=false;app.camera.target=DVec3::new(2.,2.,0.);app.camera.distance=8.;app.set_tool(ToolType::Offset);
 eframe::run_native("Independent Offset viewport",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([1000.,700.]),..Default::default()},Box::new(move |_|Ok(Box::new(Harness{app,dir,capture:String::new()}))))}
