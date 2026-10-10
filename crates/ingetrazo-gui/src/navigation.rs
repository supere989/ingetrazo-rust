//! Camera navigation state; geometry and model history stay in their existing owners.
use glam::{DVec2,DVec3};
use ingetrazo_core::{camera::OrbitCamera,math::Ray,screen_pick,visible_ray_index::VisibleRayIndex,walkthrough::{self,WalkModifiers}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Mode { Position,Look,Walk,FirstPerson }
impl Mode { pub fn hint(self)->&'static str {match self {Self::Position=>"Position Camera — click or drag from where to stand",Self::Look=>"Look Around — drag to turn",Self::Walk=>"Walk — drag; Ctrl run, Shift rise/strafe, Alt through walls",Self::FirstPerson=>"First Person — WASD move, Q/E descend/rise; Shift run, Alt through walls; drag either button to look"}} }
pub struct Navigation {
 pub mode:Option<Mode>, pub eye_height:f64,
 press:Option<(DVec2,DVec3)>, last:Option<DVec2>, look_at:Option<DVec3>,
 dragged:bool, pub base:Option<DVec3>, accumulator:f64,
 held:[bool;6], mouse_captured:bool, cursor_locked_sent:bool,
 bounds_center:DVec3, collision_key:Option<(u64,u64,u64)>, collision:Option<VisibleRayIndex>, collision_error:Option<String>,
}
impl Default for Navigation {
 fn default()->Self{Self{mode:None,eye_height:1.68,press:None,last:None,look_at:None,dragged:false,base:None,accumulator:0.,held:[false;6],mouse_captured:false,cursor_locked_sent:false,bounds_center:DVec3::ZERO,collision_key:None,collision:None,collision_error:None}}
}
impl Navigation {
 // Remember refused admission until revision changes; direct queries preserve correctness.
 fn prepare_collision(&mut self,scene:&ingetrazo_core::scene::Scene,model_revision:u64){
  let key=(scene.version,scene.view_version,model_revision);
  if self.collision_key==Some(key){return;}
  self.collision=None;self.collision_error=None;self.collision_key=Some(key);
  self.bounds_center=first_point_bounds_center(scene);
  match VisibleRayIndex::new(scene,128*1024*1024){
   Ok(index)=>self.collision=Some(index),Err(error)=>self.collision_error=Some(error),
  }
 }
 fn hit(&self,scene:&ingetrazo_core::scene::Scene,ray:&Ray)->Option<(DVec3,DVec3,f64)>{
  match &self.collision {Some(index)=>index.hit(scene,ray),None=>screen_pick::hover_face(scene,ray)}
 }
 fn distance(&self,scene:&ingetrazo_core::scene::Scene,ray:&Ray)->Option<f64>{self.hit(scene,ray).map(|hit|hit.2)}
 pub fn sync_cursor(&mut self,ctx:&egui::Context){
  let captured=self.mouse_captured&&self.mode==Some(Mode::FirstPerson);
  if captured==self.cursor_locked_sent{return;}
  ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(if captured{egui::CursorGrab::Locked}else{egui::CursorGrab::None}));
  ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(!captured));self.cursor_locked_sent=captured;
 }
 pub fn claims_key(&self,key:egui::Key,modifiers:egui::Modifiers)->bool{
  self.mode==Some(Mode::FirstPerson)&&!modifiers.ctrl&&!modifiers.command&&movement_key(key).is_some()
 }
 fn key_event(&mut self,key:egui::Key,pressed:bool,modifiers:egui::Modifiers){
  if let Some(index)=movement_key(key){if !pressed||self.claims_key(key,modifiers){self.held[index]=pressed;}}
 }
 fn axes(&self)->[i8;3]{[self.held[0]as i8-self.held[1]as i8,self.held[2]as i8-self.held[3]as i8,self.held[4]as i8-self.held[5]as i8]}
 pub fn cancel(&mut self){self.held=[false;6];self.mouse_captured=false;self.press=None;self.last=None;self.look_at=None;self.dragged=false;self.accumulator=0.;}
 pub fn activate(&mut self,mode:Mode){self.cancel();self.mode=Some(mode);}
 pub fn preview_line(&self)->Option<(DVec3,DVec3)>{
  self.press.zip(self.look_at).map(|((_,base),look)|(base+DVec3::Z*self.eye_height,look))
 }
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
fn movement_key(key:egui::Key)->Option<usize>{match key{egui::Key::W=>Some(0),egui::Key::S=>Some(1),egui::Key::D=>Some(2),egui::Key::A=>Some(3),egui::Key::E=>Some(4),egui::Key::Q=>Some(5),_=>None}}
impl crate::IngeTrazoApp {
 pub fn navigation_suspended(&self)->bool{
  self.scene.active_sheet_index.is_some()||self.section_name_prompt.is_some()||self.show_open_dialog||self.show_save_dialog||self.show_preferences_dialog||self.show_command_search||self.show_geographic_import_dialog||self.show_texture_edit_dialog||self.show_render_dialog
 }
 pub fn navigation_claims_key(&self,key:egui::Key,modifiers:egui::Modifiers)->bool{!self.navigation_suspended()&&self.navigation.claims_key(key,modifiers)}

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
 pub fn navigation_measurement(&self)->Option<String>{
  self.navigation.mode.map(|mode|ingetrazo_core::units::format_length(if mode==Mode::Position{self.navigation.eye_height}else{self.camera.eye().z},self.scene.unit,2))
 }
 pub fn activate_navigation(&mut self,mode:Mode){
  self.set_tool(ingetrazo_tools::base::ToolType::Select);
  self.navigation.activate(mode);self.vcb_text.clear();
  self.status_message=mode.hint().into();
 }
}
pub fn input(app:&mut crate::IngeTrazoApp,ui:&mut egui::Ui,rect:egui::Rect,response:&egui::Response)->Option<ingetrazo_core::snap::SnapResult>{
 if app.navigation_suspended(){app.navigation.cancel();app.navigation.sync_cursor(ui.ctx());return None;}
 let Some(mode)=app.navigation.mode else{return None};
 let pointer=ui.input(|i|i.pointer.clone());
 if ui.input(|i|i.key_pressed(egui::Key::Escape)||!i.focused){app.navigation.cancel();app.navigation.sync_cursor(ui.ctx());return None;}
 if matches!(mode,Mode::Position|Mode::Walk|Mode::FirstPerson){app.navigation.prepare_collision(&app.scene,app.model_revision);}
 if mode==Mode::FirstPerson {
  if ui.ctx().wants_keyboard_input(){app.navigation.held=[false;6];}
  else {let events=ui.input(|i|i.events.clone());for event in events{if let egui::Event::Key{key,pressed,modifiers,..}=event{app.navigation.key_event(key,pressed,modifiers);}}}
  let axes=app.navigation.axes();
  if axes!=[0,0,0]{
   let dt=ui.input(|i|i.stable_dt)as f64;app.navigation.accumulator=(app.navigation.accumulator+dt.max(0.)).min(0.112);
   let modifiers=ui.input(|i|i.modifiers);
   while app.navigation.accumulator>=0.016 {
    app.navigation.accumulator-=0.016;let scene=&app.scene;let navigation=&app.navigation;
    walkthrough::first_person_step(&mut app.camera,axes,0.016,navigation.eye_height,modifiers.shift,modifiers.alt,|o,d|navigation.distance(scene,&Ray::new(o,d)));
   }
   ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
  }else{app.navigation.accumulator=0.;}
  let was_captured=app.navigation.mouse_captured;
  if response.hovered()&&(pointer.button_pressed(egui::PointerButton::Primary)||pointer.button_pressed(egui::PointerButton::Secondary)){
   app.navigation.mouse_captured=true;
  }
  if !pointer.primary_down()&&!pointer.secondary_down(){app.navigation.mouse_captured=false;}
  if app.navigation.mouse_captured {
   app.navigation.sync_cursor(ui.ctx());
   let raw=ui.input(|i|i.events.iter().filter_map(|e|if let egui::Event::MouseMoved(d)=e{Some(*d)}else{None}).reduce(|a,b|a+b));
   let delta=raw.unwrap_or_else(||pointer.delta());let rate=app.document_session.preferences.look_sensitivity as f64/500.;
   if was_captured{app.camera.turn(delta.x as f64*rate,-delta.y as f64*rate);}
  }else{app.navigation.sync_cursor(ui.ctx());}
  return None;
 }
 let position=pointer.hover_pos().map(|p|{let p=DVec2::new((p.x-rect.min.x)as f64,(p.y-rect.min.y)as f64);if mode==Mode::Position{p.round()}else{p}});
 let mut hovered_snap=None;
 let world=position.filter(|_|mode==Mode::Position).and_then(|p|{
  let ray=crate::navigation_ray::cursor_ray(&app.camera,p.x.round(),p.y.round(),rect.width()as f64,rect.height()as f64)?;
  let hit=app.navigation.hit(&app.scene,&ray);
  // Position has no captured start plane: source uses the ordinary first-point
  // face/image/standard-view fallback during both click and stand-look drag.
  let (plane_point,normal)=crate::viewport::cursor_work_plane_with_center(app,&ray,Some(app.navigation.bounds_center));
  let candidate=crate::navigation_ray::plane_hit(&ray,plane_point,normal)?;
  app.snap_engine.note_model_revision(app.model_revision);
  let context=ingetrazo_core::snap_screen::ScreenSnapContext{viewport:screen_pick::PixelViewport::new(&app.camera,rect.width()as f64,rect.height()as f64),cursor:p,candidate,plane_normal:normal};
  let result=app.snap_engine.snap_screen_with_policy(&ray,&app.scene,None,None,&context,&ingetrazo_core::snap_screen::ScreenSnapPolicy{face_under_cursor:hit.is_some(),..Default::default()});
  app.snap_engine.note_screen_result(&result,false,ui.input(|i|i.time));
  app.snap_engine.note_inference_result(&result);
  let point=result.point;hovered_snap=Some(result);
  Some(point)
 });
 if response.hovered() && pointer.button_pressed(egui::PointerButton::Primary){
  if let Some(p)=position {if mode!=Mode::Position||world.is_some(){
   app.navigation.press=Some((p,world.unwrap_or(DVec3::ZERO)));app.navigation.last=Some(p);app.navigation.dragged=false;app.navigation.accumulator=0.;
  }}
 }
 if pointer.primary_down(){
  if let (Some((anchor,_)),Some(p))=(app.navigation.press,position){
   match mode{
    Mode::Position=>{if p.distance(anchor)>6.{app.navigation.dragged=true;}if app.navigation.dragged{app.navigation.look_at=world;}},
    Mode::Look=>{if let Some(last)=app.navigation.last{
     let d=p-last;app.camera.turn(d.x*180./rect.width().max(1.)as f64,-d.y*90./rect.height().max(1.)as f64);
    }app.navigation.last=Some(p);},
    Mode::FirstPerson=>unreachable!("first-person input handled above"),
    Mode::Walk=>{
     let dt=ui.input(|i|i.stable_dt)as f64;app.navigation.accumulator=(app.navigation.accumulator+dt.max(0.)).min(0.231);
     let modifiers=ui.input(|i|WalkModifiers{run:i.modifiers.ctrl,shift:i.modifiers.shift,through_walls:i.modifiers.alt});
     while app.navigation.accumulator>=0.033{
      app.navigation.accumulator-=0.033;
      let scene=&app.scene;let navigation=&app.navigation;
      walkthrough::step(&mut app.camera,p.x-anchor.x,anchor.y-p.y,0.033,app.navigation.eye_height,modifiers,|o,d|navigation.distance(scene,&Ray::new(o,d)));
     }
     ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
   }
  }
 }
 if pointer.button_released(egui::PointerButton::Primary){
  if mode==Mode::Position {if let Some((_,base))=app.navigation.press{
   let look=if app.navigation.dragged{app.navigation.look_at}else{None};
   if app.navigation.place(&mut app.camera,base,look){app.navigation.mode=Some(Mode::Look);app.status_message=Mode::Look.hint().into();app.vcb_text.clear();}
  }}
  app.navigation.cancel();
 }
 if mode==Mode::Walk {if let Some((anchor,_))=app.navigation.press{
  let center=egui::pos2(rect.min.x+anchor.x as f32,rect.min.y+anchor.y as f32);
  let painter=ui.painter_at(rect);let stroke=egui::Stroke::new(1.5,egui::Color32::from_rgba_unmultiplied(30,30,30,230));
  painter.line_segment([center-egui::vec2(10.,0.),center+egui::vec2(10.,0.)],stroke);
  painter.line_segment([center-egui::vec2(0.,10.),center+egui::vec2(0.,10.)],stroke);
  painter.circle_stroke(center,4.,egui::Stroke::new(1.,egui::Color32::from_rgba_unmultiplied(255,255,255,200)));
 }}
 hovered_snap
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

#[cfg(test)]mod collision_cache_tests {
 use super::*;
 #[test]fn warm_camera_queries_reuse_index_and_history_revision_rebuilds(){
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  let ids=[DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(1.,1.,0.),DVec3::new(-1.,1.,0.)].map(|p|app.scene.mesh.add_vertex(p));
  app.scene.mesh.add_face(ids.to_vec(),vec![]);
  app.navigation.prepare_collision(&app.scene,app.model_revision);
  assert_eq!(app.navigation.collision.as_ref().unwrap().triangle_count(),2);
  let key=app.navigation.collision_key;
  for i in 0..1000{
   app.camera.turn(0.01,0.);app.navigation.prepare_collision(&app.scene,app.model_revision);
   assert_eq!(app.navigation.collision_key,key);
   assert_eq!(app.navigation.distance(&app.scene,&Ray::new(DVec3::new((i%10)as f64/10.,0.,2.),-DVec3::Z)),Some(2.));
  }
  app.scene.mesh.clear();app.model_revision+=1;
  app.navigation.prepare_collision(&app.scene,app.model_revision);
  assert_ne!(app.navigation.collision_key,key);
  assert_eq!(app.navigation.collision.as_ref().unwrap().triangle_count(),0);
  assert_eq!(app.navigation.distance(&app.scene,&Ray::new(DVec3::Z,-DVec3::Z)),None);
 }
}

#[cfg(test)]mod first_person_input_tests {
 use super::*;
 fn frame(app:&mut crate::IngeTrazoApp,ctx:&egui::Context,time:f64,focused:bool,events:Vec<egui::Event>){
  let raw=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))),time:Some(time),focused,events,..Default::default()};
  let _=ctx.run(raw,|ctx|{egui::CentralPanel::default().show(ctx,|ui|{
   let rect=ui.available_rect_before_wrap();ctx.data_mut(|d|d.insert_temp(egui::Id::new("navigation-test-rect"),rect));let response=ui.allocate_rect(rect,egui::Sense::click_and_drag());input(app,ui,rect,&response);
  });});
 }
 fn key(key:egui::Key,pressed:bool,modifiers:egui::Modifiers)->egui::Event{egui::Event::Key{key,physical_key:None,pressed,repeat:false,modifiers}}
 #[test]fn movement_releases_on_key_up_and_focus_loss_without_geometry_history(){
  let ctx=egui::Context::default();let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;app.activate_navigation(Mode::FirstPerson);
  app.camera.look_from(DVec3::Z*1.68,DVec3::Y);let version=app.scene.version;let history=app.undo_stack.len();
  frame(&mut app,&ctx,0.,true,vec![]);
  frame(&mut app,&ctx,0.016,true,vec![key(egui::Key::W,true,egui::Modifiers::NONE)]);
  frame(&mut app,&ctx,0.032,true,vec![]);assert!(app.camera.eye().y>0.);
  frame(&mut app,&ctx,0.048,true,vec![key(egui::Key::W,false,egui::Modifiers::NONE)]);
  let released=app.camera.clone();frame(&mut app,&ctx,0.064,true,vec![]);assert_eq!(app.camera,released);
  frame(&mut app,&ctx,0.080,true,vec![key(egui::Key::E,true,egui::Modifiers::NONE)]);
  assert!(app.camera.eye().z>1.68);
  frame(&mut app,&ctx,0.096,false,vec![]);let unfocused=app.camera.clone();
  frame(&mut app,&ctx,0.112,true,vec![]);assert_eq!(app.camera,unfocused);assert_eq!(app.navigation.axes(),[0,0,0]);
  assert_eq!(app.scene.version,version);assert_eq!(app.undo_stack.len(),history);
 }
 #[test]fn ctrl_shortcuts_are_not_claimed_and_release_clears_pressed_axis(){
  let mut n=Navigation::default();n.activate(Mode::FirstPerson);
  assert!(n.claims_key(egui::Key::S,egui::Modifiers::NONE));assert!(!n.claims_key(egui::Key::S,egui::Modifiers::CTRL));
  n.key_event(egui::Key::S,true,egui::Modifiers::CTRL);assert_eq!(n.axes(),[0,0,0]);
  n.key_event(egui::Key::Q,true,egui::Modifiers::NONE);assert_eq!(n.axes(),[0,0,-1]);
  n.key_event(egui::Key::Q,false,egui::Modifiers::CTRL);assert_eq!(n.axes(),[0,0,0]);
  n.key_event(egui::Key::W,true,egui::Modifiers::NONE);n.cancel();assert_eq!(n.axes(),[0,0,0]);assert!(!n.mouse_captured);
 }
 #[test]fn model_context_change_stops_held_movement_and_mouse_capture(){
  let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;app.activate_navigation(Mode::FirstPerson);
  app.navigation.key_event(egui::Key::W,true,egui::Modifiers::NONE);app.navigation.mouse_captured=true;
  app.bump_model_revision();assert_eq!(app.navigation.axes(),[0,0,0]);assert!(!app.navigation.mouse_captured);
 }

 #[test]fn cursor_ownership_commands_only_on_capture_and_release_transitions(){
  let ctx=egui::Context::default();let mut navigation=Navigation::default();navigation.activate(Mode::FirstPerson);
  let mut run=|capture:bool|{navigation.mouse_captured=capture;let out=ctx.run(egui::RawInput::default(),|ctx|navigation.sync_cursor(ctx));
   out.viewport_output[&egui::ViewportId::ROOT].commands.iter().filter(|command|matches!(command,egui::ViewportCommand::CursorGrab(_)|egui::ViewportCommand::CursorVisible(_))).count()};
  assert_eq!(run(false),0);assert_eq!(run(true),2);assert_eq!(run(true),0);assert_eq!(run(false),2);assert_eq!(run(false),0);
 }

 #[test]fn sheet_and_dialog_ownership_stop_navigation_before_dispatch(){
  let ctx=egui::Context::default();let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;app.activate_navigation(Mode::FirstPerson);
  app.camera.look_from(DVec3::Z*1.68,DVec3::Y);
  for sheet in [true,false]{
   app.navigation.key_event(egui::Key::W,true,egui::Modifiers::NONE);app.navigation.mouse_captured=true;
   app.scene.active_sheet_index=if sheet{Some(0)}else{None};app.show_preferences_dialog=!sheet;
   let before=app.camera.clone();frame(&mut app,&ctx,0.,true,vec![]);
   assert_eq!(app.camera,before);assert_eq!(app.navigation.axes(),[0,0,0]);assert!(!app.navigation.mouse_captured);
   assert!(!app.navigation_claims_key(egui::Key::W,egui::Modifiers::NONE));
  }
 }

 #[test]fn position_click_snaps_endpoint_and_hands_off_to_look(){
  let ctx=egui::Context::default();let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;
  let endpoint=DVec3::new(1.,1.,0.);let a=app.scene.mesh.add_vertex(endpoint);let b=app.scene.mesh.add_vertex(DVec3::new(3.,1.,0.));app.scene.mesh.add_edge(a,b);
  app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);app.camera.target=DVec3::ZERO;app.camera.distance=10.;app.activate_navigation(Mode::Position);
  frame(&mut app,&ctx,0.,true,vec![]);let rect=ctx.data(|d|d.get_temp::<egui::Rect>(egui::Id::new("navigation-test-rect"))).unwrap();
  let pixel=screen_pick::PixelViewport::new(&app.camera,rect.width()as f64,rect.height()as f64).project_for_snapping(endpoint).unwrap();
  let p=egui::pos2(rect.min.x+pixel.x as f32+3.,rect.min.y+pixel.y as f32);
  frame(&mut app,&ctx,0.016,true,vec![egui::Event::PointerMoved(p),egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed:true,modifiers:egui::Modifiers::NONE}]);
  frame(&mut app,&ctx,0.032,true,vec![egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed:false,modifiers:egui::Modifiers::NONE}]);
  assert_eq!(app.navigation.mode,Some(Mode::Look));assert_eq!(app.status_message,Mode::Look.hint());assert_eq!(app.navigation_measurement().as_deref(),Some("1.68 m"));assert!(app.camera.eye().distance(endpoint+DVec3::Z*1.68)<1e-10);assert!(app.camera.perspective);assert!(app.undo_stack.is_empty());
 }

}

