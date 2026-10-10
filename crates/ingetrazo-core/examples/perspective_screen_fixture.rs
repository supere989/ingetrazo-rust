// SPDX-License-Identifier: GPL-3.0-or-later
//! Frozen original-projection input protocol. Production camera and screen engine.
use glam::{DVec2, DVec3};
use ingetrazo_core::{
    camera::OrbitCamera,
    scene::Scene,
    screen_pick::PixelViewport,
    snap::{SnapEngine, SnapKind},
    snap_screen::ScreenSnapContext,
};
use std::io::Read;
fn point(v: &serde_json::Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn kind(k: SnapKind) -> &'static str {
    match k {
        SnapKind::AxisLock => "axis",
        SnapKind::AxisInference => "axis_inference",
        SnapKind::Endpoint => "endpoint",
        SnapKind::Midpoint => "midpoint",
        SnapKind::Intersection => "intersection",
        SnapKind::Reference => "reference",
        SnapKind::FromPoint => "from_point",
        SnapKind::OnEdge => "on_edge",
        SnapKind::Origin => "origin",
        SnapKind::Center => "center",
        SnapKind::None => "none",
        _ => "other",
    }
}
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    let cases: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut output = vec![];
    for case in cases.as_array().unwrap() {
        let camera: OrbitCamera = serde_json::from_value(case["camera"].clone()).unwrap();
        let mut scene = Scene::new();
        for edge in case["edges"].as_array().unwrap() {
            let a = scene.mesh.add_vertex(point(&edge[0]));
            let b = scene.mesh.add_vertex(point(&edge[1]));
            let id = scene.mesh.add_edge(a, b).unwrap();
            if case["curve"].as_bool() == Some(true) {
                scene.mesh.edges[id].curve_id = Some(71);
            }
        }
        let mut engine = SnapEngine::default();
        let mut steps = vec![];
        for step in case["steps"].as_array().unwrap() {
            if let Some(delta) = step.get("move") {
                for v in &mut scene.mesh.vertices {
                    v.position += point(delta);
                }
                scene.bump_version();
                engine.note_model_revision(scene.version);
            }
            if step["delete"].as_bool() == Some(true) {
                scene.mesh.clear();
                scene.bump_version();
                engine.note_model_revision(scene.version);
            }
            let cursor = DVec2::new(
                step["pixel"][0].as_f64().unwrap(),
                step["pixel"][1].as_f64().unwrap(),
            );
            let viewport = PixelViewport::new(&camera, 1000., 700.);
            let ctx = ScreenSnapContext {
                viewport,
                cursor,
                candidate: point(&step["candidate"]),
                plane_normal: DVec3::Z,
            };
            let ray = camera.screen_to_ray(cursor.x, cursor.y, 1000., 700.);
            let result = engine.snap_screen(&ray, &scene, None, None, &ctx);
            engine.note_screen_result(&result, false, step["time"].as_f64().unwrap_or(0.));
            steps.push(serde_json::json!({"kind":kind(result.kind),"point":result.point.to_array(),"encouraged":engine.encouraged_points.iter().map(|p|p.to_array()).collect::<Vec<_>>()}));
        }
        output.push(serde_json::json!({"name":case["name"],"steps":steps}));
    }
    println!("{}", serde_json::to_string(&output).unwrap());
}
