use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let mut app = IngeTrazoApp::new();
    let args: Vec<String> = std::env::args().collect();
    app.scene = ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1])).unwrap();
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-imported-mesh/recipes.json"
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
        let state = serde_json::json!({"packed_meshes":app.engineering_python_group_pack_count(),"aliases":app.scene.groups.iter().map(|g|app.scene.groups.iter().map(|h|g.mesh_share==h.mesh_share).collect::<Vec<_>>()).collect::<Vec<_>>(),"name":s["name"],"groups":app.scene.groups.iter().map(|g|serde_json::json!({"name":g.name,"component":g.component_definition.is_some(),"placed":g.native_document.get("xform").is_some_and(|v|!v.is_null()),"positions":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"faces":g.mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"holes":f.holes,"attrs":f.attrs})).collect::<Vec<_>>(),"edges":g.mesh.edges.iter().map(|e|serde_json::json!({"vertices":[e.v0,e.v1],"soft":e.soft})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()});
        let check = s["check"].as_str().unwrap();
        if !check.is_empty() {
            app.run_script(check);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{:?}",
                app.console_logs
            );
        }
        if s["name"] == "reassignment" {
            ingetrazo_formats::igz::save_igz(
                &app.scene,
                std::path::Path::new(&format!("{}.replacement", args[2])),
            )
            .unwrap();
        }
        states.push(state);
        if let Some(budget) = app.engineering_python_budget() {
            budgets.push(budget);
        }
    }
    ingetrazo_formats::igz::save_igz(&app.scene, std::path::Path::new(&args[2])).unwrap();
    drop(app);
    for budget in budgets {
        assert_eq!(budget.snapshot().total(), 0);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({"states":states})).unwrap()
    );
}
