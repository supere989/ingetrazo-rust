//! Actual app console and application history; one shared source recipe corpus.
use ingetrazo_gui::IngeTrazoApp;
fn main() {
    let disable_owner_marker = std::env::args().any(|arg| arg == "--missing-history-owner");
    let mut app = IngeTrazoApp::new();
    let ids = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| app.scene.mesh.add_vertex(p))
        .collect();
    app.scene.mesh.add_face(ids, vec![]).unwrap();
    app.engineering_python_enabled = true;
    let recipes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/audits/ai-console-history/recipes.json"
    ))
    .unwrap();
    let mut states = vec![];
    for step in recipes.as_array().unwrap() {
        let code = step["code"].as_str().unwrap();
        if !code.is_empty() {
            app.run_script(code);
            assert!(!app.console_logs.last().unwrap().contains("refused"));
        }
        if let Some(redo) = step["history"].as_bool() {
            // Engineering regression control: literal pre-routing behavior swaps the
            // application snapshot without stepping its interpreter owner.
            if disable_owner_marker {
                for snapshot in &mut app.undo_stack {
                    snapshot.history_python = None;
                }
                for snapshot in &mut app.redo_stack {
                    snapshot.history_python = None;
                }
            }
            if redo {
                app.redo()
            } else {
                app.undo()
            }
        }
        let check = step["check"].as_str().unwrap();
        if !check.is_empty() {
            app.run_script(check);
            assert!(!app.console_logs.last().unwrap().contains("refused"));
        }
        states.push(serde_json::json!({"name":step["name"],"positions":app.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"attrs":app.scene.mesh.faces.iter().map(|f|&f.attrs).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"identity_checked":!check.is_empty()}));
    }
    let observation = app.engineering_python_history_observation().unwrap();
    assert_eq!(observation.0, (2, 0));
    assert_eq!(observation.1, 3);
    assert_eq!(observation.2, 7);
    let budget = app.engineering_python_budget().unwrap();
    drop(app);
    assert_eq!(budget.snapshot().total(), 0);
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"states":states,"metrics":{"native_exports":observation.1,"vm_history_steps":observation.2,"final_owner_charge":budget.snapshot().total()}})).unwrap());
}
