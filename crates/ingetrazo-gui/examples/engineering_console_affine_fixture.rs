use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1])).unwrap();
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-affine-placement/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    for recipe in recipes.as_array().unwrap() {
        let code = recipe["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{:?}",
                app.console_logs
            );
        }
        if let Some(redo) = recipe["history"].as_bool() {
            if redo {
                app.redo()
            } else {
                app.undo()
            }
        }
        states.push(serde_json::json!({"name":recipe["name"],"aliases":app.scene.groups.iter().map(|g|app.scene.groups.iter().map(|h|g.mesh_share==h.mesh_share).collect::<Vec<_>>()).collect::<Vec<_>>(),"transforms":app.scene.groups.iter().map(|g|g.transform.to_cols_array()).collect::<Vec<_>>(),"names":app.scene.groups.iter().map(|g|&g.name).collect::<Vec<_>>(),"components":app.scene.groups.iter().map(|g|g.component_definition.is_some()).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()}));
    }
    ingetrazo_formats::igz::save_igz(&app.scene, std::path::Path::new(&args[2])).unwrap();
    let mut refusals = vec![];
    for code in [
        "groups[0].xform.scale(float('nan'))",
        "groups[0].xform=type(groups[0].xform)(1,0,0,0,0,1,0,0,0,0,1,0,.125,0,0,1)",
        "groups[0].xform=object()",
    ] {
        let before = app.scene.clone();
        let undo = app.undo_stack.clone();
        let redo = app.redo_stack.clone();
        app.run_script(code);
        assert!(
            app.console_logs.last().unwrap().contains("refused"),
            "{:?}",
            app.console_logs
        );
        assert_eq!(app.scene, before);
        assert_eq!(app.undo_stack, undo);
        assert_eq!(app.redo_stack, redo);
        refusals.push(true);
    }
    println!(
        "{}",
        serde_json::json!({"states":states,"refusals":refusals})
    );
}
