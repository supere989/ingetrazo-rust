use ingetrazo_core::{scene::Scene,composition::ViewFrame};
struct Harness{scene:Scene,frame:ViewFrame,path:std::path::PathBuf,frames:usize}
impl eframe::App for Harness{fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{ui.painter().rect_filled(ui.max_rect(),0.,egui::Color32::WHITE);ingetrazo_gui::composer::render_model_projection(&self.scene,ui.painter(),ui.max_rect(),&self.frame);});
self.frames+=1;if self.frames==20{ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(())));}
for e in ctx.input(|i|i.events.clone()){if let egui::Event::Screenshot{image,..}=e{let p:Vec<u8>=image.pixels.iter().flat_map(|p|p.to_array()).collect();image::save_buffer(&self.path,&p,image.size[0]as u32,image.size[1]as u32,image::ColorType::Rgba8).unwrap();ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
ctx.request_repaint_after(std::time::Duration::from_millis(30));}}
fn main()->eframe::Result{let a:Vec<_>=std::env::args().collect();let scene=ingetrazo_formats::igz::load_igz(std::path::Path::new(&a[1])).unwrap();let mut frame=ViewFrame::default();frame.view_key="std:front".into();frame.rect.w=100.;frame.rect.h=100.;frame.scale_n=100.;frame.style="vectorial".into();frame.hidden_lines=true;frame.cut_fill="none".into();frame.pen_edge_mm=a[3].parse().unwrap();frame.pen_profile_mm=frame.pen_edge_mm;frame.pen_cut_mm=frame.pen_edge_mm;
eframe::run_native("Independent Composer Ink",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([800.,800.]),..Default::default()},Box::new(move |_|Ok(Box::new(Harness{scene,frame,path:a[2].clone().into(),frames:0}))))}
