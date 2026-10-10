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
fn local_component_group_move_retains_sibling_placement_and_undo_restores(){
 let mut app=IngeTrazoApp::new();let mut outer=Group::new("outer");outer.children.push(Group::with_mesh("child",mesh()));
 app.scene.groups.push(outer);let definition=app.scene.make_component(app.scene.groups[0].id).unwrap();let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.);app.scene.groups.push(sibling);
 app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();assert_eq!(app.scene.groups.len(),1);
 app.scene.select(SelectionItem::Group(0));app.active_tool=Box::new(MoveTool::new());app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_move(&context(DVec3::Y*3.));app.dispatch_tool_click(&context(DVec3::Y*3.));
 assert_eq!(app.undo_stack.len(),1);app.finish_component_edit();assert_eq!(app.undo_stack.len(),1);
 assert_eq!(app.scene.component_definitions[&definition].children[0].transform.w_axis.y,0.,"definition template is not a shared placement matrix");
 assert_eq!(app.scene.groups[0].children[0].transform.w_axis.y,3.);assert_eq!(app.scene.groups[1].children[0].transform.w_axis.y,0.,"original shared meshes retain per-instance child placement matrices");assert_eq!(app.scene.groups[1].transform,before.groups[1].transform);
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
#[test]
fn entering_edit_context_retires_live_group_preview_before_camera_frame_capture(){
 let mut app=IngeTrazoApp::new();let mut g=Group::with_mesh("target",mesh());g.transform=DMat4::from_translation(DVec3::X*2.);app.scene.groups.push(g);app.scene.select(SelectionItem::Group(0));
 let frame=app.scene.groups[0].transform;let target=app.camera.target;
 app.active_tool=Box::new(MoveTool::new());app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_move(&context(DVec3::Y*3.));assert_eq!(app.scene.groups[0].transform,DMat4::from_translation(DVec3::Y*3.)*frame);
 app.begin_component_edit();assert_eq!(app.camera.target,frame.inverse().transform_point3(target),"context capture must follow live preview cancellation");assert_eq!(app.scene.dimension_frame,frame);
 app.cancel_component_edit();assert_eq!(app.scene.groups[0].transform,frame);assert_eq!(app.camera.target,target);
}
#[test]
fn refused_singular_context_keeps_live_preview_and_revision_until_user_cancel(){
 let mut app=IngeTrazoApp::new();let mut group=Group::with_mesh("singular",mesh());group.transform=DMat4::from_scale(DVec3::new(1.,0.,1.));app.scene.groups.push(group);app.scene.select(SelectionItem::Group(0));
 let original=app.scene.groups[0].transform;app.active_tool=Box::new(MoveTool::new());app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_move(&context(DVec3::Y*3.));
 let live=app.scene.clone();let revision=app.model_revision;let anchor=app.active_tool.snap_anchor();app.begin_component_edit();
 assert!(app.component_edit_stack.is_empty());assert_eq!(app.scene,live);assert_eq!(app.model_revision,revision);assert_eq!(app.active_tool.snap_anchor(),anchor);assert!(app.status_message.contains("singular"));
 app.set_tool(ingetrazo_tools::base::ToolType::Select);assert_eq!(app.scene.groups[0].transform,original);assert!(app.active_tool.snap_anchor().is_none());
}
#[test]
fn rotated_scaled_context_uses_restored_grab_placement_and_cancel_camera(){
 let mut app=IngeTrazoApp::new();let mut group=Group::with_mesh("placed",mesh());group.transform=DMat4::from_translation(DVec3::X*2.)*DMat4::from_rotation_z(0.37)*DMat4::from_scale(DVec3::new(2.,3.,4.));app.scene.groups.push(group);app.scene.select(SelectionItem::Group(0));
 let original=app.scene.groups[0].transform;let camera=app.camera.clone();app.active_tool=Box::new(MoveTool::new());app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_move(&context(DVec3::Y*3.));
 app.begin_component_edit();assert_eq!(app.scene.dimension_frame,original);assert_eq!(app.camera.target,original.inverse().transform_point3(camera.target));
 app.cancel_component_edit();assert_eq!(app.scene.groups[0].transform,original);assert_eq!(app.camera,camera);assert!(app.undo_stack.is_empty());
}
#[test]
fn shared_child_mesh_refresh_and_native_reopen_retain_independent_placements(){
 let mut app=IngeTrazoApp::new();let mut outer=Group::new("outer");outer.children.push(Group::with_mesh("child",mesh()));app.scene.groups.push(outer);let definition=app.scene.make_component(app.scene.groups[0].id).unwrap();app.scene.groups.push(app.scene.groups[0].duplicate_instance());
 app.scene.groups[0].children[0].transform=DMat4::from_translation(DVec3::Y*3.);app.scene.groups[1].children[0].transform=DMat4::from_translation(DVec3::Z*2.);
 let ids=[app.scene.groups[0].children[0].id,app.scene.groups[1].children[0].id];
 let mut changed=app.scene.component_definitions[&definition].clone();changed.children[0].mesh.vertices[0].position.z=7.;app.scene.replace_component_definition(definition,changed).unwrap();
 for (i,transform) in [DMat4::from_translation(DVec3::Y*3.),DMat4::from_translation(DVec3::Z*2.)].into_iter().enumerate(){assert_eq!(app.scene.groups[i].children[0].transform,transform);assert_eq!(app.scene.groups[i].children[0].id,ids[i]);assert_eq!(app.scene.groups[i].children[0].mesh.vertices[0].position.z,7.);}
 let path=std::env::temp_dir().join(format!("ingetrazo-placement-{}.igz",uuid::Uuid::new_v4()));ingetrazo_formats::igz::save_igz(&app.scene,&path).unwrap();let loaded=ingetrazo_formats::igz::load_igz(&path).unwrap();if let Some(artifact)=std::env::var_os("INGETRAZO_PLACEMENT_SAVE_ARTIFACT"){std::fs::copy(&path,artifact).unwrap();}std::fs::remove_file(path).unwrap();
 for i in 0..2{assert_eq!(loaded.groups[i].children[0].transform,app.scene.groups[i].children[0].transform);assert_eq!(loaded.groups[i].children[0].id,ids[i]);assert_eq!(loaded.groups[i].children[0].mesh.vertices[0].position.z,7.);}
}

