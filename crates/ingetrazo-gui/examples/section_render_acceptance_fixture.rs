use glam::DVec3;use ingetrazo_core::{scene::Scene,camera::{OrbitCamera,StandardView},section::SectionPlane};use ingetrazo_render::{pipeline::{build_scene_render_data,render_scene_to_egui},style::StyleConfig};
struct Harness{scene:Scene,camera:OrbitCamera,style:StyleConfig,path:std::path::PathBuf,frames:usize}
impl eframe::App for Harness{fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{let r=ui.max_rect();self.camera.aspect=r.width()as f64/r.height()as f64;let data=build_scene_render_data(&self.scene,&self.camera,&self.style,None);render_scene_to_egui(&data,&self.camera,ui.painter(),r,&self.style,&Default::default());});
self.frames+=1;if self.frames==20{ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(())));}
for e in ctx.input(|i|i.events.clone()){if let egui::Event::Screenshot{image,..}=e{let p:Vec<u8>=image.pixels.iter().flat_map(|p|p.to_array()).collect();image::save_buffer(&self.path,&p,image.size[0]as u32,image.size[1]as u32,image::ColorType::Rgba8).unwrap();ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
ctx.request_repaint_after(std::time::Duration::from_millis(30));}}
fn main()->eframe::Result{let mut a=std::env::args().skip(1);let case=a.next().unwrap();let path=a.next().unwrap().into();let mut s=Scene::new();let mut layers=vec![];for z in [0.,1.,2.]{layers.push([[0.,0.,z],[1.,0.,z],[1.,1.,z],[0.,1.,z]].map(|p|s.mesh.add_vertex(DVec3::from_array(p))));}
s.mesh.add_face(layers[0].into_iter().rev().collect(),vec![]).unwrap();s.mesh.add_face(layers[2].to_vec(),vec![]).unwrap();
for i in 0..4{let j=(i+1)%4;for k in 0..2{s.mesh.add_face(vec![layers[k][i],layers[k][j],layers[k+1][j],layers[k+1][i]],vec![]).unwrap();}}
if case.starts_with("shared-slab"){let mut ring=layers[1].to_vec();if case.ends_with("reversed"){ring.reverse();}let f=s.mesh.add_face(ring,vec![]).unwrap();s.mesh.faces[f].interior=true;}
if case=="transparent"{for f in &mut s.mesh.faces{f.attrs.insert("opacity".into(),serde_json::json!(0.5));}}
if case=="duplicate"{s.mesh.faces.extend(s.mesh.faces.clone());}
s.section_planes.push(SectionPlane::new(DVec3::new(0.,0.,1.5),DVec3::Z,"A","A"));s.show_section_cuts=true;s.show_section_fill=true;
let mut camera=OrbitCamera::default();camera.set_standard_view(StandardView::Top);camera.perspective=false;camera.target=DVec3::new(0.5,0.5,1.5);camera.distance=5.;
let mut style=StyleConfig::default();style.show_edges=false;style.show_profiles=false;style.show_sky=false;style.show_grid=false;style.section_fill_color=[1.,0.,1.,1.];
eframe::run_native("Independent Section Stencil",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([800.,600.]),..Default::default()},Box::new(move |_|Ok(Box::new(Harness{scene:s,camera,style,path,frames:0}))))}
