use glam::{DMat4,DVec3};
use ingetrazo_core::{groups::Group,mesh::Mesh,scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
fn mesh(y:f64)->Mesh {let mut m=Mesh::new();let v=[DVec3::new(20.,y,0.),DVec3::new(21.,y,0.),DVec3::new(20.,y+1.,0.)].map(|p|m.add_vertex(p));m.add_face(v.to_vec(),vec![]).unwrap();m}
fn add(app:&mut IngeTrazoApp,y:f64){app.save_undo_state();let v=[DVec3::new(20.,y,0.),DVec3::new(21.,y,0.),DVec3::new(20.,y+1.,0.)].map(|p|app.scene.mesh.add_vertex(p));app.scene.mesh.add_face(v.to_vec(),vec![]).unwrap();app.scene.bump_version();}
fn setup(kind:&str)->IngeTrazoApp {let mut app=IngeTrazoApp::new();app.scene=ingetrazo_core::scene::Scene::new();let mut g=Group::with_mesh(kind,mesh(-10.));
 if kind.starts_with("placed") || kind=="container" {g.transform=DMat4::from_translation(DVec3::new(3.,2.,1.));}
 if kind=="container"{g.children.push(Group::with_mesh("child",mesh(-20.)));}
 app.scene.groups.push(g);
 if kind=="placed-component" {let id=app.scene.groups[0].id;app.scene.make_component(id).unwrap();let mut peer=app.scene.groups[0].duplicate_instance();peer.transform=DMat4::from_translation(DVec3::X*10.)*peer.transform;app.scene.groups.push(peer);}
 if kind=="placed-ordinary" {let token=uuid::Uuid::new_v4();app.scene.register_mesh_share(token,app.scene.groups[0].mesh.clone()).unwrap();app.scene.groups[0].mesh_share=Some(token);let mut peer=app.scene.groups[0].duplicate_instance();peer.transform=DMat4::from_translation(DVec3::X*10.)*peer.transform;app.scene.groups.push(peer);}
 app.scene.select(SelectionItem::Group(0));app}
fn source(name:&str)->serde_json::Value {let v:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-active-history-source.json")).unwrap();v["cases"].as_array().unwrap().iter().find(|v|v["name"]==name).unwrap().clone()}
#[test]fn source_local_commands_keep_global_order_and_finish_fold_only_sharing_leaf(){for kind in ["classic","container","placed-component"] {let source=source(&format!("{kind}-local-two-undo-redo-exit"));let mut app=setup(kind);let ids=app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>();app.begin_component_edit();assert_eq!(app.scene.history_context,vec![ids[0]]);add(&mut app,0.);add(&mut app,3.);app.undo();assert_eq!(app.scene.mesh.faces.len(),source["active_undo"]["active_faces"].as_u64().unwrap() as usize);app.redo();assert_eq!(app.scene.mesh.faces.len(),3);app.finish_component_edit();assert_eq!(app.undo_stack.len(),source["exit"]["undo_commands"].as_array().unwrap().len());assert!(app.scene.history_context.is_empty());assert!(app.undo_stack.iter().all(|s|s.history_context.is_empty()));let edited=app.scene.clone();app.undo();assert_eq!(app.scene.groups[0].mesh.faces.len(),source["exit_undo"]["group_faces"].as_u64().unwrap() as usize);app.redo();assert_eq!(app.scene,edited);assert_eq!(app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>(),ids);}}
#[test]fn source_prior_loose_undo_redo_targets_parked_mesh_without_exiting(){for kind in ["classic","container","placed-component"]{let mut app=setup(kind);add(&mut app,0.);app.begin_component_edit();let active=app.scene.clone();app.undo();assert_eq!(app.scene,active);assert_eq!(app.component_edit_stack.len(),1);assert!(app.undo_stack.is_empty());assert_eq!(app.redo_stack.len(),1);app.redo();assert_eq!(app.scene,active);assert_eq!(app.undo_stack.len(),1);assert!(app.redo_stack.is_empty());app.finish_component_edit();assert_eq!(app.scene.mesh.faces.len(),1);assert_eq!(app.undo_stack.len(),1);app.undo();assert!(app.scene.mesh.faces.is_empty());app.redo();assert_eq!(app.scene.mesh.faces.len(),1);}}
#[test]fn unsafe_make_unique_active_undo_refuses_atomically_and_authored_face_survives(){let mut app=setup("placed-ordinary");let original=app.scene.clone();app.begin_component_edit();assert_eq!(app.undo_stack.len(),1);let active=app.scene.clone();let undo=app.undo_stack.clone();let redo=app.redo_stack.clone();app.undo();assert_eq!(app.scene,active);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);assert_eq!(app.component_edit_stack.len(),1);assert!(app.status_message.contains("finish or cancel"));add(&mut app,0.);app.finish_component_edit();assert_eq!(app.scene.groups[0].mesh.faces.len(),2);assert_eq!(app.scene.groups[1].mesh.faces.len(),1);assert_eq!(app.undo_stack.len(),2);app.undo();assert_eq!(app.scene.groups[0].mesh.faces.len(),1);app.undo();assert_eq!(app.scene,original);app.redo();app.redo();assert_eq!(app.scene.groups[0].mesh.faces.len(),2);}
#[test]fn no_edit_entry_exit_preserves_global_redo_and_context_tags_are_runtime_only(){let mut app=setup("classic");add(&mut app,0.);app.undo();assert_eq!(app.redo_stack.len(),1);app.begin_component_edit();let saved=serde_json::to_value(&app.scene).unwrap();assert!(saved.get("history_context").is_none());app.finish_component_edit();assert_eq!(app.redo_stack.len(),1);app.redo();assert_eq!(app.scene.mesh.faces.len(),1);}

#[test]fn nested_ordinary_targets_preserve_three_command_global_order(){let mut app=setup("container");add(&mut app,0.);app.begin_component_edit();let parent_path=app.scene.history_context.clone();add(&mut app,3.);app.scene.select(SelectionItem::Group(0));app.begin_component_edit();assert_eq!(app.scene.history_context.len(),2);add(&mut app,6.);app.finish_component_edit();assert_eq!(app.scene.history_context,parent_path);assert_eq!(app.scene.mesh.faces.len(),2);assert_eq!(app.scene.groups[0].mesh.faces.len(),2);app.finish_component_edit();assert_eq!(app.undo_stack.len(),3);assert!(app.undo_stack.iter().all(|s|s.history_context.is_empty()));let committed=app.scene.clone();app.undo();assert_eq!(app.scene.mesh.faces.len(),1);assert_eq!(app.scene.groups[0].mesh.faces.len(),2);assert_eq!(app.scene.groups[0].children[0].mesh.faces.len(),1);app.undo();assert_eq!(app.scene.mesh.faces.len(),1);assert_eq!(app.scene.groups[0].mesh.faces.len(),1);app.undo();assert_eq!(app.scene.mesh.faces.len(),0);for _ in 0..3 {app.redo();}assert_eq!(app.scene,committed);}
#[test]fn root_target_can_undo_while_nested_and_no_edit_redo_survives_both_exits(){let mut app=setup("container");add(&mut app,0.);app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();let active=app.scene.clone();app.undo();assert_eq!(app.scene,active);assert_eq!(app.component_edit_stack.len(),2);assert_eq!(app.redo_stack.len(),1);app.finish_component_edit();app.finish_component_edit();assert!(app.scene.mesh.faces.is_empty());assert_eq!(app.redo_stack.len(),1);let baked=app.scene.groups.clone();app.redo();assert_eq!(app.scene.mesh.faces.len(),1);assert_eq!(app.scene.groups,baked);}
#[test]fn stale_context_target_and_deleted_placement_refuse_without_stack_or_model_change(){let mut app=setup("classic");app.begin_component_edit();let active=app.scene.clone();let mut stale=active.clone();stale.history_context=vec![uuid::Uuid::new_v4()];app.undo_stack.push(stale);let undo=app.undo_stack.clone();app.undo();assert_eq!(app.scene,active);assert_eq!(app.undo_stack,undo);assert!(app.redo_stack.is_empty());assert!(app.status_message.contains("no longer active"));}

#[test]fn source_line_first_point_consumes_ctrl_z_before_unsafe_global_target(){use ingetrazo_tools::base::{ToolType,ToolContext};use ingetrazo_core::math::Ray;let mut app=setup("placed-ordinary");app.begin_component_edit();app.set_tool(ToolType::Line);let context=ToolContext::new(DVec3::new(4.,5.,0.),Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&context);assert!(app.active_tool.snap_anchor().is_some());let before=app.scene.clone();let undo=app.undo_stack.clone();app.undo();assert!(app.active_tool.snap_anchor().is_none());assert_eq!(app.scene,before);assert_eq!(app.undo_stack,undo);}
#[test]fn unsafe_reference_undo_preserves_even_live_preview(){use ingetrazo_tools::base::{ToolType,ToolContext};use ingetrazo_core::math::Ray;let mut app=setup("placed-ordinary");app.begin_component_edit();app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Move);let start=app.scene.mesh.vertices[0].position;let context=ToolContext::new(start,Ray::new(DVec3::Z,-DVec3::Z));app.dispatch_tool_click(&context);app.dispatch_tool_move(&ToolContext::new(start+DVec3::X,context.ray));let before=app.scene.clone();let anchor=app.active_tool.snap_anchor();let undo=app.undo_stack.clone();app.undo();assert_eq!(app.scene,before);assert_eq!(app.active_tool.snap_anchor(),anchor);assert_eq!(app.undo_stack,undo);assert!(app.status_message.contains("finish or cancel"));}

#[test]fn native_cancel_keeps_legitimate_ancestor_undo_and_redo_checkpoint(){let mut app=setup("container");add(&mut app,0.);app.begin_component_edit();app.scene.select(SelectionItem::Group(0));app.begin_component_edit();app.undo();assert_eq!(app.redo_stack.len(),1);add(&mut app,3.);app.cancel_component_edit();app.cancel_component_edit();assert!(app.scene.mesh.faces.is_empty());assert!(app.undo_stack.is_empty());assert_eq!(app.redo_stack.len(),1);assert_eq!(app.scene.groups[0].mesh.faces.len(),1);assert_eq!(app.scene.groups[0].children[0].mesh.faces.len(),1);app.redo();assert_eq!(app.scene.mesh.faces.len(),1);app.undo();assert!(app.scene.mesh.faces.is_empty());}

#[test]fn unchanged_shared_finish_validates_and_rebinds_pending_redo_instead_of_detaching(){let mut app=setup("placed-component");app.begin_component_edit();add(&mut app,0.);app.undo();assert_eq!(app.scene.mesh.faces.len(),1);assert_eq!(app.redo_stack.len(),1);app.finish_component_edit();assert!(app.undo_stack.is_empty());assert_eq!(app.redo_stack.len(),1);assert!(app.redo_stack[0].history_context.is_empty());assert!(app.scene.groups.iter().all(|g|g.mesh.faces.len()==1));app.redo();assert!(app.scene.groups.iter().all(|g|g.mesh.faces.len()==2));assert_eq!(app.undo_stack.len(),1);app.undo();assert!(app.scene.groups.iter().all(|g|g.mesh.faces.len()==1));assert_eq!(app.redo_stack.len(),1);}

#[test]fn invalid_candidate_or_historical_mesh_refuses_finish_atomically(){for corrupt_history in [false,true]{let mut app=setup("classic");app.begin_component_edit();add(&mut app,0.);if corrupt_history{app.undo_stack[0].mesh.vertices[0].position.x=f64::INFINITY;}else{app.scene.mesh.vertices[0].position.x=f64::INFINITY;}let active=app.scene.clone();let undo=app.undo_stack.clone();let redo=app.redo_stack.clone();app.finish_component_edit();assert_eq!(app.scene,active);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);assert_eq!(app.component_edit_stack.len(),1);assert!(!app.status_message.contains("Committed"));}}

#[test]
fn refused_finish_preserves_live_tool_preview_and_revision() {
    use ingetrazo_tools::base::{ToolType,ToolContext};
    use ingetrazo_core::math::Ray;
    for corrupt_history in [false,true] {
        let mut app=setup("classic");app.begin_component_edit();add(&mut app,3.);
        app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Move);
        let start=app.scene.mesh.vertices[0].position;
        let context=ToolContext::new(start,Ray::new(DVec3::Z,-DVec3::Z));
        app.dispatch_tool_click(&context);app.dispatch_tool_move(&ToolContext::new(start+DVec3::X,context.ray));
        if corrupt_history {app.undo_stack[0].mesh.vertices[0].position.x=f64::INFINITY;}
        else {app.scene.mesh.add_vertex(DVec3::new(f64::INFINITY,0.,0.));}
        let scene=app.scene.clone();let undo=app.undo_stack.clone();let redo=app.redo_stack.clone();
        let anchor=app.active_tool.snap_anchor();let preview=app.active_tool.rubber_band_lines();let revision=app.model_revision;
        app.finish_component_edit();
        assert_eq!(app.scene,scene);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);
        assert_eq!(app.active_tool_type,ToolType::Move);assert_eq!(app.active_tool.snap_anchor(),anchor);
        assert_eq!(app.active_tool.rubber_band_lines(),preview);assert_eq!(app.model_revision,revision);
        assert_eq!(app.component_edit_stack.len(),1);assert!(!app.status_message.contains("Committed"));
    }
}
#[test]
fn successful_finish_retires_live_preview_without_authoring_it() {
    use ingetrazo_tools::base::{ToolType,ToolContext};
    use ingetrazo_core::math::Ray;
    let mut app=setup("classic");app.begin_component_edit();let original_mesh=app.scene.mesh.clone();
    app.scene.select(SelectionItem::Face(0));app.set_tool(ToolType::Move);
    let start=app.scene.mesh.vertices[0].position;let context=ToolContext::new(start,Ray::new(DVec3::Z,-DVec3::Z));
    app.dispatch_tool_click(&context);app.dispatch_tool_move(&ToolContext::new(start+DVec3::X,context.ray));
    assert_ne!(app.scene.mesh,original_mesh);app.finish_component_edit();
    assert!(app.component_edit_stack.is_empty());assert_eq!(app.active_tool_type,ToolType::Select);
    assert!(app.active_tool.snap_anchor().is_none());assert_eq!(app.scene.groups[0].mesh,original_mesh);assert!(app.undo_stack.is_empty());
}

