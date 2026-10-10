use glam::DVec3;
use ingetrazo_gui::{IngeTrazoApp,navigation::{self,Mode}};
fn frame(app:&mut IngeTrazoApp,ctx:&egui::Context,time:f64,events:Vec<egui::Event>){
 let raw=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))),time:Some(time),predicted_dt:0.170,focused:true,events,..Default::default()};
 let _=ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{let rect=ui.available_rect_before_wrap();let response=ui.allocate_rect(rect,egui::Sense::click_and_drag());navigation::input(app,ui,rect,&response);});});
}
fn main(){let mut rows=vec![];for (name,mode) in [("walk-stalled",Mode::Walk),("first-person-stalled",Mode::FirstPerson)]{
 let mut app=IngeTrazoApp::new();let ctx=egui::Context::default();app.camera.distance=10.;app.camera.look_from(DVec3::new(0.,0.,1.68),DVec3::Y);app.activate_navigation(mode);
 frame(&mut app,&ctx,0.,vec![]);let before=app.camera.eye();
 let events=if mode==Mode::Walk{vec![egui::Event::PointerMoved(egui::pos2(400.,300.)),egui::Event::PointerButton{pos:egui::pos2(400.,300.),button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]}else{vec![egui::Event::Key{key:egui::Key::W,physical_key:None,pressed:true,repeat:false,modifiers:egui::Modifiers::NONE}]};
 // The first frame dispatches the actual press/key. Freeze its result separately.
 frame(&mut app,&ctx,0.001,events);let after_start=app.camera.eye();
 frame(&mut app,&ctx,0.171,if mode==Mode::Walk{vec![egui::Event::PointerMoved(egui::pos2(400.,240.))]}else{vec![]});let after=app.camera.eye();
 rows.push(serde_json::json!({"name":name,"before":before.to_array(),"after_start":after_start.to_array(),"after":after.to_array(),"delta":(after-after_start).to_array()}));
 }println!("{}",serde_json::json!({"cases":rows}));}
