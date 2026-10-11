use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let mut app = IngeTrazoApp::new();
    app.engineering_python_enabled = true;
    let unrelated = uuid::Uuid::from_u128(7);
    app.scene.mesh_shares.insert(unrelated, Default::default());
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-house/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    let mut budgets = vec![];
    for s in recipes.as_array().unwrap() {
        let code = s["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert_eq!(
                app.console_logs.last().unwrap().contains("refused"),
                s["expect_error"].as_bool().unwrap_or(false),
                "{:?}",
                app.console_logs
            );
        }
        if let Some(redo) = s["history"].as_bool() {
            if redo { app.redo() } else { app.undo() }
        }
        let state = serde_json::json!({"name":s["name"],"groups":app.scene.groups.iter().map(|g|serde_json::json!({"name":g.name,"positions":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":g.mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"holes":f.holes,"attrs":f.attrs})).collect::<Vec<_>>(),"edges":g.mesh.edges.iter().map(|e|serde_json::json!({"vertices":[e.v0,e.v1],"soft":e.soft})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()});
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
        if let Some(budget) = app.engineering_python_budget() {
            budgets.push(budget);
        }
    }
    let a = &app.scene.groups[0];
    let b = app.scene.groups.last().unwrap();
    assert_eq!(a.mesh_share, b.mesh_share);
    assert_eq!(
        app.scene.mesh_shares.get(&a.mesh_share.unwrap()),
        Some(&a.mesh)
    );
    assert!(app.scene.mesh_shares.contains_key(&unrelated));
    let path = std::env::args().nth(1).expect("output IGZ path");
    ingetrazo_formats::igz::save_igz(&app.scene, std::path::Path::new(&path)).unwrap();
    // Literal old publication boundary: group tokens exist but registry is absent.
    let mut unpublished = app.scene.clone();
    unpublished.mesh_shares.clear();
    ingetrazo_formats::igz::save_igz(
        &unpublished,
        std::path::Path::new(&format!("{path}.unpublished")),
    )
    .unwrap();
    drop(app);
    for budget in budgets {
        assert_eq!(budget.snapshot().total(), 0);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"states":states})).unwrap()
    );
}
