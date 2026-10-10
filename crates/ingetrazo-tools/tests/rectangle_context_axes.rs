use glam::{DMat4,DVec3};
use ingetrazo_core::{scene::Scene,math::Ray};
use ingetrazo_tools::{rectangle::RectangleTool,base::{Tool,ToolContext}};
fn vector(value:&serde_json::Value)->DVec3 {DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap())}
#[test]
fn actual_original_reentry_rectangle_controller_frames_and_arrow_locks() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-rectangle-axes-source.json")).unwrap();
 for row in source["cases"].as_array().unwrap() {
  let raw:Vec<_>=row["drawing_frame"].as_array().unwrap().iter().map(|x|x.as_f64().unwrap()).collect();let raw:[f64;16]=raw.try_into().unwrap();let mut scene=Scene::new();scene.drawing_frame=Some(DMat4::from_cols_array(&raw));let before=scene.clone();let mut tool=RectangleTool::new();tool.on_activate(&mut scene);
  if row["lock"]=="world-hover" {tool.note_cursor_plane(DVec3::ZERO,vector(&row["normal"]));} else if row["lock"].is_string() {tool.lock_drawing_plane(vector(&row["normal"]));}
  let start=vector(&row["start"]);let hover=vector(&row["hover"]);let normal=vector(&row["normal"]);let ray=Ray::new(start+normal*10.,-normal);
  tool.on_click(&mut scene,&ToolContext::new(start,ray));tool.on_move(&mut scene,&ToolContext::new(hover,ray));assert_eq!(scene,before,"preview must remain inert");tool.on_vcb_input(&mut scene,"2,3").unwrap();assert_eq!(scene.mesh.faces.len(),1);
  let face=&scene.mesh.faces[0];assert_eq!(face.vertices.len(),4);let expected=row["face_points"].as_array().unwrap();
  let corners:Vec<_>=face.vertices.iter().map(|i|scene.mesh.vertices[*i].position).collect();
  assert!((0..4).any(|shift|(0..4).all(|i|corners[(i+shift)%4].distance(vector(&expected[i]))<4e-6)),"{} {}: {corners:?} vs {expected:?}",row["kind"],row["lock"]);
  let mut lengths=(0..4).map(|i|corners[i].distance(corners[(i+1)%4])).collect::<Vec<_>>();lengths.sort_by(f64::total_cmp);for (a,b) in lengths.iter().zip([2.,2.,3.,3.]){assert!((*a-b).abs()<1e-6);}
  // Reset uses the context plane, and leaving the context refreshes all axes.
  tool.on_cancel(&mut scene);assert!(tool.start_point.is_none());scene.drawing_frame=None;tool.on_activate(&mut scene);assert_eq!(tool.plane_normal,DVec3::Z);
 }
}
#[test]
fn actual_world_hover_plane_wins_while_context_red_controls_rectangle_heading() {
 let mut scene=Scene::new();scene.drawing_frame=Some(DMat4::from_rotation_z(0.4)*DMat4::from_rotation_y(0.3));let mut tool=RectangleTool::new();tool.on_activate(&mut scene);tool.note_cursor_plane(DVec3::ZERO,DVec3::Z);
 let start=DVec3::new(5.,6.,0.);let (u,v)=scene.drawing_axes().plane_axes(DVec3::Z);let ray=Ray::new(start+DVec3::Z,-DVec3::Z);tool.on_click(&mut scene,&ToolContext::new(start,ray));tool.on_move(&mut scene,&ToolContext::new(start+u+v*2.,ray));tool.on_vcb_input(&mut scene,"2,3").unwrap();let p=&scene.mesh.vertices;let f=&scene.mesh.faces[0];assert!(f.vertices.iter().any(|i|(p[*i].position-(start+u*2.)).length()<1e-9));assert!(f.vertices.iter().all(|i|p[*i].position.z.abs()<1e-9));
}
