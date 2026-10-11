use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let mut app = IngeTrazoApp::new();
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-recipes/recipes.json"
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
    drop(app);
    for budget in budgets {
        assert_eq!(budget.snapshot().total(), 0);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"states":states})).unwrap()
    );
}
