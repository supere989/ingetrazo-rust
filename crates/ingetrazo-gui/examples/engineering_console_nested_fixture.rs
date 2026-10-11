use ingetrazo_core::groups::Group;
use ingetrazo_gui::IngeTrazoApp;
fn state(app: &IngeTrazoApp, name: &serde_json::Value) -> serde_json::Value {
    let mut rows = vec![];
    let mut tokens = vec![];
    fn walk(
        g: &Group,
        path: Vec<usize>,
        parent: glam::Mat4,
        rows: &mut Vec<serde_json::Value>,
        tokens: &mut Vec<Option<uuid::Uuid>>,
    ) {
        let matrix = glam::Mat4::from_cols_array(&g.transform.to_cols_array().map(|v| v as f32));
        let effective = parent * matrix;
        rows.push(serde_json::json!({"path":path,"name":g.name,"matrix":matrix.to_cols_array(),"effective":effective.to_cols_array(),"component":g.component_definition.is_some(),"vertices":g.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>()}));
        tokens.push(g.mesh_share);
        for (i, c) in g.children.iter().enumerate() {
            let mut next = path.clone();
            next.push(i);
            walk(c, next, effective, rows, tokens)
        }
    }
    for (i, g) in app.scene.groups.iter().enumerate() {
        walk(g, vec![i], glam::Mat4::IDENTITY, &mut rows, &mut tokens)
    }
    serde_json::json!({"name":name,"rows":rows,"aliases":tokens.iter().map(|a|tokens.iter().map(|b|a==b).collect::<Vec<_>>()).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()})
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut app = IngeTrazoApp::new();
    app.scene = ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1])).unwrap();
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-nested-scene/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    for r in recipes.as_array().unwrap() {
        let code = r["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{:?}",
                app.console_logs
            )
        }
        if let Some(redo) = r["history"].as_bool() {
            if redo {
                app.redo()
            } else {
                app.undo()
            }
        }
        states.push(state(&app, &r["name"]));
    }
    ingetrazo_formats::igz::save_igz(&app.scene, std::path::Path::new(&args[2])).unwrap();
    let mut refusals = vec![];
    for code in [
        "groups[0].children.append(groups[0])",
        "groups[0].children[0].xform.scale(float('nan'))",
        "groups[0].children.append(object())",
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
