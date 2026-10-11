use glam::{DVec2, DVec3};
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::json;
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}-input.igz"))).unwrap();
        a.scene.native_document["units"]["vendor"] = json!({"keep":[1,{"opaque":true}]});
        std::sync::Arc::make_mut(&mut a.scene.text_labels[0].native_document)["vendor"] =
            json!({"keep":["authored",7]});
        if name.starts_with("nested") {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
        }
        a.camera.target = DVec3::ZERO;
        a.camera.distance = 10.;
        a.camera.yaw = 0.;
        a.camera.pitch = std::f64::consts::FRAC_PI_2;
        a.camera.perspective = false;
        a.camera.aspect = 800. / 600.;
        a.set_tool(ToolType::Text);
        let p = case["point"].as_array().unwrap();
        let point = DVec3::new(
            p[0].as_f64().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64().unwrap(),
        );
        let screen = DVec2::new(
            case["screen"][0].as_f64().unwrap(),
            case["screen"][1].as_f64().unwrap(),
        );
        a.active_tool.note_pick_viewport(
            ingetrazo_core::screen_pick::PixelViewport::new(&a.camera, 800., 600.),
            a.camera.eye(),
            a.model_revision,
        );
        a.scene.clear_selection();
        if !a.scene.text_labels.is_empty() {
            a.scene.select(SelectionItem::TextLabel(0));
        }
        let context = |point| {
            let mut c = ToolContext::new(
                point,
                ingetrazo_core::math::Ray::new(point + DVec3::Z * 10., -DVec3::Z),
            );
            c.screen = Some(screen);
            c
        };
        let record = |a: &IngeTrazoApp, phase: &str| {
            let suggestion = a
                .active_tool
                .value_label()
                .map(|(s, _)| s)
                .unwrap_or_default();
            let mut selection = a
                .scene
                .selection
                .iter()
                .filter_map(|s| {
                    if let SelectionItem::TextLabel(i) = s {
                        Some(*i)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            selection.sort();
            json!({"case":name,"phase":phase,"labels":a.scene.text_labels.iter().map(|t|json!({"anchor":t.anchor.to_array(),"offset":t.offset.to_array(),"text":t.text,"layer":t.layer})).collect::<Vec<_>>(),"selection":selection,"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"depth":a.component_edit_stack.len(),"anchor":a.active_tool.snap_anchor().map(|v|v.to_array()),"suggestion":suggestion})
        };
        let acquire = |a: &mut IngeTrazoApp| {
            a.dispatch_tool_click(&context(point));
            a.dispatch_tool_move(&context(point + DVec3::new(1., 2., 0.)));
        };
        rows.push(record(&a, "before"));
        acquire(&mut a);
        rows.push(record(&a, "anchor"));
        a.dispatch_tool_click(&context(point + DVec3::new(1., 2., 0.)));
        assert!(a.annotation_editor.is_some(), "{}", a.status_message);
        rows.push(record(&a, "pending-cancel"));
        a.cancel_annotation_edit();
        rows.push(record(&a, "cancel"));
        acquire(&mut a);
        a.dispatch_tool_click(&context(point + DVec3::new(1., 2., 0.)));
        rows.push(record(&a, "pending-accept"));
        a.annotation_editor.as_mut().unwrap().text = "  authored\nlabel  ".into();
        a.accept_annotation_edit().unwrap();
        rows.push(record(&a, "commit"));
        a.undo();
        rows.push(record(&a, "undo"));
        a.redo();
        rows.push(record(&a, "redo"));
        acquire(&mut a);
        a.dispatch_tool_click(&context(point + DVec3::new(1., 2., 0.)));
        rows.push(record(&a, "pending-accept"));
        a.annotation_editor.as_mut().unwrap().text = "  ".into();
        assert!(!a.accept_annotation_edit().unwrap());
        rows.push(record(&a, "empty"));
        if name.starts_with("nested") {
            a.finish_component_edit();
            a.finish_component_edit();
        }
        rows.push(record(&a, "finish"));
        ingetrazo_formats::igz::save_igz(&a.scene, &dir.join(format!("{name}-native.igz")))
            .unwrap();
        let reopened =
            ingetrazo_formats::igz::load_igz(&dir.join(format!("{name}-native.igz"))).unwrap();
        assert_eq!(
            reopened.native_document["units"]["vendor"],
            json!({"keep":[1,{"opaque":true}]})
        );
        assert_eq!(
            reopened.native_document["text_labels"][0]["vendor"],
            json!({"keep":["authored",7]})
        );
    }
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = ingetrazo_formats::igz::load_igz(&dir.join("root-face-input.igz")).unwrap();
    a.set_tool(ToolType::Text);
    let context = |p| {
        let mut c = ToolContext::new(p, ingetrazo_core::math::Ray::new(p + DVec3::Z, -DVec3::Z));
        c.screen = Some(DVec2::splat(-100.));
        c
    };
    a.dispatch_tool_click(&context(DVec3::new(30., 20., 0.)));
    a.dispatch_tool_click(&context(DVec3::new(31., 22., 0.)));
    a.annotation_editor.as_mut().unwrap().text = "created root".into();
    a.accept_annotation_edit().unwrap();
    let record = |a: &IngeTrazoApp, phase: &str| {
        let mut labels = a
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::TextLabel(i) = s {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        labels.sort();
        let mut edges = a
            .scene
            .selection
            .iter()
            .filter_map(|s| {
                if let SelectionItem::Edge(i) = s {
                    Some(*i)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        edges.sort();
        json!({"phase":phase,"labels":a.scene.text_labels.iter().map(|t|json!({"anchor":t.anchor.to_array(),"offset":t.offset.to_array(),"text":t.text})).collect::<Vec<_>>(),"label_selection":labels,"edge_selection":edges,"vertices":a.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"depth":a.component_edit_stack.len()})
    };
    let mut history = vec![record(&a, "created")];
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    a.scene.clear_selection();
    a.scene.select(SelectionItem::Edge(0));
    a.scene.select(SelectionItem::TextLabel(0));
    a.scene
        .select(SelectionItem::TextLabel(a.scene.text_labels.len() - 1));
    history.push(record(&a, "entered-selected"));
    a.undo();
    history.push(record(&a, "undo"));
    a.redo();
    history.push(record(&a, "redo"));
    a.dispatch_tool_cancel();
    history.push(record(&a, "tool-cancel"));
    a.finish_component_edit();
    a.finish_component_edit();
    history.push(record(&a, "finish"));
    std::fs::write(
        dir.join("native-history.json"),
        serde_json::to_vec_pretty(&json!({"states":history})).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
}
