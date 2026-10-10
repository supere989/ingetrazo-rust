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
 assert_eq!(app.scene.component_definitions[&definition].children[0].transform.w_axis.y,3.);
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
