use glam::DVec3;
use ingetrazo_core::{math::Ray, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{
    pick_scene_edge, pick_scene_face, pick_scene_group, ToolContext, ToolType,
};
use serde_json::{json, Value};
fn vector(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn selection(a: &IngeTrazoApp) -> Vec<&'static str> {
    let mut s = a
        .scene
        .selection
        .iter()
        .map(|s| match s {
            SelectionItem::Face(_) => "face",
            SelectionItem::Edge(_) => "edge",
            SelectionItem::Group(_) => "group",
            _ => "other",
        })
        .collect::<Vec<_>>();
    s.sort();
    s
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let out = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for cases in original["rows"].as_array().unwrap().chunks(4) {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(
            &dir.join(format!("{}.igz", cases[0]["name"].as_str().unwrap())),
        )
        .unwrap();
        let name = cases[0]["name"].as_str().unwrap();
        let target = if name.contains("default") || name.contains("empty") {
            0
        } else {
            1
        };
        for (i, c) in cases.iter().enumerate() {
            if i == 1 {
                a.scene
                    .selection
                    .extend((0..a.scene.mesh.faces.len()).map(SelectionItem::Face));
                a.scene
                    .selection
                    .extend((0..a.scene.mesh.edges.len()).map(SelectionItem::Edge));
                a.scene
                    .selection
                    .extend((0..a.scene.groups.len()).map(SelectionItem::Group));
                a.set_layer_locked(target, true);
            }
            if i == 2 {
                a.set_layer_locked(target, false);
                a.set_layer_visibility(target, false);
            }
            if i == 3 {
                a.set_layer_visibility(target, true);
            }
            let ray = Ray::new(vector(&c["ray"][0]), vector(&c["ray"][1]));
            rows.push(json!({"name":c["name"],"face":pick_scene_face(&a.scene,&ray).map(|v|v.0),"edge":pick_scene_edge(&a.scene,&ray,0.15),"group":pick_scene_group(&a.scene,&ray,0.15).map(|v|v.0),"selection":selection(&a),"visible_faces":(0..a.scene.mesh.faces.len()).map(|i|a.scene.mesh_face_visible(&a.scene.mesh,i)).collect::<Vec<_>>()}));
        }
    }
    // Actual application dispatch: locked targets cannot start Push/Pull or add history;
    // unlocking admits the same gesture, followed by Undo/Redo and preview Cancel.
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = ingetrazo_formats::igz::load_igz(&dir.join("face-default-open.igz")).unwrap();
    let c = &original["rows"][0];
    let context = ToolContext::new(
        vector(&c["point"]),
        Ray::new(vector(&c["ray"][0]), vector(&c["ray"][1])),
    );
    a.set_tool(ToolType::Select);
    a.dispatch_tool_click(&context);
    assert!(a.scene.selection.contains(&SelectionItem::Face(0)));
    a.set_layer_locked(0, true);
    assert!(a.scene.selection.is_empty());
    a.dispatch_tool_click(&context);
    assert!(a.scene.selection.is_empty());
    a.set_tool(ToolType::PushPull);
    let before = serde_json::to_value(&a.scene.mesh).unwrap();
    let history = a.undo_stack.len();
    a.dispatch_tool_click(&context);
    a.dispatch_tool_vcb("1").ok();
    assert_eq!(serde_json::to_value(&a.scene.mesh).unwrap(), before);
    assert_eq!(a.undo_stack.len(), history);
    a.dispatch_tool_cancel();
    a.set_layer_locked(0, false);
    a.dispatch_tool_click(&context);
    a.dispatch_tool_vcb("1").unwrap();
    assert_eq!(a.scene.mesh.faces.len(), 6);
    assert_eq!(a.undo_stack.len(), history + 1);
    a.undo();
    assert_eq!(serde_json::to_value(&a.scene.mesh).unwrap(), before);
    a.redo();
    assert_eq!(a.scene.mesh.faces.len(), 6);
    a.undo();
    a.dispatch_tool_click(&context);
    a.dispatch_tool_cancel();
    assert_eq!(serde_json::to_value(&a.scene.mesh).unwrap(), before);
    assert_eq!(a.undo_stack.len(), history);
    std::fs::write(out,serde_json::to_vec_pretty(&json!({"rows":rows,"consumer":{"locked_select":true,"locked_pushpull":true,"unlock_commit":true,"undo_redo":true,"cancel":true}})).unwrap()).unwrap();
}