#[test]
fn container_finish_copy_array_delete_match_source_independent_lists() {
 for action in ["copy","array","delete"] {
  let mut app=IngeTrazoApp::new();let mut outer=Group::new("container");outer.transform=DMat4::from_translation(DVec3::new(1.,-1.,0.));outer.children.push(Group::with_mesh("ordinary child",mesh()));
  app.scene.groups.push(outer);app.scene.make_component(app.scene.groups[0].id).unwrap();
  let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.)*sibling.transform;app.scene.groups.push(sibling);
  app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();app.scene.select(SelectionItem::Group(0));
  if action=="delete" {app.delete_selection();} else {
   app.active_tool=Box::new(MoveTool::new());let mut grab=context(DVec3::ZERO);grab.ctrl_held=true;app.dispatch_tool_click(&grab);app.dispatch_tool_move(&context(DVec3::X*2.));app.dispatch_tool_vcb(if action=="copy" {"1x200cm"} else {"3x200cm"}).unwrap();
  }
  app.finish_component_edit();assert!(app.component_edit_stack.is_empty());
  assert_eq!(app.scene.groups[0].children.len(),match action {"copy"=>2,"array"=>4,_=>0});
  assert_eq!(app.scene.groups[1],before.groups[1],"source sibling whole placement subtree must remain unchanged");
  assert_eq!(app.undo_stack.len(),1);let after=app.scene.clone();app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.mesh_shares,before.mesh_shares);
  app.redo();assert_eq!(app.scene.groups,after.groups);assert_eq!(app.scene.mesh_shares,after.mesh_shares);
 }
}

#[test]
fn transformed_nonempty_container_own_mesh_detaches_source_prototype() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-own-mesh-source.json")).unwrap();
 for transformed in [false,true] {for nonempty in [false,true] {
  let mut app=IngeTrazoApp::new();let mut parent=Group::new("container");
  if nonempty {let v=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|parent.mesh.add_vertex(p));parent.mesh.add_face(v.to_vec(),vec![]).unwrap();}
  if transformed {parent.transform=DMat4::from_translation(DVec3::new(1.,-1.,0.))*DMat4::from_rotation_z(23f64.to_radians())*DMat4::from_scale(DVec3::new(2.,3.,1.));}
  parent.children.push(Group::new("child"));app.scene.groups.push(parent);let definition=app.scene.make_component(app.scene.groups[0].id).unwrap();
  let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.)*sibling.transform;app.scene.groups.push(sibling);app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();
  app.begin_component_edit();if nonempty {app.scene.mesh.vertices[0].position.z+=2.;} else {let inverse=app.scene.dimension_frame.inverse();let v=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|app.scene.mesh.add_vertex(inverse.transform_point3(p)));app.scene.mesh.add_face(v.to_vec(),vec![]).unwrap();}
  app.finish_component_edit();let private=transformed&&nonempty;
  assert_eq!(app.scene.groups[0].mesh_share!=app.scene.groups[1].mesh_share,private);
  let expected=source["cases"].as_array().unwrap().iter().find(|c|c["transformed"]==transformed&&c["nonempty"]==nonempty).unwrap();
  for (actual,expected) in app.scene.groups[1].mesh.vertices.iter().zip(expected["sibling_vertices"].as_array().unwrap()) {let e=DVec3::new(expected[0].as_f64().unwrap(),expected[1].as_f64().unwrap(),expected[2].as_f64().unwrap());assert!(actual.position.distance(e)<1e-6);}
  assert_eq!(app.scene.groups[1].mesh.vertices.len(),expected["sibling_vertices"].as_array().unwrap().len());
  if private {assert_eq!(app.scene.groups[1],before.groups[1]);assert_eq!(app.scene.component_definitions[&definition],before.component_definitions[&definition]);}
  let after=app.scene.clone();assert_eq!(app.undo_stack.len(),1);app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.mesh_shares,before.mesh_shares);app.redo();assert_eq!(app.scene.groups,after.groups);
 }}
}

