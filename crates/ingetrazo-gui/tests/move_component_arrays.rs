use glam::{DMat4,DVec3};
use ingetrazo_core::{groups::Group,mesh::Mesh,scene::SelectionItem,math::Ray};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::{move_tool::MoveTool,base::ToolContext};
fn mesh()->Mesh{let mut m=Mesh::new();let v=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|m.add_vertex(p));m.add_face(v.to_vec(),vec![]).unwrap();m}
fn context(p:DVec3)->ToolContext{ToolContext::new(p,Ray::new(DVec3::Z,-DVec3::Z))}

// Source Scene._bake_container_xform is outside History. The active-history
// original controls freeze identity/reflected parent Undo and loose Redo
// retaining this representation. Build the pre-command expectation from the
// pre-entry model, preserving every authored field and independently applying
// that source matrix/axes rule. Only freshly allocated identity is observed.
fn source_baked_before(before:&ingetrazo_core::scene::Scene,after:&ingetrazo_core::scene::Scene)->ingetrazo_core::scene::Scene {
 let mut expected=before.clone();
 fn bake(group:&mut Group,after:&Group,shares:&mut std::collections::HashMap<uuid::Uuid,Mesh>){
  if group.children.is_empty(){return;}
  let frame=group.transform;
  let has_frame=group.component_definition.is_some() || frame!=DMat4::IDENTITY || group.axes_matrix().is_some();
  if frame!=DMat4::IDENTITY {
   if !group.mesh.vertices.is_empty(){group.mesh=ingetrazo_core::containers::transform_mesh(&group.mesh,frame).unwrap();group.mesh_share=after.mesh_share;if let Some(token)=group.mesh_share{shares.insert(token,group.mesh.clone());}}
   for child in &mut group.children{child.transform=frame*child.transform;}
   group.transform=DMat4::IDENTITY;
  }
  if has_frame {let axes=frame*group.axes_matrix().unwrap_or(DMat4::IDENTITY);if !group.native_document.is_object(){group.native_document=serde_json::json!({});}group.native_document["axes"]=serde_json::json!(axes.to_cols_array());}
  // A container child also entered during the tested session retains its own
  // bake. An unentered child has no authored axes and must stay unchanged.
  for child in &mut group.children{if let Some(next)=after.children.iter().find(|g|g.id==child.id){if next.axes_matrix().is_some(){bake(child,next,shares);}}}
 }
 for group in &mut expected.groups {if let Some(next)=after.groups.iter().find(|g|g.id==group.id){if next.axes_matrix().is_some(){bake(group,next,&mut expected.mesh_shares);}}}
 expected
}
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
 let after=app.scene.clone();let expected=source_baked_before(&before,&after);app.undo();assert_eq!(app.scene.groups,expected.groups);assert_eq!(app.scene.component_definitions,expected.component_definitions);app.redo();assert_eq!(app.scene.groups,after.groups);assert_eq!(app.scene.component_definitions,after.component_definitions);
}
#[test]
fn crossing_and_exact_overlap_arrays_match_retained_original_segments(){
 let baseline:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/move-array-crossing-source-controls.json")).unwrap();
 for (index,overlap) in [false,true].into_iter().enumerate(){
  let mut app=IngeTrazoApp::new();let m=&mut app.scene.mesh;
  let a=m.add_vertex(-DVec3::X);let b=m.add_vertex(DVec3::X);let edge=m.add_edge(a,b).unwrap();
  let c=m.add_vertex(if overlap{-DVec3::X+DVec3::Y*2.}else{DVec3::Y});let d=m.add_vertex(if overlap{DVec3::X+DVec3::Y*2.}else{DVec3::Y*5.});m.add_edge(c,d).unwrap();
  app.scene.select(SelectionItem::Edge(edge));let before=app.scene.clone();app.active_tool=Box::new(MoveTool::new());let mut grab=context(DVec3::ZERO);grab.ctrl_held=true;app.dispatch_tool_click(&grab);
  // Source comma is decimal for Move; semicolons delimit the delta.
  assert!(app.dispatch_tool_vcb("0,2,0").is_err());
  app.dispatch_tool_vcb("0;2;0").unwrap();app.dispatch_tool_vcb("2x").unwrap();
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
 app.begin_component_edit();assert_eq!(app.camera.target,target,"original entry leaves the world camera unchanged after live preview cancellation");assert_eq!(app.scene.dimension_frame,DMat4::IDENTITY);assert_eq!(app.scene.drawing_frame,Some(frame));
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
 app.begin_component_edit();assert_eq!(app.scene.dimension_frame,DMat4::IDENTITY);assert_eq!(app.scene.drawing_frame,Some(original));assert_eq!(app.camera,camera);
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
  assert_eq!(app.undo_stack.len(),1);let after=app.scene.clone();let expected=source_baked_before(&before,&after);app.undo();assert_eq!(app.scene.groups,expected.groups);assert_eq!(app.scene.mesh_shares,expected.mesh_shares);
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
  app.begin_component_edit();app.save_undo_state();if nonempty {app.scene.mesh.vertices[0].position.z+=2.;} else {let inverse=app.scene.dimension_frame.inverse();let v=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|app.scene.mesh.add_vertex(inverse.transform_point3(p)));app.scene.mesh.add_face(v.to_vec(),vec![]).unwrap();}
  app.finish_component_edit();let private=transformed&&nonempty;
  assert_eq!(app.scene.groups[0].mesh_share!=app.scene.groups[1].mesh_share,private);
  let expected=source["cases"].as_array().unwrap().iter().find(|c|c["transformed"]==transformed&&c["nonempty"]==nonempty).unwrap();
  for (actual,expected) in app.scene.groups[0].mesh.vertices.iter().zip(expected["selected_vertices"].as_array().unwrap()) {let e=DVec3::new(expected[0].as_f64().unwrap(),expected[1].as_f64().unwrap(),expected[2].as_f64().unwrap());assert!(actual.position.distance(e)<1e-6);}
  for (actual,expected) in app.scene.groups[1].mesh.vertices.iter().zip(expected["sibling_vertices"].as_array().unwrap()) {let e=DVec3::new(expected[0].as_f64().unwrap(),expected[1].as_f64().unwrap(),expected[2].as_f64().unwrap());assert!(actual.position.distance(e)<1e-6);}
  assert_eq!(app.scene.groups[1].mesh.vertices.len(),expected["sibling_vertices"].as_array().unwrap().len());
  if private {assert_eq!(app.scene.groups[1],before.groups[1]);assert_eq!(app.scene.component_definitions[&definition],before.component_definitions[&definition]);}
  let after=app.scene.clone();assert_eq!(app.undo_stack.len(),1);let expected=source_baked_before(&before,&after);app.undo();assert_eq!(app.scene.groups,expected.groups);assert_eq!(app.scene.mesh_shares,expected.mesh_shares);app.redo();assert_eq!(app.scene.groups,after.groups);
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
  let inverse=app.scene.dimension_frame.inverse();app.save_undo_state();let authored=row["authored_world_points"].as_array().unwrap();let ids=authored.iter().map(|p|{let world=DVec3::new(p[0].as_f64().unwrap(),p[1].as_f64().unwrap(),p[2].as_f64().unwrap());app.scene.mesh.add_vertex(inverse.transform_point3(world))}).collect();app.scene.mesh.add_face(ids,vec![]).unwrap();app.scene.bump_version();
  assert_eq!(app.scene.mesh.faces.len(),1);app.finish_component_edit();if nested {app.finish_component_edit();}
  let (selected,sibling,enclosing)=if nested {(&app.scene.groups[0].children[0],&app.scene.groups[0].children[1],app.scene.groups[0].transform)} else {(&app.scene.groups[0],&app.scene.groups[1],DMat4::IDENTITY)};
  assert_eq!(selected.mesh_share,sibling.mesh_share);compare(points(&selected.mesh,enclosing*selected.transform),&row["selected_world_vertices"]);compare(points(&sibling.mesh,enclosing*sibling.transform),&row["sibling_world_vertices"]);
  compare(points(&selected.children[0].mesh,enclosing*selected.transform*selected.children[0].transform),&row["selected_child_world_vertices"]);compare(points(&sibling.children[0].mesh,enclosing*sibling.transform*sibling.children[0].transform),&row["sibling_child_world_vertices"]);
  assert_eq!(app.undo_stack.len(),1);let after=app.scene.clone();let expected=source_baked_before(&before,&after);app.undo();assert_eq!(app.scene.groups,expected.groups);assert_eq!(app.scene.mesh_shares,expected.mesh_shares);app.redo();assert_eq!(app.scene.groups,after.groups);
 }
}

