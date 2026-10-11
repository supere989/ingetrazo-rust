fn observe(app: &ingetrazo_gui::IngeTrazoApp, name: &str) -> serde_json::Value {
    serde_json::json!({"name":name,"faces":app.scene.mesh.faces.iter().map(|f|serde_json::json!({"attrs":f.attrs,"layer":f.layer})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()})
}
fn main() {
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| app.scene.mesh.add_vertex(p))
        .collect();
    let id = app.scene.mesh.add_face(ids, vec![]).unwrap();
    app.scene.mesh.faces[id]
        .attrs
        .insert("color".into(), serde_json::json!([0.2, 0.4, 0.8]));
    app.scene.mesh.faces[id]
        .attrs
        .insert("layer".into(), serde_json::json!("Imported"));
    app.scene.mesh.faces[id].layer = Some("Imported".into());
    let uids = app
        .scene
        .mesh
        .vertices
        .iter()
        .map(|v| v.uid)
        .collect::<Vec<_>>();
    let mut states = vec![observe(&app, "imported")];
    app.engineering_python_enabled = true;
    app.run_script(
        "f=mesh.faces[0]; token=41; f.attrs['mat']='Edited'; f.loop[0].position.setZ(2)",
    );
    assert_eq!(app.scene.mesh.vertices[0].position.z, 2.);
    app.run_script("token+=1; assert token==42; assert f is mesh.faces[0]");
    assert_eq!(app.undo_stack.len(), 1);
    states.push(observe(&app, "edited"));
    app.undo();
    assert_eq!(app.scene.mesh.vertices[0].position.z, 0.);
    states.push(observe(&app, "undone"));
    app.redo();
    states.push(observe(&app, "redone"));
    assert_eq!(
        app.scene
            .mesh
            .vertices
            .iter()
            .map(|v| v.uid)
            .collect::<Vec<_>>(),
        uids
    );
    let budget = app.engineering_python_budget().unwrap();
    drop(app);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::json!({"states":states,"actual_native_scene_import":true,"packed_uids_preserved":true,"native_document_edit_return":true,"final_charge":budget.snapshot().total(),"production_enabled":false})
    );
}
