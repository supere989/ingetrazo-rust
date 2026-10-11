// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual guide producers and camera inference to Line/history.
use glam::{DVec2, DVec3};
use ingetrazo_core::{
    guides::{Guide, GuideType},
    math::Ray,
    scene::Scene,
    screen_pick::PixelViewport,
    snap::{SnapKind, SnapResult},
    snap_screen::{guide_snap_segment, source_camera_matrix, ScreenSnapContext, ScreenSnapPolicy},
};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
fn p(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn coord(v: DVec3) -> Vec<f64> {
    v.to_array()
        .into_iter()
        .map(|x| (x * 1e5).round() / 1e5)
        .collect()
}
fn guides(a: &IngeTrazoApp) -> Value {
    json!(a
        .scene
        .guides
        .iter()
        .map(|g| match g.kind {
            GuideType::Point(p) => json!({"point":p.to_array()}),
            GuideType::InfiniteLine { point, dir } =>
                json!({"point":point.to_array(),"direction":dir.to_array()}),
            _ => panic!("unexpected segment"),
        })
        .collect::<Vec<_>>())
}
fn state(a: &IngeTrazoApp) -> Value {
    let m = &a.scene.mesh;
    let mut vs: Vec<_> = m.vertices.iter().map(|v| coord(v.position)).collect();
    vs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut es: Vec<_> = m
        .edges
        .iter()
        .map(|e| {
            let mut x = vec![
                coord(m.vertices[e.v0].position),
                coord(m.vertices[e.v1].position),
            ];
            x.sort_by(|a, b| a.partial_cmp(b).unwrap());
            x
        })
        .collect();
    es.sort_by(|a, b| a.partial_cmp(b).unwrap());
    json!({"guides":guides(a),"vertices":vs,"edges":es,"history":[a.undo_stack.len(),a.redo_stack.len()]})
}
fn kind(k: SnapKind) -> &'static str {
    match k {
        SnapKind::Intersection => "intersection",
        SnapKind::Endpoint => "endpoint",
        SnapKind::OnEdge => "on_edge",
        SnapKind::Guide => "on_line",
        SnapKind::None => "none",
        SnapKind::Origin => "origin",
        _ => "other",
    }
}
fn make(r: &Value) -> IngeTrazoApp {
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = Scene::new();
    a.camera = serde_json::from_value(r["camera"].clone()).unwrap();
    for recipe in r["producer_points"].as_array().unwrap() {
        a.set_tool(ToolType::Protractor);
        for point in recipe.as_array().unwrap() {
            let point = p(point);
            a.dispatch_tool_click(&ToolContext::new(
                point,
                Ray::new(point + DVec3::Z * 10., -DVec3::Z),
            ));
        }
    }
    a
}
fn snap(a: &mut IngeTrazoApp, cursor: DVec2, candidate: DVec3) -> SnapResult {
    let vp = PixelViewport::new(&a.camera, 1000., 800.);
    let ctx = ScreenSnapContext {
        viewport: vp,
        cursor,
        candidate,
        plane_normal: DVec3::Z,
    };
    a.snap_engine.snap_screen_with_policy(
        &vp.ray(cursor).unwrap(),
        &a.scene,
        None,
        None,
        &ctx,
        &ScreenSnapPolicy {
            source_matrix: Some(source_camera_matrix(&a.camera)),
            ..Default::default()
        },
    )
}
fn egui(r: &Value) -> Value {
    fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, t: f64, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 800.),
                )),
                time: Some(t),
                events,
                ..Default::default()
            },
            |ctx| a.update_ui(ctx),
        );
    }
    let mut a = make(r);
    a.native_gpu_viewport_enabled = true;
    a.set_tool(ToolType::Line);
    let ctx = egui::Context::default();
    frame(&ctx, &mut a, 0., vec![]);
    frame(&ctx, &mut a, 0.1, vec![]);
    let rect = a.native_gpu_viewport_rect.unwrap();
    let mouse = rect.center();
    frame(&ctx, &mut a, 0.2, vec![egui::Event::PointerMoved(mouse)]);
    for (t, pressed) in [(0.3, true), (0.4, false)] {
        frame(
            &ctx,
            &mut a,
            t,
            vec![egui::Event::PointerButton {
                pos: mouse,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }],
        );
    }
    let anchor = a.active_tool.snap_anchor();
    let result = a.dispatch_tool_vcb("1;0;0");
    let after = state(&a);
    a.dispatch_tool_cancel();
    a.undo();
    let undo = state(&a);
    a.redo();
    json!({"anchor":anchor.map(coord),"accepted":result.is_ok(),"after":after,"undo":undo,"redo":state(&a)})
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let s: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    let mut actual = vec![];
    for r in s["cases"].as_array().unwrap() {
        let name = r["name"].as_str().unwrap();
        let mut a = make(r);
        let authored = guides(&a);
        let mut states = vec![json!({"phase":"guides","state":state(&a)})];
        if ["point-inside9", "point-exact9", "point-outside9"]
            .iter()
            .any(|label| name.ends_with(label))
        {
            a.scene.guides = vec![Guide::point(DVec3::new(3., 2., 1.))];
        } else if name.contains("-line-") {
            a.scene.guides.truncate(1);
        }
        a.set_tool(ToolType::Line);
        a.active_tool
            .note_cursor_plane(DVec3::new(3., 2., 1.), DVec3::Z);
        let matrix = source_camera_matrix(&a.camera);
        let vp = PixelViewport {
            matrix: matrix.as_dmat4(),
            width: 1000.,
            height: 800.,
        };
        let spans: Vec<_> =
            a.scene
                .guides
                .iter()
                .map(|g| match g.kind {
                    GuideType::InfiniteLine { point, dir } => guide_snap_segment(&vp, point, dir)
                        .map(|(x, y)| [x.to_array(), y.to_array()]),
                    _ => None,
                })
                .collect();
        let cursor = DVec2::new(
            r["cursor"][0].as_f64().unwrap(),
            r["cursor"][1].as_f64().unwrap(),
        );
        let hit = snap(&mut a, cursor, p(&r["candidate"]));
        let context_hit = snap(&mut a, DVec2::new(500., 400.), DVec3::new(3., 2., 1.));
        if name.ends_with("-cross") {
            let point = context_hit.point;
            a.dispatch_tool_click(&ToolContext::new(
                point,
                a.camera.screen_to_ray(500., 400., 1000., 800.),
            ));
            states.push(json!({"phase":"anchor","state":state(&a)}));
            a.dispatch_tool_vcb("1;0;0").unwrap();
            states.push(json!({"phase":"line","state":state(&a)}));
            a.dispatch_tool_cancel();
            a.undo();
            states.push(json!({"phase":"undo","state":state(&a)}));
            a.redo();
            states.push(json!({"phase":"redo","state":state(&a)}));
            actual.push(json!({"name":name,"result":egui(r)}));
        }
        rows.push(json!({"name":name,"authored":authored,"matrix":matrix.to_cols_array().map(f64::from),"baseline_matrix":(a.camera.projection_matrix().as_mat4()*a.camera.view_matrix().as_mat4()).to_cols_array().map(f64::from),"segments":spans,"source":{"kind":kind(hit.kind),"point":hit.point.to_array()},"production_context":{"kind":kind(context_hit.kind),"point":context_hit.point.to_array()},"states":states}));
    }
    let position = DVec3::new(1e12 + 0.125, 1e12 + 0.25, 1e12 + 0.5);
    let dir = DVec3::new(1.234567890123, 2.345678901234, 3.456789012345);
    let native = Guide::infinite_line(position, dir);
    assert!(
        matches!(native.kind,GuideType::InfiniteLine{point,dir:d}if point==position&&d==dir.normalize_or_zero())
    );
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &json!({"cases":rows,"egui":actual,"native_double_preserved":true}),
        )
        .unwrap(),
    )
    .unwrap();
}