#[test]
fn no_edit_container_bake_preserves_source_zero_history_and_clears_selection() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-reentry-source.json")).unwrap();
 for row in source["cases"].as_array().unwrap() {
  let kind=row["kind"].as_str().unwrap();let mut app=IngeTrazoApp::new();let mut parent=Group::new(kind);
  if kind!="identity-empty" {parent.transform=DMat4::from_translation(DVec3::new(1.,-1.,2.))*DMat4::from_rotation_z(23f64.to_radians())*DMat4::from_scale(DVec3::new(if kind=="reflected-nonempty" {-2.} else {2.},3.,1.));}
  if kind=="reflected-nonempty"||kind=="custom-axes" {parent.mesh=mesh();}
  if kind=="custom-axes" {parent.native_document=serde_json::json!({"axes":(DMat4::from_translation(DVec3::new(4.,-2.,1.))*DMat4::from_rotation_y(17f64.to_radians())).to_cols_array()});}
  parent.children.push(Group::new("child"));app.scene.groups.push(parent);app.scene.make_component(app.scene.groups[0].id).unwrap();app.scene.groups.push(app.scene.groups[0].duplicate_instance());app.scene.select(SelectionItem::Group(0));
  let sibling=app.scene.groups[1].clone();app.begin_component_edit();app.finish_component_edit();assert!(app.scene.selection.is_empty());assert_eq!(app.undo_stack.len(),row["history_after_no_edit"].as_u64().unwrap()as usize);assert_eq!(app.scene.groups[1],sibling);
  if let Some(stored)=row["stored_axes_after_exit"].as_array() {let actual=app.scene.groups[0].native_document["axes"].as_array().unwrap();for (a,b) in actual.iter().zip(stored){assert!((a.as_f64().unwrap()-b.as_f64().unwrap()).abs()<4e-6);}} else {assert!(app.scene.groups[0].native_document.get("axes").is_none());}
 }
}