/// Frozen Scene.bounds policy for the first-point standard-view plane. Loose
/// orphan vertices are excluded; visible placements include their full mesh.
/// Source edge.hidden is a draw flag, not Scene.entity_visible's bounds flag.
pub(crate) fn first_point_bounds_center(scene:&ingetrazo_core::scene::Scene)->DVec3{
 let mut lo=DVec3::splat(f64::INFINITY);let mut hi=DVec3::splat(f64::NEG_INFINITY);
 fn absorb(p:DVec3,lo:&mut DVec3,hi:&mut DVec3){if p.is_finite(){*lo=lo.min(p);*hi=hi.max(p);}}
 for edge in &scene.mesh.edges{if scene.is_layer_visible(edge.layer.as_deref()){for id in [edge.v0,edge.v1]{if let Some(v)=scene.mesh.vertices.get(id){absorb(v.position,&mut lo,&mut hi);}}}}
 for (id,face) in scene.mesh.faces.iter().enumerate(){if scene.mesh_face_visible(&scene.mesh,id){for &id in &face.vertices{if let Some(v)=scene.mesh.vertices.get(id){absorb(v.position,&mut lo,&mut hi);}}}}
 fn placements(scene:&ingetrazo_core::scene::Scene,group:&ingetrazo_core::groups::Group,parent:glam::DMat4,lo:&mut DVec3,hi:&mut DVec3){
  let world=parent*group.transform;
  if !group.hidden&&!group.billboard&&scene.is_layer_visible(group.layer.as_deref()){for v in &group.mesh.vertices{absorb(world.transform_point3(v.position),lo,hi);}}
  // iter_placements traverses every child; Scene.placements filters each yielded
  // placement, rather than dropping a hidden nested placement's descendants.
  for child in &group.children{placements(scene,child,world,lo,hi);}
 }
 for group in &scene.groups{if !group.hidden&&!group.billboard&&scene.is_layer_visible(group.layer.as_deref()){placements(scene,group,glam::DMat4::IDENTITY,&mut lo,&mut hi);}}
 if lo.is_finite()&&hi.is_finite(){(lo+hi)*0.5}else{DVec3::ZERO}
}

