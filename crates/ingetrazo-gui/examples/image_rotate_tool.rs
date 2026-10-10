use glam::DVec3 as V;
use ingetrazo_core::{image_plane::ImagePlane, layers::Layer, math::Ray, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
fn ctx(p: V) -> ToolContext {
    ToolContext::new(p, Ray::new(V::new(1.375, -0.375, 10.), -V::Z))
}
fn state(a: &IngeTrazoApp) -> serde_json::Value {
    let p = &a.scene.image_planes[0];
    serde_json::json!({"frame":[p.origin,p.u,p.v],"active":a.active_tool.snap_anchor().is_some(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()})
}
fn main() {
    let mut rows = serde_json::Map::new();
    for case in [
        "selected",
        "implicit",
        "locked_implicit",
        "locked_selected",
        "hidden_selected",
        "cancel",
        "copy",
        "fractional_vcb",
        "lazy_snapshot",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let mut p = ImagePlane::new(
            "missing",
            V::new(0.125, -1.25, 2.5),
            V::new(2., 0.25, 0.),
            V::new(0.5, 1.5, 0.),
        );
        p.locked = case.starts_with("locked");
        if case == "hidden_selected" {
            p.layer = Some("Hidden".into());
            let mut layer = Layer::new("Hidden");
            layer.visible = false;
            a.scene.layers.push(layer);
        }
        a.scene.image_planes.push(p);
        if !["implicit", "locked_implicit"].contains(&case) {
            a.scene.select(SelectionItem::ImagePlane(0));
        }
        a.set_tool(ToolType::Rotate);
        if std::env::args().any(|v| v == "--zero-axis-control") {
            a.active_tool = Box::new(ingetrazo_tools::RotateTool::with_normal(V::ZERO));
        }
        let mut states = vec![state(&a)];
        a.dispatch_tool_click(&ctx(V::ZERO));
        states.push(state(&a));
        if case == "lazy_snapshot" {
            let origin = (a.scene.image_planes[0].origin.as_vec3()
                + glam::Vec3::new(0.7, 1.2, -0.3))
            .as_dvec3();
            a.scene.image_planes[0].origin = origin;
            states.push(state(&a));
        }
        a.dispatch_tool_click(&ctx(V::X));
        a.dispatch_tool_move(&ctx(V::Y));
        states.push(state(&a));
        a.dispatch_tool_move(&ctx(V::X + V::Y));
        states.push(state(&a));
        if case == "copy" {
            a.active_tool.on_control_state(&mut a.scene, true);
            states.push(state(&a));
        }
        if case == "cancel" {
            a.active_tool.on_cancel(&mut a.scene);
            states.push(state(&a));
        } else {
            if case == "fractional_vcb" {
                a.dispatch_tool_vcb("33.7").unwrap();
            } else {
                let mut c = ctx(V::Y);
                c.ctrl_held = case == "copy";
                a.dispatch_tool_click(&c);
            }
            states.push(state(&a));
            if !a.undo_stack.is_empty() {
                a.undo();
                states.push(state(&a));
                a.redo();
                states.push(state(&a));
            }
        }
        rows.insert(case.into(), serde_json::json!(states));
    }
    println!("{}", serde_json::to_string_pretty(&rows).unwrap());
}
