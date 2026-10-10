use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("fixture dir"));
    let original: Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut cases = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}.igz"))).unwrap();
        let mut states = vec![app.ai_query_model()];
        let mut history = Vec::new();
        for step in case["steps"].as_array().unwrap() {
            match step.as_str().unwrap() {
                "enter-root" | "enter-child" => {
                    app.scene.selection.clear();
                    app.scene.selection.insert(SelectionItem::Group(0));
                    app.begin_component_edit();
                }
                "leave" => app.finish_component_edit(),
                "add-edge" => {
                    app.save_undo_state();
                    let a = app
                        .scene
                        .mesh
                        .add_vertex(glam::DVec3::new(101., 102., 103.));
                    let b = app
                        .scene
                        .mesh
                        .add_vertex(glam::DVec3::new(104., 105., 106.));
                    app.scene.mesh.add_edge(a, b).unwrap();
                    app.scene.bump_version();
                }
                "undo" | "redo" => {
                    let reply = app.ai_bridge_request(&json!({"id":1,"tool":step}));
                    assert_eq!(reply["ok"], true);
                    history.push(json!({"tool":step,"result":reply["result"]}));
                }
                _ => unreachable!(),
            };
            states.push(app.ai_query_model());
        }
        cases.push(json!({"name":name,"steps":case["steps"],"states":states,"history":history}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"source_commit":original["source_commit"],"cases":cases})
        )
        .unwrap()
    );
}