#[test]
fn original_world_edit_move_scalar_vector_and_leaf_shareback_are_source_exact() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-world-move-source.json")).unwrap();
 fn vector(value:&serde_json::Value)->DVec3 {DVec3::new(value[0].as_f64().unwrap(),value[1].as_f64().unwrap(),value[2].as_f64().unwrap())}
 fn compare(mesh:&Mesh,expected:&serde_json::Value) {let expected=expected.as_array().unwrap();assert_eq!(mesh.vertices.len(),expected.len());for (v,p) in mesh.vertices.iter().zip(expected){assert!(v.position.distance(vector(p))<2e-5,"{:?} vs {p}",v.position);}}
 for row in source["cases"].as_array().unwrap() {
  let kind=row["kind"].as_str().unwrap();let mut app=IngeTrazoApp::new();app.scene=ingetrazo_core::scene::Scene::new();
  let mut group=Group::with_mesh(kind,{let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]).unwrap();mesh.faces[0].attrs.insert("vendor".into(),serde_json::json!(71));mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"uvw":[1.,0.,0.,3.,0.,1.,0.,4.]}));mesh});
  group.transform=DMat4::from_cols_array(&[-2.,0.,0.,0.,0.6,3.,0.,0.,0.,0.4,1.5,0.,3.,-2.,1.,1.]);
  if kind=="container" {group.children.push(Group::new("child"));}
  app.scene.groups.push(group);app.scene.make_component(app.scene.groups[0].id).unwrap();let mut sibling=app.scene.groups[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*10.)*sibling.transform;app.scene.groups.push(sibling);
  if kind=="nested-leaf" {let mut outer=Group::new("outer");outer.transform=DMat4::from_translation(DVec3::new(2.,-1.,4.))*DMat4::from_rotation_z(23f64.to_radians());outer.children=std::mem::take(&mut app.scene.groups);app.scene.groups.push(outer);app.scene.make_component(app.scene.groups[0].id).unwrap();}
  app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();let camera=app.camera.clone();app.begin_component_edit();if kind=="nested-leaf" {app.scene.select(SelectionItem::Group(0));app.begin_component_edit();}
  assert_eq!(app.scene.dimension_frame,DMat4::IDENTITY);assert_eq!(app.camera,camera);compare(&app.scene.mesh,&row["entry_world_points"]);
  app.scene.select(SelectionItem::Face(0));app.set_tool(ingetrazo_tools::base::ToolType::Move);app.dispatch_tool_click(&context(vector(&row["world_grab"])));app.dispatch_tool_move(&context(vector(&row["world_hover"])));
  app.dispatch_tool_vcb(if row["mode"]=="scalar" {"2"} else {"1;-2;0.5"}).unwrap();compare(&app.scene.mesh,&row["active_world_after"]);assert_eq!(app.camera,camera);
  app.finish_component_edit();
  let groups=&app.scene.groups;
  compare(&groups[0].mesh,&row["selected_prototype_after"]);compare(&groups[1].mesh,&row["sibling_prototype_after"]);assert_eq!(groups[0].mesh_share==groups[1].mesh_share,row["shared_after"].as_bool().unwrap());
  for (g,attrs) in [(&groups[0],&row["selected_attrs_after"]),(&groups[1],&row["sibling_attrs_after"])] {
   assert_eq!(g.mesh.faces[0].attrs["vendor"],attrs["vendor"]);
   for (a,b) in g.mesh.faces[0].attrs["texture"]["uvw"].as_array().unwrap().iter().zip(attrs["texture"]["uvw"].as_array().unwrap()) {assert!((a.as_f64().unwrap()-b.as_f64().unwrap()).abs()<2e-5);}
  }
  if kind=="nested-leaf" {app.finish_component_edit();}
