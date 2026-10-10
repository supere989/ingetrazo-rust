//! Camera navigation state; geometry and model history stay in their existing owners.
use glam::{DVec2,DVec3};
use ingetrazo_core::{camera::OrbitCamera,math::{Plane,Ray},screen_pick,walkthrough::{self,WalkModifiers}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Mode { Position,Look,Walk }
impl Mode { pub fn hint(self)->&'static str {match self {Self::Position=>"Position Camera — click or drag from where to stand",Self::Look=>"Look Around — drag to turn",Self::Walk=>"Walk — drag; Ctrl run, Shift rise/strafe, Alt through walls"}} }
pub struct Navigation {
 pub mode:Option<Mode>, pub eye_height:f64,
 press:Option<(DVec2,DVec3)>, last:Option<DVec2>, look_at:Option<DVec3>,
 dragged:bool, pub base:Option<DVec3>, accumulator:f64,
}
impl Default for Navigation {
 fn default()->Self{Self{mode:None,eye_height:1.68,press:None,last:None,look_at:None,dragged:false,base:None,accumulator:0.}}
}
impl Navigation {
 pub fn cancel(&mut self){self.press=None;self.last=None;self.look_at=None;self.dragged=false;self.accumulator=0.;}
 pub fn activate(&mut self,mode:Mode){self.cancel();self.mode=Some(mode);}
 pub fn place(&mut self,camera:&mut OrbitCamera,base:DVec3,look:Option<DVec3>)->bool{
  let eye=base+DVec3::Z*self.eye_height;
  let mut direction=look.map(|p|p-eye).filter(|d|d.length()>1e-6).unwrap_or_else(||{
   let f=camera.forward();let h=DVec3::new(f.x,f.y,0.);if h.length()>1e-6{h.normalize()}else{DVec3::Y}
  });
  if !direction.is_finite(){return false;}
  if direction.length_squared()<1e-18{direction=DVec3::Y;}
  if !camera.look_from(eye,direction){return false;}
  camera.perspective=true;self.base=Some(base);true
 }
}
impl crate::IngeTrazoApp {
 pub fn navigation_height(&mut self,text:&str)->Result<(),String>{
  let mode=self.navigation.mode.ok_or("Camera navigation is inactive")?;
  let height=ingetrazo_core::units::parse_length(text,self.scene.unit)?;
  if !height.is_finite() || (mode==Mode::Position && height<=0.) {return Err("Invalid camera height".into());}
  let eye=self.camera.eye();
  let new_eye=if mode==Mode::Position {
   self.navigation.base.map(|base|base+DVec3::Z*height)
  }else{Some(DVec3::new(eye.x,eye.y,height))};
  if let Some(new_eye)=new_eye {if !self.camera.look_from(new_eye,self.camera.forward()){return Err("Invalid camera placement".into());}}
  if mode==Mode::Position {
   self.document_session.preferences.walk_eye_height=height;
   self.document_session.preferences.clamp();
   self.navigation.eye_height=self.document_session.preferences.walk_eye_height;
   self.persist_preferences();
  }
  Ok(())
 }
 pub fn activate_navigation(&mut self,mode:Mode){
  self.set_tool(ingetrazo_tools::base::ToolType::Select);
  self.navigation.activate(mode);self.vcb_text.clear();
  self.status_message=mode.hint().into();
 }
}
pub fn input(app:&mut crate::IngeTrazoApp,ui:&mut egui::Ui,rect:egui::Rect,response:&egui::Response){
 let Some(mode)=app.navigation.mode else{return};
 let pointer=ui.input(|i|i.pointer.clone());
 if ui.input(|i|i.key_pressed(egui::Key::Escape)||!i.focused){app.navigation.cancel();return;}
 let position=pointer.hover_pos().map(|p|DVec2::new((p.x-rect.min.x)as f64,(p.y-rect.min.y)as f64));
 let world=position.filter(|_|mode==Mode::Position).map(|p|{
  let ray=app.camera.screen_to_ray(p.x,p.y,rect.width()as f64,rect.height()as f64);
  screen_pick::hover_face(&app.scene,&ray).map(|hit|hit.0).or_else(||Plane::new(DVec3::ZERO,DVec3::Z).intersect_ray(&ray).filter(|t|*t>0.).map(|t|ray.at(t)))
 });
 if response.hovered() && pointer.button_pressed(egui::PointerButton::Primary){
  if let Some(p)=position {if mode!=Mode::Position||world.flatten().is_some(){
   app.navigation.press=Some((p,world.flatten().unwrap_or(DVec3::ZERO)));app.navigation.last=Some(p);app.navigation.dragged=false;app.navigation.accumulator=0.;
  }}
 }
 if pointer.primary_down(){
  if let (Some((anchor,_)),Some(p))=(app.navigation.press,position){
   match mode{
    Mode::Position=>{if p.distance(anchor)>6.{app.navigation.dragged=true;}if app.navigation.dragged{app.navigation.look_at=world.flatten();}},
    Mode::Look=>{if let Some(last)=app.navigation.last{
     let d=p-last;app.camera.turn(d.x*180./rect.width().max(1.)as f64,-d.y*90./rect.height().max(1.)as f64);
    }app.navigation.last=Some(p);},
    Mode::Walk=>{
     let dt=ui.input(|i|i.stable_dt)as f64;app.navigation.accumulator=(app.navigation.accumulator+dt.max(0.)).min(0.231);
     let modifiers=ui.input(|i|WalkModifiers{run:i.modifiers.ctrl,shift:i.modifiers.shift,through_walls:i.modifiers.alt});
     while app.navigation.accumulator>=0.033{
      app.navigation.accumulator-=0.033;
      let scene=&app.scene;
      walkthrough::step(&mut app.camera,p.x-anchor.x,anchor.y-p.y,0.033,app.navigation.eye_height,modifiers,|o,d|screen_pick::hover_face(scene,&Ray::new(o,d)).map(|h|h.2));
     }
     ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
   }
  }
 }
 if pointer.button_released(egui::PointerButton::Primary){
  if mode==Mode::Position {if let Some((_,base))=app.navigation.press{
   let look=if app.navigation.dragged{app.navigation.look_at}else{None};
   if app.navigation.place(&mut app.camera,base,look){app.navigation.mode=Some(Mode::Look);}
  }}
  app.navigation.cancel();
 }
 if mode==Mode::Walk {if let Some((anchor,_))=app.navigation.press{
  let center=egui::pos2(rect.min.x+anchor.x as f32,rect.min.y+anchor.y as f32);
  let painter=ui.painter_at(rect);let stroke=egui::Stroke::new(1.,egui::Color32::WHITE);
  painter.line_segment([center-egui::vec2(8.,0.),center+egui::vec2(8.,0.)],stroke);
  painter.line_segment([center-egui::vec2(0.,8.),center+egui::vec2(0.,8.)],stroke);
 }}
}

#[cfg(test)]mod tests {
 use super::*;
 #[test]fn camera_placement_preserves_model_and_hands_off_state_independently(){
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;let version=app.scene.version;let undo=app.undo_stack.len();
  app.activate_navigation(Mode::Position);assert_eq!(app.navigation.mode,Some(Mode::Position));
  let base=DVec3::new(2.,3.,0.4);assert!(app.navigation.place(&mut app.camera,base,None));
  assert!(app.camera.eye().distance(base+DVec3::Z*1.68)<1e-12);assert!(app.camera.perspective);
  assert_eq!(app.scene.version,version);assert_eq!(app.undo_stack.len(),undo);
  app.set_tool(ingetrazo_tools::base::ToolType::Line);assert_eq!(app.navigation.mode,None);
 }
 #[test]fn cancellation_discards_gesture_and_stops_ticks(){
  let mut n=Navigation::default();n.activate(Mode::Walk);n.press=Some((DVec2::ZERO,DVec3::ZERO));n.accumulator=0.2;n.cancel();
  assert!(n.press.is_none());assert_eq!(n.accumulator,0.);assert_eq!(n.mode,Some(Mode::Walk));
 }
}