#[cfg(test)]mod bounds_cache_tests{
 use super::*;
 #[test]fn model_visibility_rebuilds_position_center_but_unchanged_hover_reuses_it(){let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;let a=app.scene.mesh.add_vertex(DVec3::new(5.,4.,0.));let b=app.scene.mesh.add_vertex(DVec3::new(6.,4.,1.));let edge=app.scene.mesh.add_edge(a,b).unwrap();app.scene.mesh.edges[edge].layer=Some("visible".into());let layer=ingetrazo_core::layers::Layer::new("visible");app.scene.layers.push(layer);app.scene.mesh.add_vertex(DVec3::splat(1000.));app.navigation.prepare_collision(&app.scene,app.model_revision);assert_eq!(app.navigation.bounds_center,DVec3::new(5.5,4.,0.5));let key=app.navigation.collision_key;app.navigation.prepare_collision(&app.scene,app.model_revision);assert_eq!(app.navigation.collision_key,key);assert!(app.set_layer_visibility(app.scene.layers.len()-1,false));app.navigation.prepare_collision(&app.scene,app.model_revision);assert_eq!(app.navigation.bounds_center,DVec3::ZERO);assert_ne!(app.navigation.collision_key,key);}
}

#[cfg(test)]mod measurement_display_tests{
 use super::*;
 fn visible_text(shape:&egui::Shape,expected:&str)->bool{match shape{egui::Shape::Text(text)=>text.galley.job.text==expected,egui::Shape::Vec(shapes)=>shapes.iter().any(|shape|visible_text(shape,expected)),_=>false}}
 #[test]fn actual_statusbar_submits_navigation_readout_without_mutating_input_or_history(){let mut app=crate::IngeTrazoApp::new();app.system_clipboard=None;app.show_tray=false;app.show_toolbar=false;app.activate_navigation(Mode::Position);let ctx=egui::Context::default();let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.,700.))),..Default::default()},|ctx|app.update_ui(ctx));assert!(output.shapes.iter().any(|shape|visible_text(&shape.shape,"1.68 m")));assert!(app.vcb_text.is_empty());assert!(app.undo_stack.is_empty());app.navigation.mode=Some(Mode::Look);app.camera.look_from(DVec3::new(0.,0.,3.25),DVec3::Y);let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.,700.))),..Default::default()},|ctx|app.update_ui(ctx));assert!(output.shapes.iter().any(|shape|visible_text(&shape.shape,"3.25 m")));assert!(app.vcb_text.is_empty());app.vcb_text="275cm".into();let output=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.,700.))),..Default::default()},|ctx|app.update_ui(ctx));assert!(output.shapes.iter().any(|shape|visible_text(&shape.shape,"275cm")));assert_eq!(app.vcb_text,"275cm");assert!(app.undo_stack.is_empty());}
}
