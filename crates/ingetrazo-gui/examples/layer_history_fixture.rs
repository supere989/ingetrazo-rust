use glam::DVec3;
use ingetrazo_core::{
    layers::{Layer, DEFAULT_LAYER},
    scene::Scene,
};
use ingetrazo_gui::app::IngeTrazoApp;
fn state(a: &IngeTrazoApp) -> serde_json::Value {
    serde_json::json!({"layers":a.scene.layers.iter().map(|l|(&l.name,l.visible,l.locked)).collect::<Vec<_>>(),"edge":a.scene.mesh.edges[0].layer.as_deref().unwrap_or(DEFAULT_LAYER),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
}
fn setup() -> IngeTrazoApp {
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = Scene::new();
    let v = a.scene.mesh.add_vertex(DVec3::ZERO);
    let w = a.scene.mesh.add_vertex(DVec3::X);
    a.scene.mesh.add_edge(v, w);
    a
}
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    std::fs::create_dir_all(&out).unwrap();
    std::env::set_var("XDG_CONFIG_HOME", out.join("config"));
    let mut rows = serde_json::Map::new();
    let mut a = setup();
    a.save_undo_state();
    a.scene.mesh.vertices[1].position.x += 1.;
    a.scene.bump_version();
    a.add_layer("Layer 1".into());
    let i = a.scene.layers.len() - 1;
    a.set_layer_visibility(i, false);
    a.set_layer_locked(i, true);
    let mut states = vec![state(&a)];
    a.undo();
    states.push(state(&a));
    a.redo();
    states.push(state(&a));
    rows.insert(
        "panel_state_across_model_history".into(),
        serde_json::json!(states),
    );
    let mut a = setup();
    a.scene.layers.push(Layer::new("Unused"));
    a.purge_unused_layers();
    let mut states = vec![state(&a)];
    a.undo();
    let i = a
        .scene
        .layers
        .iter()
        .position(|l| l.name == "Unused")
        .unwrap();
    a.set_layer_visibility(i, false);
    states.push(state(&a));
    a.redo();
    states.push(state(&a));
    a.undo();
    states.push(state(&a));
    rows.insert(
        "purge_held_object_visibility".into(),
        serde_json::json!(states),
    );
    let mut a = setup();
    a.scene
        .select(ingetrazo_core::scene::SelectionItem::Edge(0));
    a.assign_selection_to_named_layer("Assigned");
    let i = a
        .scene
        .layers
        .iter()
        .position(|l| l.name == "Assigned")
        .unwrap();
    a.rename_layer(i, "Renamed".into());
    let mut states = vec![state(&a)];
    a.undo();
    states.push(state(&a));
    a.redo();
    states.push(state(&a));
    rows.insert(
        "assign_created_rename_lookup".into(),
        serde_json::json!(states),
    );
    let mut a = setup();
    a.save_undo_state();
    a.scene.layers.push(Layer::new("Imported"));
    a.scene.bump_version();
    let i = a.scene.layers.len() - 1;
    a.rename_layer(i, "Renamed import".into());
    a.delete_layer(i);
    let mut states = vec![state(&a)];
    a.undo();
    states.push(state(&a));
    a.redo();
    states.push(state(&a));
    rows.insert(
        "import_deleted_held_object".into(),
        serde_json::json!(states),
    );
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
