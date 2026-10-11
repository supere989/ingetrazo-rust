use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let mut app = IngeTrazoApp::new();
    app.engineering_python_enabled = true;
    let mut em = ingetrazo_core::mesh::Mesh::new();
    let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| em.add_vertex(p))
        .collect();
    em.add_face(ids, vec![]).unwrap();
    app.scene
        .groups
        .push(ingetrazo_core::groups::Group::with_mesh("Existing", em));
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-groups/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    for s in recipes.as_array().unwrap() {
        let code = s["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{:?}",
                app.console_logs
            );
        }
        if let Some(redo) = s["history"].as_bool() {
            if redo { app.redo() } else { app.undo() }
        }
        let state = serde_json::json!({"name":s["name"],"groups":app.scene.groups.iter().map(|g|serde_json::json!({"name":g.name,"layer":g.layer,"hidden":g.hidden,"positions":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"attrs":g.mesh.faces.iter().map(|f|&f.attrs).collect::<Vec<_>>()})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()});
        let check = s["check"].as_str().unwrap();
        if !check.is_empty() {
            app.run_script(check);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{:?}",
                app.console_logs
            );
        }
        states.push(state);
    }
    let budget = app.engineering_python_budget().unwrap();
    drop(app);
    assert_eq!(budget.snapshot().total(), 0);
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"states":states,"final_charge":budget.snapshot().total()})
        )
        .unwrap()
    );
}