#[cfg(test)]mod height_tests {
 use super::*;
 #[test]fn measurement_routes_camera_height_without_model_transaction(){
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  app.activate_navigation(Mode::Position);let version=app.scene.version;let undo=app.undo_stack.len();
  app.dispatch_tool_vcb("300cm").unwrap();assert_eq!(app.navigation.eye_height,3.);
  let base=DVec3::new(2.,3.,0.4);assert!(app.navigation.place(&mut app.camera,base,None));
  app.activate_navigation(Mode::Look);app.dispatch_tool_vcb("2m").unwrap();
  assert!((app.camera.eye().z-2.).abs()<1e-12);
  let camera=app.camera.clone();assert!(app.dispatch_tool_vcb("NaN").is_err());assert_eq!(app.camera,camera);
  assert_eq!(app.scene.version,version);assert_eq!(app.undo_stack.len(),undo);
 }
 #[test]fn typed_offset_survives_settings_restart(){
  let root=std::env::temp_dir().join(format!("ingetrazo-navigation-settings-{}",uuid::Uuid::new_v4()));
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  app.attach_session_store(crate::session_state::SessionStore::isolated(&root));
  app.activate_navigation(Mode::Position);app.dispatch_tool_vcb("185cm").unwrap();
  let mut restarted=crate::IngeTrazoApp::new();restarted.system_clipboard=None;
  restarted.attach_session_store(crate::session_state::SessionStore::isolated(&root));
  assert_eq!(restarted.navigation.eye_height,1.85);
  std::fs::remove_dir_all(root).unwrap();
 }
}
