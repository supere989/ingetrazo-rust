use glam::{DMat4,DVec3};
use ingetrazo_core::{dimensions::Dimension,groups::Group,mesh::Mesh,scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
fn main(){
 let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
 let cases:serde_json::Value=serde_json::from_slice(&std::fs::read(dir.join("world-cases.json")).unwrap()).unwrap();
 let mut rows=vec![];
 for value in cases.as_array().unwrap(){
  let matrix=|key:&str|DMat4::from_cols_array(&serde_json::from_value::<[f64;16]>(value[key].clone()).unwrap());
  let frame=matrix("parent")*matrix("child");
  let vec3=|key:&str|DVec3::from_array(serde_json::from_value::<[f64;3]>(value[key].clone()).unwrap());
  let a=vec3("a");let b=vec3("b");
  let mut mesh=Mesh::new();let va=mesh.add_vertex(a);let vb=mesh.add_vertex(b);mesh.add_edge(va,vb);
  let mut leaf=Group::with_mesh("leaf",mesh);leaf.transform=matrix("child");
  let mut outer=Group::new("outer");outer.transform=matrix("parent");outer.children.push(leaf);
  let mut app=IngeTrazoApp::new();app.scene.groups.push(outer);
  let mut dim=Dimension::new(frame.transform_point3(a),frame.transform_point3(b),frame.transform_vector3(vec3("offset")));
  dim.axis=value["axis"].as_str().map(str::to_owned);
  let before=dim.clone();app.scene.dimensions.push(dim);
  app.scene.select(SelectionItem::Group(0));app.begin_component_edit();
  app.scene.select(SelectionItem::Group(0));app.begin_component_edit();
  // Editing tools work in WORLD coordinates after placement baking. Drawing
  // axes remain a distinct frame and must not transform annotations twice.
  assert_eq!(app.scene.dimension_frame,DMat4::IDENTITY);
  let d=&app.scene.dimensions[0];assert_eq!(d.measurement_frame,DMat4::IDENTITY);
  assert_eq!((d.start,d.end,d.offset),(before.start,before.end,before.offset));
  let(first,last)=d.line_points();
  let(placement,paxis)=Dimension::placement_in_frame(d.start,d.end,frame.transform_point3(vec3("cursor")),app.scene.dimension_frame);
  rows.push(serde_json::json!({"length":d.length(),"line_points":[frame.transform_point3(first).to_array(),frame.transform_point3(last).to_array()],"placement_offset":frame.transform_vector3(placement).to_array(),"placement_axis":paxis,"world_annotations_preserved":true}));
  app.cancel_component_edit();app.cancel_component_edit();
 }
 std::fs::write(dir.join("world-result.json"),serde_json::to_vec(&rows).unwrap()).unwrap();
}