assert_eq!(app.camera,camera);assert_eq!(app.undo_stack.len(),1);let after=app.scene.clone();let mut expected=source_baked_before(&before,&after);app.undo();expected.selection=app.scene.selection.clone();expected.selection_order=app.scene.selection_order.clone();expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);app.redo();let mut expected=after;expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);
  let path=std::env::temp_dir().join(format!("world-edit-{}.igz",uuid::Uuid::new_v4()));
  let document=ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
  ingetrazo_formats::igz::save_igz(&app.scene,&path).unwrap();
  let reopened=ingetrazo_formats::igz::load_igz(&path).unwrap();
  assert_eq!(ingetrazo_formats::igz::scene_to_igz_document(&reopened),document);
  let groups=if kind=="nested-leaf" {&reopened.groups[0].children} else {&reopened.groups};
  compare(&groups[0].mesh,&row["selected_prototype_after"]);compare(&groups[1].mesh,&row["sibling_prototype_after"]);
  assert_eq!(groups[0].mesh_share==groups[1].mesh_share,row["shared_after"].as_bool().unwrap());
  if let Some(directory)=std::env::var_os("INGETRAZO_WORLD_EDIT_REOPEN_DIR") {
   let directory=std::path::PathBuf::from(directory);std::fs::create_dir_all(&directory).unwrap();
   std::fs::copy(&path,directory.join(format!("{}-{}.igz",kind,row["mode"].as_str().unwrap()))).unwrap();
  }
  std::fs::remove_file(path).unwrap();

 }
}

