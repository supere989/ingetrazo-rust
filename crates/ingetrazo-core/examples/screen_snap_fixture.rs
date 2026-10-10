// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON protocol for source/native screen inference comparisons.
use glam::{DMat4,DVec2,DVec3};
use ingetrazo_core::{scene::Scene,snap::{SnapEngine,SnapKind,SnapResult},snap_screen::ScreenSnapContext,screen_pick::PixelViewport,math::Ray};
use std::io::Read;
fn point(v:&serde_json::Value)->DVec3 { DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap()) }
fn main(){let mut input=String::new();std::io::stdin().read_to_string(&mut input).unwrap();let cases:serde_json::Value=serde_json::from_str(&input).unwrap();let mut out=vec![];
for case in cases.as_array().unwrap(){let mut scene=Scene::new();for e in case["edges"].as_array().unwrap(){let a=scene.mesh.add_vertex(point(&e[0]));let b=scene.mesh.add_vertex(point(&e[1]));scene.mesh.add_edge(a,b);}
if case.get("nested").and_then(|v|v.as_bool())==Some(true){let mesh=std::mem::take(&mut scene.mesh);let mut child=ingetrazo_core::groups::Group::with_mesh("child",mesh);child.transform=DMat4::from_translation(DVec3::new(3.,0.,0.));let mut parent=ingetrazo_core::groups::Group::new("parent");parent.transform=DMat4::from_translation(DVec3::new(0.,4.,0.));parent.children.push(child);scene.groups.push(parent);}
if case.get("occluded").and_then(|v|v.as_bool())==Some(true){let ids=[DVec3::new(2.,3.,0.5),DVec3::new(4.,3.,0.5),DVec3::new(4.,5.,0.5),DVec3::new(2.,5.,0.5)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);}
let candidate=point(&case["candidate"]);let start=case.get("start").map(point);let mut engine=SnapEngine::default();
if case.get("shift").and_then(|v|v.as_bool())==Some(true){let s=start.unwrap();engine.note_inference_result(&SnapResult{point:s+DVec3::X,kind:SnapKind::AxisInference,axis:Some('x'),..Default::default()});engine.update_shift_state(true,start);}
if let Some(axis)=case.get("axis").and_then(|v|v.as_str()){engine.locked_axis=axis.chars().next();}
let viewport=PixelViewport{matrix:DMat4::from_translation(DVec3::new(0.,0.,0.5))*DMat4::from_scale(DVec3::new(0.1,0.1,-0.001)),width:2000.,height:2000.};let cursor=viewport.project(candidate).unwrap();let ctx=ScreenSnapContext{viewport,cursor:DVec2::new(cursor.x,cursor.y),candidate,plane_normal:DVec3::Z};let result=engine.snap_screen(&Ray::new(candidate+DVec3::Z*100.,-DVec3::Z),&scene,start,None,&ctx);
let kind=match result.kind{SnapKind::AxisLock=>"axis",SnapKind::AxisInference=>"axis_inference",SnapKind::Endpoint=>"endpoint",SnapKind::Midpoint=>"midpoint",SnapKind::Intersection=>"intersection",SnapKind::Reference=>"reference",SnapKind::FromPoint=>"from_point",SnapKind::OnEdge=>"on_edge",SnapKind::Origin=>"origin",SnapKind::None=>"none",SnapKind::OnFace=>"on_face",_=>"other"};out.push(serde_json::json!({"name":case["name"],"kind":kind,"point":result.point.to_array()}));}println!("{}",serde_json::to_string(&out).unwrap());}