#[test]
fn root_and_nested_empty_container_world_authoring_matches_original_prototype_surfaces() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-world-authoring-source.json")).unwrap();
 fn frame()->DMat4 {DMat4::from_translation(DVec3::new(1.,-1.,0.))*DMat4::from_rotation_z(23f64.to_radians())*DMat4::from_scale(DVec3::new(2.,3.,1.))}
 fn points(mesh:&Mesh,transform:DMat4)->Vec<DVec3>{mesh.vertices.iter().map(|v|transform.transform_point3(v.position)).collect()}
 fn compare(actual:Vec<DVec3>,expected:&serde_json::Value) {let expected=expected.as_array().unwrap();assert_eq!(actual.len(),expected.len());for p in actual {assert!(expected.iter().any(|value|p.distance(DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap()))<2e-5),"world point {p:?} missing from source {expected:?}");}}
 for row in source["cases"].as_array().unwrap() {
  let nested=row["nested"].as_bool().unwrap();let mut app=IngeTrazoApp::new();let mut parent=Group::new("authored container");parent.transform=if nested {DMat4::from_translation(DVec3::new(2.,1.,0.))} else {frame()};
  let mut child=Mesh::new();let v=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|child.add_vertex(p));child.add_face(v.to_vec(),vec![]).unwrap();parent.children.push(Group::with_mesh("existing child",child));app.scene.groups.push(parent);app.scene.make_component(app.scene.groups[0].id).unwrap();let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.)*sibling.transform;app.scene.groups.push(sibling);
  if nested {let mut outer=Group::new("outer");outer.transform=frame();outer.children=std::mem::take(&mut app.scene.groups);app.scene.groups.push(outer);app.scene.make_component(app.scene.groups[0].id).unwrap();}
  app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();if nested {app.scene.select(SelectionItem::Group(0));app.begin_component_edit();}
  let inverse=app.scene.dimension_frame.inverse();app.active_tool=Box::new(ingetrazo_tools::line::LineTool::new());
  let authored=row["authored_world_points"].as_array().unwrap();for p in authored.iter().chain(std::iter::once(&authored[0])) {let world=DVec3::new(p[0].as_f64().unwrap(),p[1].as_f64().unwrap(),p[2].as_f64().unwrap());app.dispatch_tool_click(&context(inverse.transform_point3(world)));}
  assert_eq!(app.scene.mesh.faces.len(),1);app.finish_component_edit();if nested {app.finish_component_edit();}
  let (selected,sibling,enclosing)=if nested {(&app.scene.groups[0].children[0],&app.scene.groups[0].children[1],app.scene.groups[0].transform)} else {(&app.scene.groups[0],&app.scene.groups[1],DMat4::IDENTITY)};
  assert_eq!(selected.mesh_share,sibling.mesh_share);compare(points(&selected.mesh,enclosing*selected.transform),&row["selected_world_vertices"]);compare(points(&sibling.mesh,enclosing*sibling.transform),&row["sibling_world_vertices"]);
  compare(points(&selected.children[0].mesh,enclosing*selected.transform*selected.children[0].transform),&row["selected_child_world_vertices"]);compare(points(&sibling.children[0].mesh,enclosing*sibling.transform*sibling.children[0].transform),&row["sibling_child_world_vertices"]);
  assert_eq!(app.undo_stack.len(),1);let after=app.scene.clone();app.undo();assert_eq!(app.scene.groups,before.groups);assert_eq!(app.scene.mesh_shares,before.mesh_shares);app.redo();assert_eq!(app.scene.groups,after.groups);
 }
}
