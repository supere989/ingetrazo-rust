use glam::{DVec2, DVec3};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::json;
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("original.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for kind in ["label", "dimension"] {
        for context in ["root", "nested"] {
            let case = format!("{kind}-{context}");
            let mut a = IngeTrazoApp::new();
            a.system_clipboard = None;
            a.scene = ingetrazo_formats::igz::load_igz(&dir.join("input.igz")).unwrap();
            a.scene.native_document["dimension_style"]["vendor"] =
                json!({"opaque":[1,{"keep":true}]});
            if context == "nested" {
                a.scene
                    .select(ingetrazo_core::scene::SelectionItem::Group(0));
                a.begin_component_edit();
                a.scene
                    .select(ingetrazo_core::scene::SelectionItem::Group(0));
                a.begin_component_edit();
            }
            a.camera.target = DVec3::ZERO;
            a.camera.distance = 10.;
            a.camera.yaw = 0.;
            a.camera.pitch = std::f64::consts::FRAC_PI_2;
            a.camera.perspective = false;
            a.camera.aspect = 800. / 600.;
            a.set_tool(ToolType::Select);
            let s = source["states"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["case"] == case)
                .unwrap();
            let screen = DVec2::new(
                s["screen"][0].as_f64().unwrap(),
                s["screen"][1].as_f64().unwrap(),
            );
            let record = |a: &IngeTrazoApp, phase: &str| json!({"case":case,"phase":phase,"screen":screen.to_array(),"labels":a.scene.text_labels.iter().map(|t|t.text.clone()).collect::<Vec<_>>(),"dimensions":a.scene.dimensions.iter().map(|d|d.text_override.clone()).collect::<Vec<_>>(),"style":a.scene.dimension_style(),"value":a.formatted_dimension_value(&a.scene.dimensions[0]).unwrap(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"selection":a.scene.selection.len(),"depth":a.component_edit_stack.len()});
            let answer = |a: &mut IngeTrazoApp, text: &str, ok: bool| {
                a.update_move_annotation_hits([800., 600.], screen).unwrap();
                let mut ctx = ToolContext::new(
                    DVec3::ZERO,
                    ingetrazo_core::math::Ray::new(DVec3::Z * 10., -DVec3::Z),
                );
                ctx.screen = Some(screen);
                a.dispatch_tool_double_click(&ctx);
                assert!(
                    a.annotation_editor.is_some(),
                    "{case} dialog did not open: {}",
                    a.status_message
                );
                if ok {
                    a.annotation_editor.as_mut().unwrap().text = text.into();
                    a.accept_annotation_edit().unwrap();
                } else {
                    a.cancel_annotation_edit();
                }
            };
            rows.push(record(&a, "before"));
            answer(&mut a, " ignored ", false);
            rows.push(record(&a, "cancel"));
            answer(&mut a, "  authored\nvalue  ", true);
            rows.push(record(&a, "edit"));
            answer(&mut a, "authored\nvalue", true);
            rows.push(record(&a, "noop"));
            let mut style = a.dimension_style_controls();
            style.decimals = 3;
            style.units = "cm".into();
            style.font_size = 17;
            style.ends = "tick".into();
            style.base_step_mm = 12.5;
            style.color = [17, 42, 99];
            a.apply_dimension_style(style).unwrap();
            rows.push(record(&a, "style"));
            a.undo();
            rows.push(record(&a, "undo"));
            a.redo();
            rows.push(record(&a, "redo"));
            let automatic = a.formatted_dimension_value(&a.scene.dimensions[0]).unwrap();
            answer(
                &mut a,
                if kind == "dimension" {
                    &automatic
                } else {
                    "   "
                },
                true,
            );
            rows.push(record(&a, "automatic-or-empty"));
            a.undo();
            rows.push(record(&a, "undo-last"));
            a.redo();
            rows.push(record(&a, "redo-last"));
            if context == "nested" {
                a.finish_component_edit();
                a.finish_component_edit();
            }
            rows.push(record(&a, "finish"));
            ingetrazo_formats::igz::save_igz(&a.scene, &dir.join(format!("native-{case}.igz")))
                .unwrap();
        }
    }
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
}
