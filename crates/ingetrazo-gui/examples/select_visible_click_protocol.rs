use glam::{DVec2,DVec3,DMat4};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_core::{scene::SelectionItem,groups::Group,screen_pick::PixelViewport};
use ingetrazo_tools::base::{ToolContext,ToolType};
use serde_json::{Value,json};
fn face(app:&mut IngeTrazoApp,points:&[DVec3])->usize {let ids=points.iter().map(|p|app.scene.mesh.add_vertex(*p)).collect();app.scene.mesh.add_face(ids,vec![]).unwrap()}
fn setup(kind:usize)->IngeTrazoApp {
 let mut app=IngeTrazoApp::new();app.set_tool(ToolType::Select);
 if kind==0 {let p=[DVec3::new(0.,0.,1.),DVec3::new(2.,0.,1.),DVec3::new(2.,2.,1.),DVec3::new(0.,2.,1.)];face(&mut app,&[p[0],p[1],p[2]]);face(&mut app,&[p[0],p[2],p[3]]);let a=app.scene.mesh.add_vertex(p[0]);let c=app.scene.mesh.add_vertex(p[2]);let seam=app.scene.mesh.find_edge(a,c).unwrap();app.scene.mesh.edges[seam].soft=true;let a=app.scene.mesh.add_vertex(DVec3::new(-1.,1.,0.));let b=app.scene.mesh.add_vertex(DVec3::new(3.,1.,0.));app.scene.mesh.add_edge(a,b).unwrap();}
 else if kind==1 {let mut g=Group::new("caja");let ids=[DVec3::ZERO,DVec3::new(2.,0.,0.),DVec3::new(2.,2.,0.),DVec3::new(0.,2.,0.)].map(|p|g.mesh.add_vertex(p));g.mesh.add_face(ids.to_vec(),vec![]).unwrap();g.transform=DMat4::from_translation(DVec3::new(1.,1.,0.));g.hidden=true;app.scene.groups.push(g);}
 else {let id=face(&mut app,&[DVec3::ZERO,DVec3::new(2.,0.,0.),DVec3::new(2.,2.,0.),DVec3::new(0.,2.,0.)]);app.scene.mesh.faces[id].attrs.insert("hidden".into(),json!(true));}
 app.scene.bump_version();app
}
fn ui_frame(ctx:&egui::Context,app:&mut IngeTrazoApp,time:f64,events:Vec<egui::Event>) {
 let input=egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.,700.))),time:Some(time),events,..Default::default()};
 let _=ctx.run(input,|ctx|app.update_ui(ctx));
}
fn ui_click(kind:usize,point:DVec3,shown:bool)->Value {
 let mut app=setup(kind);app.native_gpu_viewport_enabled=true;app.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);app.camera.target=DVec3::new(3.,3.,0.);app.camera.distance=12.;app.camera.perspective=false;
 if kind==0{app.scene.set_hidden_view(false,shown);}if kind==1{app.scene.set_hidden_view(true,shown);}if kind==2{app.scene.set_hidden_view(false,shown);}
 let ctx=egui::Context::default();ui_frame(&ctx,&mut app,0.,vec![]);ui_frame(&ctx,&mut app,0.1,vec![]);
 let rect=app.native_gpu_viewport_rect.unwrap();let v=PixelViewport::new(&app.camera,rect.width() as f64,rect.height() as f64);let p=v.project_for_snapping(point).unwrap();let pos=egui::pos2(rect.min.x+p.x as f32,rect.min.y+p.y as f32);
 ui_frame(&ctx,&mut app,0.2,vec![egui::Event::PointerMoved(pos),egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}]);
 ui_frame(&ctx,&mut app,0.3,vec![egui::Event::PointerButton{pos,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}]);
 let mut selection=app.scene.selection.iter().map(|i|match i{SelectionItem::Edge(id)=>json!(["edge",id]),SelectionItem::Face(id)=>json!(["face",id]),SelectionItem::Group(id)=>json!(["group",id]),_=>panic!("Unexpected selection")}).collect::<Vec<_>>();selection.sort_by_key(|v|v.to_string());
 json!({"kind":kind,"shown":shown,"selection":selection,"viewport":[rect.min.x,rect.min.y,rect.max.x,rect.max.y]})
}
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args=std::env::args().collect::<Vec<_>>();let source:Value=serde_json::from_slice(&std::fs::read(&args[1])?)?;
 let mut app=setup(0);let mut rows=vec![];
 for row in source["rows"].as_array().unwrap(){let name=row["name"].as_str().unwrap();if matches!(name,"hidden-object-off"|"object-before-toggle"|"undo-object-before"){app=setup(1);}if name=="hidden-face-off"{app=setup(2);}
  if matches!(name,"double-soft"|"locked-occluder"){app=setup(0);}
  if name=="locked-occluder"{let mut layer=ingetrazo_core::layers::Layer::new("solid");layer.locked=true;app.scene.layers.push(layer);for face in &mut app.scene.mesh.faces{face.layer=Some("solid".into());}app.scene.bump_version();}
  if name=="excluded-occluder"{app.scene.layers.last_mut().unwrap().visible=false;app.scene.bump_version();}
  if name=="undo-object-before"{app.scene.groups[0].hidden=false;app.scene.bump_version();}
  if name=="xray-below"{app=setup(0);app.scene.display_style.face_mode=ingetrazo_core::presentation::FaceMode::Xray;}
  if name=="wireframe-below"{app.scene.display_style.face_mode=ingetrazo_core::presentation::FaceMode::Wireframe;}
  if name=="hidden-face-locked"{app.scene.layers[0].locked=true;app.scene.bump_version();}
  if name=="hidden-face-layer-off"{app.scene.layers[0].locked=false;app.scene.layers[0].visible=false;app.scene.bump_version();}
  if name=="hidden-face-layer-restored"{app.scene.layers[0].visible=true;app.scene.bump_version();}
  app.scene.set_hidden_view(true,row["objects"].as_bool().unwrap());app.scene.set_hidden_view(false,row["geometry"].as_bool().unwrap());
  let m:Vec<f64>=serde_json::from_value(row["matrix"].clone())?;let viewport=PixelViewport{matrix:DMat4::from_cols_array(m.as_slice().try_into()?),width:1000.,height:600.};let eye=DVec3::from_array(serde_json::from_value(row["eye"].clone())?);let cursor=DVec2::from_array(serde_json::from_value(row["pixel"].clone())?);let point=DVec3::from_array(serde_json::from_value(row["point"].clone())?);
  app.active_tool.note_pick_viewport(viewport,eye);let mut ctx=ToolContext::new(point,viewport.ray(cursor).unwrap());ctx.screen=Some(cursor);ctx.shift_held=matches!(name,"shift-remove"|"both-remove");ctx.ctrl_held=matches!(name,"ctrl-add"|"both-remove");match row["action"].as_str().unwrap(){"click"=>app.dispatch_tool_click(&ctx),"double"=>app.dispatch_tool_double_click(&ctx),"state"=>(),"hide"=>app.hide_selection(),"undo"=>app.undo(),_=>panic!("unknown action")}
  let mut selection=app.scene.selection.iter().map(|i|match i{SelectionItem::Edge(id)=>json!(["edge",id]),SelectionItem::Face(id)=>json!(["face",id]),SelectionItem::Group(id)=>json!(["group",id]),_=>panic!("Unexpected selection")}).collect::<Vec<_>>();selection.sort_by_key(|v|v.to_string());
  rows.push(json!({"name":name,"edit":!app.component_edit_stack.is_empty(),"group_hidden":if app.component_edit_stack.is_empty(){json!(app.scene.groups.iter().map(|g|g.hidden).collect::<Vec<_>>())}else{Value::Null},"active_vertices":app.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"selection":selection,"objects":app.scene.show_hidden_objects,"geometry":app.scene.show_hidden_geometry}));
 }
 let ui=vec![ui_click(0,DVec3::new(1.2,1.,1.),false),ui_click(0,DVec3::new(1.,1.,1.),true),ui_click(0,DVec3::new(1.6,1.,0.),false),ui_click(1,DVec3::new(2.,2.,0.),false),ui_click(1,DVec3::new(2.,2.,0.),true),ui_click(2,DVec3::new(1.,1.,0.),false),ui_click(2,DVec3::new(1.,1.,0.),true)];
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"rows":rows,"ui_events":ui}))?)?;Ok(())
}
