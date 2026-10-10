use glam::{DMat4,DVec3};
use ingetrazo_core::{groups::Group,mesh::Mesh,scene::SelectionItem,math::Ray};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::{move_tool::MoveTool,base::ToolContext};
fn mesh()->Mesh{let mut m=Mesh::new();let v=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|m.add_vertex(p));m.add_face(v.to_vec(),vec![]).unwrap();m}
fn context(p:DVec3)->ToolContext{ToolContext::new(p,Ray::new(DVec3::Z,-DVec3::Z))}
#[test]
fn linked_nested_group_spaced_arrays_keep_definitions_and_atomic_history(){
 let mut app=IngeTrazoApp::new();let mut g=Group::with_mesh("linked",mesh());
 g.transform=DMat4::from_translation(DVec3::Y*2.)*DMat4::from_rotation_z(0.3);
 g.children.push(Group::with_mesh("nested private",mesh()));app.scene.groups.push(g);
 let definition=app.scene.make_component(app.scene.groups[0].id).unwrap();
 app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();
 app.active_tool=Box::new(MoveTool::new());let mut grab=context(DVec3::ZERO);grab.ctrl_held=true;
 app.dispatch_tool_click(&grab);app.dispatch_tool_move(&context(DVec3::X*3.));
 app.dispatch_tool_vcb("3x200cm").unwrap();assert_eq!(app.undo_stack.len(),1);
 assert_eq!(app.scene.component_definitions,before.component_definitions);
 assert_eq!(app.scene.groups.iter().map(|g|g.transform.w_axis.x).collect::<Vec<_>>(),vec![0.,2.,4.,6.]);
 let ids=app.scene.groups.iter().flat_map(|g|[g.id,g.children[0].id]).collect::<std::collections::HashSet<_>>();assert_eq!(ids.len(),8);
 assert!(app.scene.groups.iter().all(|g|g.component_definition==Some(definition)));
 app.dispatch_tool_vcb("2*400cm").unwrap();assert_eq!(app.undo_stack.len(),1);assert_eq!(app.scene.groups.len(),3);
 let committed=app.scene.clone();assert!(app.dispatch_tool_vcb("2xNaNm").is_err());assert_eq!(app.scene,committed);assert_eq!(app.undo_stack.len(),1);
 app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.mesh,before.mesh);assert_eq!(app.scene.selection,before.selection);assert_eq!(app.scene.component_definitions,before.component_definitions);
 app.redo();assert_eq!(app.scene.groups,committed.groups);assert_eq!(app.scene.component_definitions,committed.component_definitions);
}
#[test]
fn local_component_group_move_commits_shared_definition_once_and_undo_restores(){
 let mut app=IngeTrazoApp::new();let mut outer=Group::new("outer");outer.children.push(Group::with_mesh("child",mesh()));
 app.scene.groups.push(outer);let definition=app.scene.make_component(app.scene.groups[0].id).unwrap();let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.);app.scene.groups.push(sibling);
 app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();assert_eq!(app.scene.groups.len(),1);
 app.scene.select(SelectionItem::Group(0));app.active_tool=Box::new(MoveTool::new());app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_move(&context(DVec3::Y*3.));app.dispatch_tool_click(&context(DVec3::Y*3.));
 assert_eq!(app.undo_stack.len(),1);app.finish_component_edit();assert_eq!(app.undo_stack.len(),1);
 assert_eq!(app.scene.component_definitions[&definition].children[0].transform.w_axis.y,3.);
 assert!(app.scene.groups.iter().all(|g|g.children[0].transform.w_axis.y==3.));assert_eq!(app.scene.groups[1].transform,before.groups[1].transform);
 let after=app.scene.clone();app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.component_definitions,before.component_definitions);app.redo();assert_eq!(app.scene.groups,after.groups);assert_eq!(app.scene.component_definitions,after.component_definitions);
}
#[test]
fn crossing_and_exact_overlap_arrays_match_retained_original_segments(){
 let baseline:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/move-array-crossing-source-controls.json")).unwrap();
 for (index,overlap) in [false,true].into_iter().enumerate(){
  let mut app=IngeTrazoApp::new();let m=&mut app.scene.mesh;
  let a=m.add_vertex(-DVec3::X);let b=m.add_vertex(DVec3::X);let edge=m.add_edge(a,b).unwrap();
  let c=m.add_vertex(if overlap{-DVec3::X+DVec3::Y*2.}else{DVec3::Y});let d=m.add_vertex(if overlap{DVec3::X+DVec3::Y*2.}else{DVec3::Y*5.});m.add_edge(c,d).unwrap();
  app.scene.select(SelectionItem::Edge(edge));let before=app.scene.clone();app.active_tool=Box::new(MoveTool::new());let mut grab=context(DVec3::ZERO);grab.ctrl_held=true;app.dispatch_tool_click(&grab);
  app.dispatch_tool_vcb("0,2,0").unwrap();app.dispatch_tool_vcb("2x").unwrap();
  fn segments(m:&Mesh)->serde_json::Value{let mut out=m.edges.iter().map(|e|{let mut p=[m.vertices[e.v0].position.to_array(),m.vertices[e.v1].position.to_array()];p.sort_by(|a,b|a.partial_cmp(b).unwrap());p}).collect::<Vec<_>>();out.sort_by(|a,b|a.partial_cmp(b).unwrap());serde_json::json!(out)}
  assert_eq!(segments(&app.scene.mesh),baseline["cases"][index]["copied_segments"]);assert_eq!(app.undo_stack.len(),1);
  let after=app.scene.clone();app.undo();assert_eq!(app.scene.mesh,before.mesh);assert_eq!(segments(&app.scene.mesh),baseline["cases"][index]["undo_segments"]);
  app.redo();assert_eq!(app.scene.mesh,after.mesh);assert_eq!(segments(&app.scene.mesh),baseline["cases"][index]["redo_segments"]);
 }
}