#[test]
fn line_chain_then_pending_point_take_priority_over_make_unique_ancestor() {
    use ingetrazo_tools::base::{ToolType,ToolContext};
    use ingetrazo_core::math::Ray;
    let mut app=setup("placed-ordinary");app.begin_component_edit();
    let unique_history=app.undo_stack.clone();let active_baseline=app.scene.clone();
    assert_eq!(unique_history.len(),1);assert!(unique_history[0].history_context.is_empty());
    let context_path=app.scene.history_context.clone();let ids=app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>();
    app.set_tool(ToolType::Line);
    let first=ToolContext::new(DVec3::new(4.,5.,0.),Ray::new(DVec3::Z,-DVec3::Z));
    let second=ToolContext::new(first.world+DVec3::X,first.ray);
    app.dispatch_tool_click(&first);app.dispatch_tool_click(&second);
    assert_eq!(app.undo_stack.len(),2);assert_eq!(app.active_tool.snap_anchor(),Some(second.world));
    app.undo();
    assert_eq!(app.scene,active_baseline);assert_eq!(app.undo_stack,unique_history);
    assert_eq!(app.active_tool.snap_anchor(),Some(first.world));assert_eq!(app.redo_stack.len(),1);
    app.dispatch_tool_move(&ToolContext::new(first.world+DVec3::Y,first.ray));
    assert!(!app.active_tool.rubber_band_lines().is_empty());
    let scene=app.scene.clone();let undo=app.undo_stack.clone();let redo=app.redo_stack.clone();let revision=app.model_revision;
    app.status_message="pending Line point consumes Undo".into();app.undo();
    assert!(app.active_tool.snap_anchor().is_none());assert!(app.active_tool.rubber_band_lines().is_empty());
    assert_eq!(app.active_tool_type,ToolType::Line);assert_eq!(app.status_message,"pending Line point consumes Undo");
    assert_eq!(app.scene,scene);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);assert_eq!(app.model_revision,revision);
    assert_eq!(app.component_edit_stack.len(),1);assert_eq!(app.scene.history_context,context_path);
    assert_eq!(app.scene.groups.iter().map(|g|g.id).collect::<Vec<_>>(),ids);
    // Now idle Line releases Undo to history; replacing its open mesh is refused.
    app.undo();assert!(app.status_message.contains("finish or cancel"));
    assert_eq!(app.scene,scene);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);assert_eq!(app.model_revision,revision);
    // A newly armed first point must retain precedence even after this refusal.
    app.dispatch_tool_click(&first);app.dispatch_tool_move(&second);
    assert_eq!(app.active_tool.snap_anchor(),Some(first.world));
    app.status_message="rearmed Line owns Undo".into();app.undo();
    assert_eq!(app.status_message,"rearmed Line owns Undo");assert!(app.active_tool.snap_anchor().is_none());
    assert_eq!(app.scene,scene);assert_eq!(app.undo_stack,undo);assert_eq!(app.redo_stack,redo);assert_eq!(app.model_revision,revision);
}