#[test]
fn nested_no_edit_bakes_do_not_add_history_or_mask_prior_parent_commands() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-nested-noedit-source.json")).unwrap();
 for row in source["cases"].as_array().unwrap() {
  let mut app=IngeTrazoApp::new();app.scene=ingetrazo_core::scene::Scene::new();let mut child_mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|child_mesh.add_vertex(p));child_mesh.add_face(ids.to_vec(),vec![]).unwrap();
  let mut child=Group::with_mesh("inner",child_mesh);child.transform=DMat4::from_translation(DVec3::new(2.,1.,0.))*DMat4::from_rotation_y(17f64.to_radians());if row["inner_container"]==true {child.children.push(Group::new("grandchild"));}
  let mut outer=Group::new("outer");outer.transform=DMat4::from_translation(DVec3::new(2.,-1.,4.))*DMat4::from_rotation_z(23f64.to_radians())*DMat4::from_scale(DVec3::new(-2.,3.,1.));outer.children.push(child);app.scene.groups.push(outer);app.scene.make_component(app.scene.groups[0].id).unwrap();let mut sibling=app.scene.groups[0].children[0].duplicate_instance();sibling.transform=DMat4::from_translation(DVec3::X*8.)*sibling.transform;app.scene.groups[0].children.push(sibling);
  app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();app.begin_component_edit();
  match row["parent_command"].as_str().unwrap() {
   "face"=>{app.save_undo_state();let ids=[DVec3::X*20.,DVec3::X*21.,DVec3::new(20.,1.,0.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();app.scene.bump_version();},
   "move"=>{app.scene.select(SelectionItem::Group(0));app.set_tool(ingetrazo_tools::base::ToolType::Move);app.dispatch_tool_click(&context(DVec3::ZERO));app.dispatch_tool_vcb("1;0;0").unwrap();},
   _=>{},
  }
  app.scene.clear_selection();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();app.finish_component_edit();app.finish_component_edit();assert!(app.component_edit_stack.is_empty());
  assert_eq!(app.undo_stack.len(),row["history_after_outer"].as_u64().unwrap() as usize,"{row}");assert_eq!(app.scene.groups[0].mesh.faces.len(),row["parent_faces"].as_u64().unwrap()as usize);
  let selected=&app.scene.groups[0].children[0];let sibling=&app.scene.groups[0].children[1];assert_eq!(selected.mesh_share==sibling.mesh_share,row["selected_shared_with_sibling"].as_bool().unwrap());
  for (group,key) in [(selected,"selected_matrix"),(sibling,"sibling_matrix")] {for (a,b) in group.transform.to_cols_array().iter().zip(row[key].as_array().unwrap()) {assert!((*a-b.as_f64().unwrap()).abs()<4e-6);}}
  if row["authored_parent"]==true {let after=app.scene.clone();let expected=source_baked_before(&before,&after);app.undo();
   if row["inner_container"]==true && row["parent_command"]=="move" {
    // The source command composes inverse Move against the CURRENT baked
    // child, rather than restoring the pre-bake representation. Compare the
    // actual frozen original Undo matrix/mesh/axes, not a guessed snapshot.
    let undo_source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-nested-bake-undo-source.json")).unwrap();let reference=undo_source["cases"].as_array().unwrap().iter().find(|c|c["inner_container"]==true && c["parent_command"]=="move").unwrap();let child=&app.scene.groups[0].children[0];
    for (a,b) in child.transform.to_cols_array().iter().zip(reference["selected_matrix"].as_array().unwrap()){assert!((*a-b.as_f64().unwrap()).abs()<4e-6);}
    for (vertex,point) in child.mesh.vertices.iter().zip(reference["selected_vertices"].as_array().unwrap()){let p=DVec3::new(point[0].as_f64().unwrap(),point[1].as_f64().unwrap(),point[2].as_f64().unwrap());assert!(vertex.position.distance(p)<4e-6);}
    for (a,b) in child.axes_matrix().unwrap().to_cols_array().iter().zip(reference["axes_after_undo"].as_array().unwrap()){assert!((*a-b.as_f64().unwrap()).abs()<4e-6);}
    assert_eq!(child.mesh_share==app.scene.groups[0].children[1].mesh_share,reference["selected_shared_with_sibling"].as_bool().unwrap());assert_eq!(&child.mesh,&app.scene.mesh_shares[&child.mesh_share.unwrap()]);assert_eq!(child.id,before.groups[0].children[0].id);assert_eq!(app.scene.mesh,before.mesh);
   } else {assert_eq!(app.scene,expected);}
   app.redo();assert_eq!(app.scene,after);
  }
 }
}

