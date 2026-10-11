use glam::{DVec2, DVec3};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::json;
fn main() {
    let p = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(p.join("original.json")).unwrap()).unwrap();
    let mut rows = vec![];
    for case in [
        "glyph",
        "dimension",
        "leader-miss",
        "hidden",
        "locked",
        "nested",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = ingetrazo_formats::igz::load_igz(&p.join("input.igz")).unwrap();
        if case == "nested" {
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
        let layer = a
            .scene
            .layers
            .iter_mut()
            .find(|l| l.name == "named")
            .unwrap();
        layer.visible = case != "hidden";
        layer.locked = case == "locked";
        a.scene.clear_selection();
        a.set_tool(ToolType::Move);
        let source = original["states"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["case"] == case)
            .unwrap();
        let screen = DVec2::new(
            source["screen"][0].as_f64().unwrap(),
            source["screen"][1].as_f64().unwrap(),
        );
        let hits = a.update_move_annotation_hits([800., 600.], screen).unwrap();
        let label = hits
            .label
            .and_then(|id| a.scene.text_labels.iter().position(|t| t.history_id == id));
        let dimension = hits
            .dimension
            .and_then(|id| a.scene.dimensions.iter().position(|d| d.id == id));
        let record = |a: &IngeTrazoApp, phase: &str| json!({"case":case,"phase":phase,"screen":screen.to_array(),"hit_label":label,"hit_dimension":dimension,"labels":a.scene.text_labels.iter().map(|t|t.offset.to_array()).collect::<Vec<_>>(),"dimensions":a.scene.dimensions.iter().map(|d|json!({"a":a.scene.dimension_points(d).0.to_array(),"b":a.scene.dimension_points(d).1.to_array(),"offset":d.offset.to_array()})).collect::<Vec<_>>(),"selection":a.scene.selection.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"active":a.active_tool.snap_anchor().is_some()});
        let ctx = |v| {
            ToolContext::new(
                v,
                ingetrazo_core::math::Ray::new(DVec3::new(0., 0., 10.), -DVec3::Z),
            )
        };
        let delta = DVec3::new(0.5, 0.25, 0.);
        rows.push(record(&a, "before"));
        a.dispatch_tool_click(&ctx(DVec3::ZERO));
        rows.push(record(&a, "grab"));
        a.dispatch_tool_move(&ctx(delta));
        rows.push(record(&a, "preview"));
        a.dispatch_tool_cancel();
        rows.push(record(&a, "cancel"));
        a.update_move_annotation_hits([800., 600.], screen).unwrap();
        a.dispatch_tool_click(&ctx(DVec3::ZERO));
        a.dispatch_tool_move(&ctx(delta));
        a.dispatch_tool_click(&ctx(delta));
        rows.push(record(&a, "commit"));
        a.undo();
        rows.push(record(&a, "undo"));
        a.redo();
        rows.push(record(&a, "redo"));
    }
    std::fs::write(
        p.join("native.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
}
