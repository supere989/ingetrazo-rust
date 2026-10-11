use glam::DVec3;
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::json;
fn context(p: DVec3, ctrl: bool) -> ToolContext {
    ToolContext::with_modifiers(
        p,
        ingetrazo_core::math::Ray::new(DVec3::new(50., 50., 50.), -DVec3::Z),
        false,
        ctrl,
        false,
    )
}
fn record(a: &IngeTrazoApp, case: &str, phase: &str, held: &[u64]) -> serde_json::Value {
    json!({"case":case,"phase":phase,"dimensions":a.scene.dimensions.iter().map(|d|json!({"a":a.scene.dimension_points(d).0.to_array(),"b":a.scene.dimension_points(d).1.to_array(),"offset":d.offset.to_array()})).collect::<Vec<_>>(),"labels":a.scene.text_labels.iter().map(|t|json!({"anchor":t.anchor.to_array(),"offset":t.offset.to_array(),"text":t.text})).collect::<Vec<_>>(),"identity":a.scene.text_labels.iter().map(|t|held.iter().position(|id|*id==t.history_id).unwrap()).collect::<Vec<_>>(),"vertices":a.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"selection":a.scene.selection.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"active":a.active_tool.snap_anchor().is_some()})
}
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut rows = vec![];
    for case in [
        "free", "line", "partial", "riding", "mixed", "nested", "guide", "copy", "parked", "one-riding",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&p.join(if case=="one-riding"{"one-riding-input.igz"}else{"move-input.igz"})).unwrap();
        let held = a
            .scene
            .text_labels
            .iter()
            .map(|t| t.history_id)
            .collect::<Vec<_>>();
        if case == "nested" {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.clear_selection();
        if case == "guide" {
            a.scene.select(SelectionItem::Guide(0));
        } else {
            a.scene.select(SelectionItem::TextLabel(0));
            a.scene.select(SelectionItem::Dimension(match case {
                "line" | "riding" | "one-riding" => 1,
                "partial" => 2,
                _ => 0,
            }));
            if matches!(case, "mixed" | "riding" | "one-riding") {
                a.scene.select(SelectionItem::Edge(12));
            }
        }
        a.set_tool(ToolType::Move);
        rows.push(record(&a, case, "before", &held));
        a.dispatch_tool_click(&context(DVec3::ZERO, false));
        rows.push(record(&a, case, "grab", &held));
        let copy = case == "copy";
        let delta = if matches!(case, "mixed" | "riding" | "one-riding") {
            DVec3::new(1., 2., 0.)
        } else {
            DVec3::new(0.7, 0.8, 0.9)
        };
        a.dispatch_tool_move(&context(delta, copy));
        rows.push(record(&a, case, "preview", &held));
        a.dispatch_tool_cancel();
        rows.push(record(&a, case, "cancel", &held));
        a.dispatch_tool_move(&context(DVec3::ZERO, false));
        a.dispatch_tool_click(&context(DVec3::ZERO, false));
        a.dispatch_tool_move(&context(delta, copy));
        a.dispatch_tool_click(&context(delta, copy));
        rows.push(record(&a, case, "commit", &held));
        if case == "parked" {
            a.scene.clear_selection();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            rows.push(record(&a, case, "entry", &held));
        }
        a.undo();
        rows.push(record(&a, case, "undo", &held));
        a.redo();
        rows.push(record(&a, case, "redo", &held));
        if matches!(case, "nested" | "parked") {
            while !a.component_edit_stack.is_empty() {
                a.finish_component_edit();
            }
            rows.push(record(&a, case, "finish", &held));
            a.undo();
            rows.push(record(&a, case, "finish-undo", &held));
            a.redo();
            rows.push(record(&a, case, "finish-redo", &held));
        }
    }
    let mut boundaries = vec![];
    for kind in ["oblique", "collapsed"] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&p.join("move-input.igz")).unwrap();
        if kind == "oblique" {
            let source = ingetrazo_formats::igz::load_igz(&p.join("input.igz")).unwrap();
            a.scene.groups = source.groups;
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.scene.clear_selection();
        a.scene.select(SelectionItem::TextLabel(0));
        a.scene.select(SelectionItem::Dimension(0));
        if kind == "collapsed" {
            a.scene.select(SelectionItem::Edge(0));
        }
        a.set_tool(ToolType::Move);
        let held = a
            .scene
            .text_labels
            .iter()
            .map(|t| t.history_id)
            .collect::<Vec<_>>();
        let mut baseline = a.scene.clone();
        let depth = a.undo_stack.len();
        let mut observations = vec![];
        observations.push(record(&a, kind, "before", &held));
        a.dispatch_tool_click(&context(DVec3::ZERO, false));
        a.dispatch_tool_move(&context(
            if kind == "oblique" {
                DVec3::new(0.7, 0.8, 0.9)
            } else {
                DVec3::new(1., 2., 0.)
            },
            false,
        ));
        a.dispatch_tool_cancel();
        baseline = a.scene.clone();
        a.dispatch_tool_click(&context(DVec3::ZERO, false));
        let delta = if kind == "oblique" {
            DVec3::new(0.7, 0.8, 0.9)
        } else {
            DVec3::new(1., 2., 0.)
        };
        a.dispatch_tool_move(&context(delta, false));
        observations.push(record(&a, kind, "preview", &held));
        a.dispatch_tool_click(&context(delta, false));
        observations.push(record(&a, kind, "commit", &held));
        if kind == "collapsed" {
            assert_eq!(a.scene.mesh, baseline.mesh);
            assert_eq!(a.scene.dimensions, baseline.dimensions);
            assert_eq!(a.scene.text_labels, baseline.text_labels);
            assert_eq!(a.undo_stack.len(), depth);
            assert!(!a.status_message.is_empty());
        } else {
            assert_eq!(a.scene.mesh, baseline.mesh);
            a.undo();
            observations.push(record(&a, kind, "undo", &held));
            a.redo();
            observations.push(record(&a, kind, "redo", &held));
            while !a.component_edit_stack.is_empty() {
                a.finish_component_edit();
            }
        }
        boundaries.push(json!({"kind":kind,"geometry_preserved":a.scene.mesh==baseline.mesh,"observations":observations}));
    }
    std::fs::write(
        p.join("boundaries.json"),
        serde_json::to_vec_pretty(&boundaries).unwrap(),
    )
    .unwrap();
    std::fs::write(
        p.join("native.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
}
