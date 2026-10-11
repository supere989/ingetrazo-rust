use glam::{DMat4, DVec2};
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::BoxSelection;
use serde_json::{json, Value};
fn item(v: &Value) -> SelectionItem {
    let i = v[1].as_u64().unwrap() as usize;
    match v[0].as_str().unwrap() {
        "edge" => SelectionItem::Edge(i),
        "face" => SelectionItem::Face(i),
        "group" => SelectionItem::Group(i),
        "guide" => SelectionItem::Guide(i),
        "dimension" => SelectionItem::Dimension(i),
        "text" => SelectionItem::TextLabel(i),
        _ => panic!("unknown kind"),
    }
}
fn selected(app: &IngeTrazoApp) -> Vec<Value> {
    let mut r = app
        .scene
        .selection
        .iter()
        .map(|s| match s {
            SelectionItem::Edge(i) => json!(["edge", i]),
            SelectionItem::Face(i) => json!(["face", i]),
            SelectionItem::Group(i) => json!(["group", i]),
            SelectionItem::Guide(i) => json!(["guide", i]),
            SelectionItem::Dimension(i) => json!(["dimension", i]),
            SelectionItem::TextLabel(i) => json!(["text", i]),
            _ => panic!("unexpected target"),
        })
        .collect::<Vec<_>>();
    r.sort_by_key(Value::to_string);
    r
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let source: Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let matrix = source["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect::<Vec<_>>();
    let mut rows = vec![];
    for c in source["cases"].as_array().unwrap() {
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.scene =
            ingetrazo_formats::igz::load_igz(&dir.join(c["input"].as_str().unwrap_or("input.igz")))
                .unwrap();
        if c["orphan"] == true {
            app.scene.groups[0].children[0]
                .mesh
                .add_vertex(glam::DVec3::new(100., 100., 0.));
        }
        if c["context"] == true {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            app.scene.clear_selection();
        }
        for v in c["initial"].as_array().unwrap() {
            app.scene.select(item(v));
        }
        let named = app
            .scene
            .layers
            .iter_mut()
            .find(|l| l.name == "named")
            .unwrap();
        named.locked = c["policy"] == "locked";
        named.visible = c["policy"] != "hidden";
        let r = c["rect"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect::<Vec<_>>();
        let crossing = c["crossing"].as_bool().unwrap();
        let (start, end) = if crossing {
            (DVec2::new(r[2], r[1]), DVec2::new(r[0], r[3]))
        } else {
            (DVec2::new(r[0], r[1]), DVec2::new(r[2], r[3]))
        };
        let mode = c["mode"].as_str().unwrap();
        let selection = BoxSelection {
            viewport: ingetrazo_core::screen_pick::PixelViewport {
                matrix: DMat4::from_cols_array(matrix.as_slice().try_into().unwrap()),
                width: 800.,
                height: 600.,
            },
            eye: app.camera.eye(),
            start,
            end,
            shift: matches!(mode, "toggle" | "remove"),
            ctrl: matches!(mode, "add" | "remove"),
        };
        let before = app.scene.clone();
        let depth = app.undo_stack.len();
        app.dispatch_box_selection(&selection).unwrap();
        assert_eq!(app.undo_stack.len(), depth);
        assert!(app.redo_stack.is_empty());
        assert_eq!(app.scene.mesh, before.mesh);
        assert_eq!(app.scene.groups, before.groups);
        assert_eq!(app.scene.dimensions, before.dimensions);
        assert_eq!(app.scene.text_labels, before.text_labels);
        rows.push(json!({"name":c["name"],"selected":selected(&app)}));
    }
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.scene = ingetrazo_formats::igz::load_igz(&dir.join("guide-input.igz")).unwrap();
    assert_eq!(app.scene.guides.len(), 2);
    assert_eq!(
        app.scene.guides[0].origin,
        Some(glam::DVec3::new(1., 2., 3.))
    );
    app.scene.guides[0].kind =
        ingetrazo_core::guides::GuideType::Point(glam::DVec3::new(7., 8., 9.));
    app.save_to_path(&dir.join("guide-edited.igz"));
    assert!(dir.join("guide-edited.igz").exists());
    let raw: Value =
        serde_json::from_slice(&std::fs::read(dir.join("guide-edited.igz")).unwrap()).unwrap();
    assert_eq!(
        raw["scene"]["guides"][0]["vendor"],
        json!({"opaque":[false,17,"kept"]})
    );
    assert_eq!(raw["scene"]["guides"][0]["origin"], json!([1, 2, 3]));
    let reopen = ingetrazo_formats::igz::load_igz(&dir.join("guide-edited.igz")).unwrap();
    assert_eq!(reopen.guides[0].kind, app.scene.guides[0].kind);
    let mut fresh = IngeTrazoApp::new();
    fresh.system_clipboard = None;
    let mut point = ingetrazo_core::guides::Guide::point(glam::DVec3::new(4., 5., 6.));
    point.origin = Some(glam::DVec3::ZERO);
    fresh.scene.guides.push(point);
    fresh.save_to_path(&dir.join("guide-fresh.igz"));
    assert!(dir.join("guide-fresh.igz").exists());
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&json!({"cases":rows})).unwrap(),
    )
    .unwrap();
}
