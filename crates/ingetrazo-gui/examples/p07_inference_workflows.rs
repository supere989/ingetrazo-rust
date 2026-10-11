// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual production inference and App typed authoring, frozen source inputs.
use glam::{DMat4,DVec2,DVec3};
use ingetrazo_core::{scene::Scene,guides::{Guide,GuideType},math::Ray,screen_pick::PixelViewport,snap::{SnapEngine,SnapKind},snap_screen::{ScreenSnapContext,guide_snap_segment},units::vcb};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::base::{ToolType,ToolContext};
use serde_json::{json,Value};
fn p(v:&Value)->DVec3{DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
fn viewport(r:&Value)->PixelViewport{let a:Vec<f64>=r["matrix"].as_array().unwrap().iter().map(|x|x.as_f64().unwrap()).collect();PixelViewport{matrix:DMat4::from_cols_slice(&a),width:1000.,height:800.}}
fn fill(scene:&mut Scene,r:&Value){
 for g in r["guides"].as_array().unwrap(){let point=p(&g["point"]);let mut owner=Guide::point(point);if !g["direction"].is_null(){owner.kind=GuideType::InfiniteLine{point,dir:p(&g["direction"])};}scene.guides.push(owner);}
 for e in r["edges"].as_array().unwrap(){let a=scene.mesh.add_vertex(p(&e[0]));let b=scene.mesh.add_vertex(p(&e[1]));scene.mesh.add_edge(a,b);}
 if r["occluded"]==true {let c=p(&r["center"])+p(&r["normal"])*0.5;let u=p(&r["u"]);let v=p(&r["v"]);let ids=[c-u-v,c+u-v,c+u+v,c-u+v].map(|x|scene.mesh.add_vertex(x));scene.mesh.add_face(ids.to_vec(),vec![]);}
}
fn snap(engine:&mut SnapEngine,scene:&Scene,r:&Value)->ingetrazo_core::snap::SnapResult{
 let vp=viewport(r);let candidate=p(&r["candidate"]);let cursor=DVec2::new(r["cursor"][0].as_f64().unwrap(),r["cursor"][1].as_f64().unwrap());let ctx=ScreenSnapContext{viewport:vp,cursor,candidate,plane_normal:p(&r["normal"])};
 engine.snap_screen(&vp.ray(cursor).unwrap(),scene,None,None,&ctx)
}
fn kind(k:SnapKind)->&'static str{match k{SnapKind::Intersection=>"intersection",SnapKind::Endpoint=>"endpoint",SnapKind::OnEdge=>"on_edge",SnapKind::Guide=>"on_line",SnapKind::Origin=>"origin",SnapKind::Midpoint=>"midpoint",SnapKind::None=>"none",_=>"other"}}
fn coord(p:DVec3)->Vec<f64>{p.to_array().into_iter().map(|x|(x*1e5).round()/1e5).collect()}
fn ring(mut xs:Vec<Vec<f64>>)->Vec<Vec<f64>>{if xs.is_empty(){return xs;}let base=xs.clone();for i in 1..base.len(){let mut next=base[i..].to_vec();next.extend_from_slice(&base[..i]);if next<xs{xs=next;}}xs}
fn state(a:&IngeTrazoApp)->Value{let m=&a.scene.mesh;let mut vs:Vec<_>=m.vertices.iter().map(|v|coord(v.position)).collect();vs.sort_by(|a,b|a.partial_cmp(b).unwrap());let mut es:Vec<_>=m.edges.iter().map(|e|{let mut x=vec![coord(m.vertices[e.v0].position),coord(m.vertices[e.v1].position)];x.sort_by(|a,b|a.partial_cmp(b).unwrap());x}).collect();es.sort_by(|a,b|a.partial_cmp(b).unwrap());let mut fs:Vec<_>=m.faces.iter().map(|f|ring(f.vertices.iter().map(|&i|coord(m.vertices[i].position)).collect())).collect();fs.sort_by(|a,b|a.partial_cmp(b).unwrap());json!({"vertices":vs,"edges":es,"faces":fs,"history":[a.undo_stack.len(),a.redo_stack.len()]})}
fn egui_ingress()->Value{
 fn frame(ctx:&egui::Context,a:&mut IngeTrazoApp,time:f64,events:Vec<egui::Event>){let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(1000.,800.))),time:Some(time),events,..Default::default()},|ctx|a.update_ui(ctx));}
 let mut a=IngeTrazoApp::new();a.system_clipboard=None;a.scene=Scene::new();a.camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);a.camera.perspective=false;a.camera.target=DVec3::ZERO;a.camera.distance=12.;a.native_gpu_viewport_enabled=true;a.set_tool(ToolType::Line);let point=DVec3::new(1.,1.,0.);a.scene.guides.push(Guide::point(point));let ctx=egui::Context::default();frame(&ctx,&mut a,0.,vec![]);frame(&ctx,&mut a,0.1,vec![]);let rect=a.native_gpu_viewport_rect.unwrap();let vp=PixelViewport{matrix:ingetrazo_core::snap_screen::source_camera_matrix(&a.camera).as_dmat4(),width:rect.width()as f64,height:rect.height()as f64};let pixel=vp.project_for_snapping(point).unwrap();let mouse=rect.min+egui::vec2(pixel.x as f32+8.9,pixel.y as f32);let button=|pressed|egui::Event::PointerButton{pos:mouse,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()};frame(&ctx,&mut a,0.2,vec![egui::Event::PointerMoved(mouse)]);frame(&ctx,&mut a,0.3,vec![button(true)]);frame(&ctx,&mut a,0.4,vec![button(false)]);let anchor=a.active_tool.snap_anchor();let hover=rect.min+egui::vec2(pixel.x as f32+60.,pixel.y as f32);frame(&ctx,&mut a,0.5,vec![egui::Event::PointerMoved(hover)]);ctx.memory_mut(|m|m.request_focus(egui::Id::new("measurements_vcb")));frame(&ctx,&mut a,0.6,vec![egui::Event::Text("٠".into()),egui::Event::Text(",".into()),egui::Event::Text("٥".into())]);let buffered=a.vcb_text.clone();frame(&ctx,&mut a,0.7,vec![egui::Event::Key{key:egui::Key::Enter,physical_key:None,pressed:true,repeat:false,modifiers:Default::default()}]);let after=state(&a);a.dispatch_tool_cancel();a.undo();let undo=state(&a);a.redo();let redo=state(&a);json!({"anchor":anchor.map(|p|p.to_array()),"expected_anchor":point.to_array(),"buffered":buffered,"after":after,"undo":undo,"redo":redo,"status":a.status_message})
}
fn main(){let args:Vec<_>=std::env::args().collect();let source:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();let appsource:Value=serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();let mut rows=vec![];
 for r in source["cases"].as_array().unwrap(){let mut scene=Scene::new();fill(&mut scene,r);let hit=snap(&mut SnapEngine::default(),&scene,r);let mut segments=vec![];for g in &scene.guides{match g.kind{GuideType::Point(x)=>segments.push([x.to_array(),x.to_array()]),GuideType::InfiniteLine{point,dir}=>{if let Some((a,b))=guide_snap_segment(&viewport(r),point,dir){segments.push([a.to_array(),b.to_array()]);}},_=>{}}}rows.push(json!({"name":r["name"],"kind":kind(hit.kind),"point":hit.point.to_array(),"segments":segments}));}
 let numeric:Vec<_>=source["numeric"].as_array().unwrap().iter().map(|r|{let value=if r["mode"]=="angle"{vcb::parse_angle(r["text"].as_str().unwrap()).ok().map(|x|vec![x])}else{vcb::parse_values(r["text"].as_str().unwrap(),1.,r["comma_lists"].as_bool().unwrap()).ok()};json!({"text":r["text"],"mode":r["mode"],"comma_lists":r["comma_lists"],"value":value})}).collect();
 let mut app_rows=vec![];
 for row in appsource["cases"].as_array().unwrap(){let r=&row["recipe"];let mut a=IngeTrazoApp::new();a.system_clipboard=None;a.scene=Scene::new();let tool=match r["tool"].as_str().unwrap(){"rectangle"=>ToolType::Rectangle,"circle"=>ToolType::Circle,"arc"=>ToolType::Arc,_=>ToolType::Line};a.set_tool(tool);a.active_tool.note_cursor_plane(DVec3::ZERO,p(&r["normal"]));let mut states=vec![json!({"phase":"before","state":state(&a)})];let mut accepted=None;
 if r["count_first"]==true{accepted=Some(a.dispatch_tool_vcb(r["text"].as_str().unwrap()).is_ok());}
 let point=p(&r["points"][0]);let mut c=ToolContext::new(point,Ray::new(point+p(&r["normal"])*10.,-p(&r["normal"])));
 if let Some(name)=r["guide_case"].as_str(){let guide=source["cases"].as_array().unwrap().iter().find(|x|x["name"]==name).unwrap();fill(&mut a.scene,guide);let s=snap(&mut a.snap_engine,&a.scene,guide);c.world=s.point;}
 a.dispatch_tool_click(&c);states.push(json!({"phase":"press","state":state(&a)}));let point=p(&r["points"][1]);let c=ToolContext::new(point,Ray::new(point+p(&r["normal"])*10.,-p(&r["normal"])));
 if r["two_clicks"]==true{a.dispatch_tool_click(&c);}else{a.dispatch_tool_move(&c);}states.push(json!({"phase":"hover","state":state(&a)}));
 if r["count_first"]==true{a.dispatch_tool_click(&c);}else{accepted=Some(a.dispatch_tool_vcb(r["text"].as_str().unwrap()).is_ok());}
 states.push(json!({"phase":"commit-or-refuse","state":state(&a)}));a.dispatch_tool_cancel();states.push(json!({"phase":"cancel","state":state(&a)}));a.undo();states.push(json!({"phase":"undo","state":state(&a)}));a.redo();states.push(json!({"phase":"redo","state":state(&a)}));app_rows.push(json!({"name":r["name"],"accepted":accepted,"states":states}));}
 // The source ASCII fast path must not allocate. Unicode decimal authoring
 // does not alter native document f64 coordinates or current active input.
 assert!(matches!(vcb::normalize_source_numeric("1.25m"),std::borrow::Cow::Borrowed(_)));
 let double_view=PixelViewport{matrix:DMat4::from_scale(DVec3::new(0.1,0.1,0.1)),width:1000.,height:800.};let (ga,gb)=guide_snap_segment(&double_view,DVec3::ZERO,DVec3::X).unwrap();assert_eq!(ga.x,-15.);assert_eq!(gb.x,15.);
 let native_double=DVec3::new(1e12+0.125,1e12+0.25,1e12+0.5);let mut m=Scene::new();m.mesh.add_vertex(native_double);let before=m.mesh.vertices[0].position;let _=snap(&mut SnapEngine::default(),&m,&source["cases"][0]);assert_eq!(m.mesh.vertices[0].position,before);
 std::fs::write(&args[3],serde_json::to_vec_pretty(&json!({"cases":rows,"numeric":numeric,"app":app_rows,"ascii_borrowed":true,"native_f64_preserved":true,"double_guide_matrix_preserved":true,"egui":egui_ingress()})).unwrap()).unwrap();
}
