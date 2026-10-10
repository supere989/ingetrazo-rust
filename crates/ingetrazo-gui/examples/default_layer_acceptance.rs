use ingetrazo_core::{groups::Group, layers::Layer, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn state(app: &IngeTrazoApp) -> Value {
    json!({"names":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"default_visible":app.scene.is_layer_visible(None)})
}
fn assignment(app: &IngeTrazoApp) -> Value {
    json!({"layers":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"face":app.scene.mesh.faces[0].attrs.get("layer"),"edge":app.scene.mesh.edges[0].layer,"group":app.scene.groups[0].layer})
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let new_scene = state(&IngeTrazoApp::new());
    let mut cases = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}.igz"))).unwrap();
        ingetrazo_formats::igz::save_igz(&app.scene, &dir.join(format!("native-{name}.igz")))
            .unwrap();
        let mut s = state(&app);
        s["name"] = name.into();
        cases.push(s);
    }
    let mut app = IngeTrazoApp::new();
    app.scene
        .layers
        .extend([Layer::new("Layer0"), Layer::new("Untagged")]);
    let vertices = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y]
        .into_iter()
        .map(|p| app.scene.mesh.add_vertex(p))
        .collect();
    app.scene.mesh.add_face(vertices, vec![]).unwrap();
    app.scene.groups.push(Group::new("tagged"));
    app.scene.selection.extend([
        SelectionItem::Face(0),
        SelectionItem::Edge(0),
        SelectionItem::Group(0),
    ]);
    let mut steps = vec![assignment(&app)];
    ingetrazo_formats::igz::save_igz(&app.scene, &dir.join("assignment-0.igz")).unwrap();
    app.assign_selection_to_named_layer("Layer0");
    steps.push(assignment(&app));
    ingetrazo_formats::igz::save_igz(
        &app.scene,
        &dir.join(format!("assignment-{}.igz", steps.len() - 1)),
    )
    .unwrap();
    app.assign_selection_to_named_layer("Layer 0");
    steps.push(assignment(&app));
    ingetrazo_formats::igz::save_igz(
        &app.scene,
        &dir.join(format!("assignment-{}.igz", steps.len() - 1)),
    )
    .unwrap();
    app.undo();
    steps.push(assignment(&app));
    ingetrazo_formats::igz::save_igz(
        &app.scene,
        &dir.join(format!("assignment-{}.igz", steps.len() - 1)),
    )
    .unwrap();
    app.redo();
    steps.push(assignment(&app));
    ingetrazo_formats::igz::save_igz(
        &app.scene,
        &dir.join(format!("assignment-{}.igz", steps.len() - 1)),
    )
    .unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"new_scene":new_scene,"cases":cases,"assignment":steps})
        )
        .unwrap()
    );
}
