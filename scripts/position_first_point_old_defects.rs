use ingetrazo_gui::{IngeTrazoApp,navigation::{self,Mode}};
use ingetrazo_core::{camera::StandardView,image_plane::ImagePlane};
use glam::DVec3;
fn frame(app:&mut IngeTrazoApp,ctx:&egui::Context,time:f64,events:Vec<egui::Event>)->Option<ingetrazo_core::snap::SnapResult>{
 let snap=None;let raw=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))),time:Some(time),focused:true,events,..Default::default()};
 let _=ctx.run(raw,|ctx|{egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{let rect=ui.available_rect_before_wrap();let response=ui.allocate_rect(rect,egui::Sense::click_and_drag());navigation::input(app,ui,rect,&response);});});snap
}
fn button(p:egui::Pos2,pressed:bool)->egui::Event{egui::Event::PointerButton{pos:p,button:egui::PointerButton::Primary,pressed,modifiers:egui::Modifiers::NONE}}
fn main(){
 let mut rows=vec![];
 for(name,view,kind,drag,cancel)in[
 ("front-empty",StandardView::Front,"empty",None,false),("right-empty",StandardView::Right,"empty",None,false),
 ("top-empty",StandardView::Top,"empty",None,false),("iso-ground",StandardView::Iso,"empty",None,false),
 ("front-locked-image",StandardView::Front,"image",None,false),("face-before-image",StandardView::Front,"face-image",None,false),
 ("position-drag",StandardView::Top,"empty",Some(egui::pos2(470.,350.)),false),("position-cancel",StandardView::Top,"empty",Some(egui::pos2(470.,350.)),true),
 ("position-six-pixels",StandardView::Top,"empty",Some(egui::pos2(436.,330.)),false),
 ("loose-orphan",StandardView::Front,"orphan",None,false),("visible-edge-bounds",StandardView::Front,"visible-edge",None,false),
 ("draw-hidden-edge-bounds",StandardView::Front,"draw-hidden-edge",None,false),("hidden-layer-bounds",StandardView::Front,"hidden-layer",None,false),
 ("hidden-group-bounds",StandardView::Front,"hidden-group",None,false),("group-orphan-bounds",StandardView::Front,"group-orphan",None,false),
 ("hidden-nested-descendant-bounds",StandardView::Front,"hidden-nested",None,false)]{
 let mut app=IngeTrazoApp::new();app.system_clipboard=None;app.camera.aspect=800./600.;app.camera.distance=10.;app.camera.target=DVec3::ZERO;app.camera.perspective=false;app.camera.set_standard_view(view);
 if kind.contains("image"){let mut image=ImagePlane::new("scan",DVec3::new(-4.,2.,-4.),DVec3::X*8.,DVec3::Z*8.);image.locked=true;app.scene.image_planes.push(image);}
 if kind=="face-image"{let ids=[DVec3::new(-4.,1.,-4.),DVec3::new(4.,1.,-4.),DVec3::new(4.,1.,4.),DVec3::new(-4.,1.,4.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]);}
 if kind=="orphan"{app.scene.mesh.add_vertex(DVec3::new(100.,200.,300.));}
 if ["visible-edge","draw-hidden-edge","hidden-layer","hidden-group"].contains(&kind){
  let a=app.scene.mesh.add_vertex(DVec3::new(5.,4.,0.));let b=app.scene.mesh.add_vertex(DVec3::new(6.,4.,1.));let id=app.scene.mesh.add_edge(a,b).unwrap();
  if kind=="draw-hidden-edge"{app.scene.mesh.edges[id].hidden=true;}
  if kind=="hidden-layer"{let mut layer=ingetrazo_core::layers::Layer::new("off");layer.visible=false;app.scene.layers.push(layer);let a=app.scene.mesh.add_vertex(DVec3::new(100.,200.,300.));let b=app.scene.mesh.add_vertex(DVec3::new(101.,201.,301.));let id=app.scene.mesh.add_edge(a,b).unwrap();app.scene.mesh.edges[id].layer=Some("off".into());}
  if kind=="hidden-group"{let mut g=ingetrazo_core::groups::Group::new("hidden");g.hidden=true;g.mesh.add_vertex(DVec3::new(100.,200.,300.));app.scene.groups.push(g);}
 }
 if kind=="group-orphan"{let mut g=ingetrazo_core::groups::Group::new("orphan");g.mesh.add_vertex(DVec3::new(50.,8.,20.));app.scene.groups.push(g);}
 if kind=="hidden-nested"{let mut g=ingetrazo_core::groups::Group::new("root");let mut child=ingetrazo_core::groups::Group::new("hidden");child.hidden=true;child.mesh.add_vertex(DVec3::new(100.,200.,300.));let mut grand=ingetrazo_core::groups::Group::new("visible grandchild");grand.mesh.add_vertex(DVec3::new(50.,12.,20.));child.children.push(grand);g.children.push(child);app.scene.groups.push(g);}
 app.activate_navigation(Mode::Position);let ctx=egui::Context::default();let before=app.camera.eye();frame(&mut app,&ctx,0.,vec![]);let p=egui::pos2(430.,330.);
 let snap=frame(&mut app,&ctx,0.016,vec![egui::Event::PointerMoved(p),button(p,true)]);
 if let Some(end)=drag{frame(&mut app,&ctx,0.032,vec![egui::Event::PointerMoved(end)]);}
 let preview:Vec<Vec<[f64;3]>>=vec![]; // old controller has no preview API
 if cancel{frame(&mut app,&ctx,0.048,vec![egui::Event::Key{key:egui::Key::Escape,physical_key:None,pressed:true,repeat:false,modifiers:egui::Modifiers::NONE}]);}else{frame(&mut app,&ctx,0.048,vec![button(drag.unwrap_or(p),false)]);}
 rows.push(serde_json::json!({"name":name,"point":snap.as_ref().map(|s|s.point.to_array()),"snap":snap.map(|s|format!("{:?}",s.kind)),"preview":preview,"after_eye":app.camera.eye().to_array(),"after_forward":app.camera.forward().to_array(),"before_eye":before.to_array(),"cancelled":cancel,"undo":app.undo_stack.len(),"mode":format!("{:?}",app.navigation.mode)}));
 }
 println!("{}",serde_json::json!({"cases":rows}));
}
