use ingetrazo_core::{guides::GuideType, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn guides(a: &IngeTrazoApp) -> Vec<Value> {
    a.scene
        .guides
        .iter()
        .map(|g| {
            let mut v = match g.kind {
                GuideType::Point(p) => json!({"point":p.to_array()}),
                GuideType::InfiniteLine { point, dir } => {
                    json!({"point":point.to_array(),"direction":dir.to_array()})
                }
                _ => panic!("unexpected guide"),
            };
            if let Some(p) = g.origin {
                v["origin"] = json!(p.to_array());
            }
            v
        })
        .collect()
}
fn record(
    a: &IngeTrazoApp,
    case: &str,
    phase: &str,
    g: &[uuid::Uuid],
    d: &[uuid::Uuid],
    t: &[u64],
) -> Value {
    json!({"case":case,"phase":phase,"guides":guides(a),"dimensions":a.scene.dimensions.iter().map(|v|json!({"a":v.start.to_array(),"b":v.end.to_array(),"offset":v.offset.to_array(),"text":v.text_override.as_deref().unwrap_or(""),"axis":v.axis,"layer":v.layer})).collect::<Vec<_>>(),"labels":a.scene.text_labels,"identity":{"guides":a.scene.guides.iter().map(|v|g.iter().position(|id|*id==v.id).map(|i|i as i64).unwrap_or(-1)).collect::<Vec<_>>(),"dimensions":a.scene.dimensions.iter().map(|v|d.iter().position(|id|*id==v.id).map(|i|i as i64).unwrap_or(-1)).collect::<Vec<_>>(),"labels":a.scene.text_labels.iter().map(|v|t.iter().position(|id|*id==v.history_id).map(|i|i as i64).unwrap_or(-1)).collect::<Vec<_>>()},"selection":a.scene.selection.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"groups":a.scene.groups.len(),"faces":a.scene.mesh.faces.len()})
}
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut rows = Vec::new();
    for case in ["root", "nested-annotations", "nested-mixed", "parked"] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&dir.join("delete-input.igz")).unwrap();
        let g = a.scene.guides.iter().map(|v| v.id).collect::<Vec<_>>();
        let d = a.scene.dimensions.iter().map(|v| v.id).collect::<Vec<_>>();
        let t = a
            .scene
            .text_labels
            .iter()
            .map(|v| v.history_id)
            .collect::<Vec<_>>();
        if case.starts_with("nested-") {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.clear_selection();
        rows.push(record(&a, case, "before", &g, &d, &t));
        for target in [
            SelectionItem::Guide(0),
            SelectionItem::Guide(2),
            SelectionItem::Dimension(0),
            SelectionItem::TextLabel(1),
        ] {
            a.scene.select(target);
        }
        if case == "root" {
            a.scene.select(SelectionItem::Group(0));
        }
        if case == "nested-mixed" {
            a.scene.select(SelectionItem::Face(0));
        }
        a.delete_selection();
        rows.push(record(&a, case, "delete", &g, &d, &t));
        if case=="parked"{a.scene.select(SelectionItem::Group(0));a.begin_component_edit();a.scene.select(SelectionItem::Group(0));a.begin_component_edit();a.scene.clear_selection();rows.push(record(&a,case,"entry",&g,&d,&t));}
        a.undo();
        rows.push(record(&a, case, "undo", &g, &d, &t));
        a.redo();
        rows.push(record(&a, case, "redo", &g, &d, &t));
        if case != "root" {
            while !a.component_edit_stack.is_empty() {
                a.finish_component_edit();
            }
            rows.push(record(&a, case, "finish", &g, &d, &t));
            a.undo();
            rows.push(record(&a, case, "finish-undo", &g, &d, &t));
            a.redo();
            rows.push(record(&a, case, "finish-redo", &g, &d, &t));
        }
        a.save_to_path(&dir.join(format!("{case}-native.igz")));
        assert!(dir.join(format!("{case}-native.igz")).exists());
        rows.push(record(&a, case, "save", &g, &d, &t));
    }
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
    // Native Cancel restores its established entry checkpoint, not source Finish.
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = ingetrazo_formats::igz::load_igz(&dir.join("delete-input.igz")).unwrap();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    let before = a.scene.clone();
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Guide(0));
    a.delete_selection();
    a.cancel_component_edit();
    assert_eq!(a.scene.guides, before.guides);
    assert!(a.undo_stack.is_empty());
    println!("annotation delete consumer complete");
}