#[test]
fn actual_viewport_plain_copied_leaf_entry_is_unique_only_outside_definition() {
 let source:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-entry-unique-source.json")).unwrap();
 fn vector(v:&serde_json::Value)->DVec3 {DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
 for row in source["cases"].as_array().unwrap() {
  let kind=row["kind"].as_str().unwrap();let mut app=IngeTrazoApp::new();app.scene=ingetrazo_core::scene::Scene::new();let mut mesh=Mesh::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|mesh.add_vertex(p));mesh.add_face(ids.to_vec(),vec![]).unwrap();mesh.faces[0].attrs.insert("vendor".into(),serde_json::json!(71));mesh.faces[0].attrs.insert("texture".into(),serde_json::json!({"uvw":[1.,0.,0.,3.,0.,1.,0.,4.]}));let token=uuid::Uuid::new_v4();app.scene.register_mesh_share(token,mesh.clone()).unwrap();
  let mut group=Group::with_mesh(kind,mesh);group.mesh_share=Some(token);let component=kind=="root-component-shared";group.native_document=serde_json::json!({"component":component,"xform":row["before_matrix"]});
  // Source nested frame already includes its parent's entry bake.
  let parent_frame=DMat4::from_translation(DVec3::new(2.,-1.,4.));let world=if let Some(values)=row["before_matrix"].as_array() {let raw:Vec<_>=values.iter().map(|v|v.as_f64().unwrap()).collect();DMat4::from_cols_array(&raw.try_into().unwrap())}else{DMat4::IDENTITY};
  group.transform=if kind.starts_with("nested") {parent_frame.inverse()*world} else {world};app.scene.groups.push(group);if component {app.scene.make_component(app.scene.groups[0].id).unwrap();}
  if kind!="root-plain-unshared" {let mut sibling=app.scene.groups[0].duplicate_instance();if kind!="classic-plain-shared" {sibling.transform=DMat4::from_translation(DVec3::X*10.)*sibling.transform;}app.scene.groups.push(sibling);}
  if kind.starts_with("nested") {let mut outer=Group::new("outer");outer.transform=parent_frame;outer.native_document=serde_json::json!({"component":kind=="nested-component-parent","xform":parent_frame.to_cols_array()});outer.children=std::mem::take(&mut app.scene.groups);app.scene.groups.push(outer);if kind=="nested-component-parent" {app.scene.make_component(app.scene.groups[0].id).unwrap();}}
  app.scene.select(SelectionItem::Group(0));let before=app.scene.clone();let camera=app.camera.clone();app.begin_component_edit();if kind.starts_with("nested") {if kind=="nested-deleted-sibling" {app.scene.select(SelectionItem::Group(1));app.delete_selection();app.scene.clear_selection();}app.scene.select(SelectionItem::Group(0));app.begin_component_edit();}
  // This control checks entry geometry/policy and global History after exit.
  assert_eq!(app.camera,camera);assert_eq!(app.scene.mesh.vertices.len(),row["entry_world_points"].as_array().unwrap().len());
  for (p,e) in app.scene.mesh.vertices.iter().zip(row["entry_world_points"].as_array().unwrap()){assert!(p.position.distance(vector(e))<2e-5);}
  app.finish_component_edit();assert_eq!(app.undo_stack.len(),row["history_after_no_edit_exit"].as_u64().unwrap()as usize);
  let selected=&app.scene.groups[0];assert_eq!(selected.mesh_share!=Some(token),row["source_unique_on_entry"].as_bool().unwrap());for (p,e) in selected.mesh.vertices.iter().zip(row["after_points"].as_array().unwrap()){assert!(p.position.distance(vector(e))<2e-5);}
  if row["source_unique_on_entry"]==true {assert_eq!(selected.transform,DMat4::IDENTITY);assert!(selected.native_document["xform"].is_null());assert!(selected.axes_matrix().is_some());}
  if row["sibling_in_context_after"]==true {assert_eq!(selected.mesh_share==app.scene.groups[1].mesh_share,row["sibling_same_mesh_after"].as_bool().unwrap());}
  assert_eq!(selected.mesh.faces[0].attrs["vendor"],row["after_attrs"]["vendor"]);for (a,b) in selected.mesh.faces[0].attrs["texture"]["uvw"].as_array().unwrap().iter().zip(row["after_attrs"]["texture"]["uvw"].as_array().unwrap()) {assert!((a.as_f64().unwrap()-b.as_f64().unwrap()).abs()<2e-5);}
  if kind.starts_with("nested") {app.finish_component_edit();assert_eq!(app.undo_stack.len(),row["history_after_no_edit_exit"].as_u64().unwrap()as usize);}
if row["source_unique_on_entry"]==true {let after=app.scene.clone();let mut expected=source_baked_before(&before,&after);app.undo();expected.selection=app.scene.selection.clone();expected.selection_order=app.scene.selection_order.clone();expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);app.redo();let mut expected=after;expected.version=app.scene.version;expected.view_version=app.scene.view_version;assert_eq!(app.scene,expected);}
  let document=ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
  let path=std::env::temp_dir().join(format!("unique-entry-{}.igz",uuid::Uuid::new_v4()));
  ingetrazo_formats::igz::save_igz(&app.scene,&path).unwrap();
  let reopened=ingetrazo_formats::igz::load_igz(&path).unwrap();
  assert_eq!(ingetrazo_formats::igz::scene_to_igz_document(&reopened),document,"native reopen {kind}");
  let selected=if kind.starts_with("nested") {&reopened.groups[0].children[0]} else {&reopened.groups[0]};
  assert_eq!(selected.mesh.faces[0].attrs["vendor"],row["after_attrs"]["vendor"]);
  if row["source_unique_on_entry"]==true {assert_eq!(selected.transform,DMat4::IDENTITY);assert!(selected.axes_matrix().is_some());}
  if let Some(directory)=std::env::var_os("INGETRAZO_ENTRY_REOPEN_DIR") {
   let directory=std::path::PathBuf::from(directory);std::fs::create_dir_all(&directory).unwrap();
   std::fs::copy(&path,directory.join(format!("{kind}.igz"))).unwrap();
  }
  std::fs::remove_file(path).unwrap();

 }
}
