//! Source-derived actual app console topology/paint/pose publication.
fn main() {
    let mut app = ingetrazo_gui::IngeTrazoApp::new();
    let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| app.scene.mesh.add_vertex(p))
        .collect();
    app.scene.mesh.add_face(ids, vec![]).unwrap();
    app.scene.mesh.faces[0].attrs=serde_json::from_value(serde_json::json!({"color":[0.2,0.4,0.8],"layer":"Imported","vendor":{"unit":[1,2],"name":"kept"}})).unwrap();
    app.scene.mesh.faces[0].layer = Some("Imported".into());
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-rows/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    let mut publications = vec![];
    for step in recipes.as_array().unwrap() {
        let before = app.engineering_python_publication_observation().0;
        let code = step["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{}",
                app.status_message
            );
        }
        if let Some(redo) = step["history"].as_bool() {
            if redo {
                app.redo()
            } else {
                app.undo()
            }
        }
        let after = app.engineering_python_publication_observation();
        if after.0 != before {
            publications.push(serde_json::json!({"name":step["name"],"metrics":after.1.unwrap()}));
        }
        states.push(serde_json::json!({"name":step["name"],"positions":app.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":app.scene.mesh.edges.iter().map(|e|serde_json::json!({"v0":e.v0,"v1":e.v1,"faces":e.faces})).collect::<Vec<_>>(),"faces":app.scene.mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices,"holes":f.holes,"attrs":f.attrs,"layer":f.layer})).collect::<Vec<_>>(),"layers":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"identity_checked":!step["check"].as_str().unwrap().is_empty()}));
        let before_check = app.engineering_python_publication_observation().0;
        let check = step["check"].as_str().unwrap();
        if !check.is_empty() {
            app.run_script(check);
            assert!(
                !app.console_logs.last().unwrap().contains("refused"),
                "{}",
                app.status_message
            );
        }

        assert_eq!(
            app.engineering_python_publication_observation().0,
            before_check,
            "Inspection unexpectedly published {}",
            step["name"]
        );
    }
    let (_, exports, history_steps) = app.engineering_python_history_observation().unwrap();
    let row_publications = app.engineering_python_publication_observation().0;
    let budget = app.engineering_python_budget().unwrap();
    drop(app);
    assert_eq!(budget.snapshot().total(), 0);
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"states":states,"publications":publications,"native_exports":exports,"row_publications":row_publications,"vm_history_steps":history_steps,"final_owner_charge":budget.snapshot().total()})).unwrap());
}
