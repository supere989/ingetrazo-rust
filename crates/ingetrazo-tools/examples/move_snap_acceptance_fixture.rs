//! Source-controller protocol: real Move target/exclusion and screen inference.
use glam::{DVec2,DVec3};
use ingetrazo_core::{scene::{Scene,SelectionItem},math::Ray,camera::OrbitCamera,screen_pick::PixelViewport,snap::SnapEngine,snap_screen::{ScreenSnapContext,ScreenSnapPolicy}};
use ingetrazo_tools::{base::{Tool,ToolContext},move_tool::MoveTool};
use std::io::{self,Read};
fn v(x:&serde_json::Value)->DVec3{DVec3::new(x[0].as_f64().unwrap(),x[1].as_f64().unwrap(),x[2].as_f64().unwrap())}
fn main(){
 let mut text=String::new();io::stdin().read_to_string(&mut text).unwrap();let input:serde_json::Value=serde_json::from_str(&text).unwrap();let mut output=vec![];
 for case in input.as_array().unwrap(){
  let mut scene=Scene::new();for edge in case["edges"].as_array().unwrap(){let a=scene.mesh.add_vertex(v(&edge[0]));let b=scene.mesh.add_vertex(v(&edge[1]));scene.mesh.add_edge(a,b);}
  scene.select(SelectionItem::Edge(0));let ray=Ray::new(v(&case["ray"][0]),v(&case["ray"][1]));
  let mut tool=MoveTool::new();let mut click=ToolContext::new(v(&case["start"]),ray);click.ctrl_held=case["copy"].as_bool().unwrap();tool.on_click(&mut scene,&click);
  let mut camera=OrbitCamera::new();camera.target=v(&case["camera"]["target"]);camera.pitch=case["camera"]["pitch"].as_f64().unwrap();camera.yaw=case["camera"]["yaw"].as_f64().unwrap();camera.distance=10.;camera.perspective=true;camera.aspect=1.;
  let ctx=ScreenSnapContext{viewport:PixelViewport::new(&camera,800.,800.),cursor:DVec2::new(case["pixel"][0].as_f64().unwrap(),case["pixel"][1].as_f64().unwrap()),candidate:v(&case["candidate"]),plane_normal:v(&case["normal"])};
  let mut engine=SnapEngine::default();engine.locked_axis=case["lock"].as_str().and_then(|s|s.chars().next());
  let r=engine.snap_screen_with_policy(&ray,&scene,tool.snap_anchor(),None,&ctx,&ScreenSnapPolicy{exclusions:tool.snap_exclusions(),magnetic_axis_deg:tool.magnetic_axis_deg(),screen_axis_px:tool.screen_axis_px()});
  output.push(serde_json::json!({"name":case["name"],"kind":format!("{:?}",r.kind),"point":r.point.to_array(),"axis":r.axis,"excluded":tool.snap_exclusions().map(|s|{let mut v=s.loose_edges.iter().copied().collect::<Vec<_>>();v.sort();v})}));
 }
 println!("{}",serde_json::to_string(&output).unwrap());
}
