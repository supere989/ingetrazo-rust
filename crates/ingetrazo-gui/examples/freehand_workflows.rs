// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual App shape activation/dispatch/history with explicit source world recipes.
use glam::{DVec2, DVec3};
use ingetrazo_core::{math::Ray, scene::Scene};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
fn v(a: &Value) -> DVec3 {
    DVec3::new(
        a[0].as_f64().unwrap(),
        a[1].as_f64().unwrap(),
        a[2].as_f64().unwrap(),
    )
    .as_vec3()
    .as_dvec3()
}
fn state(a: &IngeTrazoApp) -> Value {
    let m = &a.scene.mesh;
    let mut curves = std::collections::HashMap::new();
    let mut next = 0usize;
    let edges=m.edges.iter().map(|e|{let curve=e.curve_id.map(|id|*curves.entry(id).or_insert_with(||{let n=next;next+=1;n}));json!({"vertices":[e.v0,e.v1],"soft":e.soft,"hidden":e.hidden,"layer":e.layer,"curve":curve})}).collect::<Vec<_>>();
    json!({"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":edges,"faces":m.faces.iter().map(|f|json!({"vertices":f.vertices,"holes":f.holes,"attrs":f.attrs,"interior":f.interior})).collect::<Vec<_>>(),"preview":a.active_tool.rubber_band_lines().iter().map(|(x,y)|[x.to_array(),y.to_array()]).collect::<Vec<_>>(),"preview_color":a.active_tool.preview_color().map(|c|[c[0],c[1],c[2],1.]),"caption":if a.active_tool.vcb_label().is_empty(){None}else{Some(a.active_tool.vcb_label())},"history":[a.undo_stack.len(),a.redo_stack.len()],"last_error":a.active_tool.error_message()})
}
fn egui_ingress() -> Vec<Value> {
    fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, time: f64, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 700.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| a.update_ui(ctx),
        );
    }
    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    let mut out = vec![];
    for mode in ["closed-release", "open-release", "cancel-repeat"] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a.camera
            .set_standard_view(ingetrazo_core::camera::StandardView::Top);
        a.camera.perspective = false;
        a.camera.target = DVec3::ZERO;
        a.camera.distance = 12.;
        a.native_gpu_viewport_enabled = true;
        a.set_tool(ToolType::Freehand);
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, 0., vec![]);
        frame(&ctx, &mut a, 0.1, vec![]);
        let rect = a.native_gpu_viewport_rect.unwrap();
        let origin = rect.center();
        let points = [
            origin,
            origin + egui::vec2(100., 0.),
            origin + egui::vec2(100., 100.),
            origin + egui::vec2(0., 100.),
            origin,
        ];
        // Declared raw pixel-to-plane contract uses the already validated camera;
        // no snap-engine output participates in expected authored coordinates.
        let raw = |pixel: egui::Pos2| {
            let ray = a.camera.screen_to_ray(
                (pixel.x - rect.min.x) as f64,
                (pixel.y - rect.min.y) as f64,
                rect.width() as f64,
                rect.height() as f64,
            );
            let t = -ray.origin.z / ray.dir.z;
            ray.at(t).as_vec3().as_dvec3()
        };
        let expected: Vec<_> = points.iter().map(|&p| raw(p).to_array()).collect();
        let near = raw(origin + egui::vec2(5., 2.));
        let near_end = raw(origin + egui::vec2(5., 25.));
        let x = a.scene.mesh.add_vertex(near);
        let y = a.scene.mesh.add_vertex(near_end);
        a.scene.mesh.add_edge(x, y).unwrap();
        let initial = state(&a);
        frame(&ctx, &mut a, 0.2, vec![egui::Event::PointerMoved(origin)]);
        frame(&ctx, &mut a, 0.3, vec![button(origin, true)]);
        let press = state(&a);
        let count = if mode == "open-release" { 3 } else { 5 };
        for (i, &p) in points[1..count].iter().enumerate() {
            frame(
                &ctx,
                &mut a,
                0.4 + i as f64 * 0.1,
                vec![egui::Event::PointerMoved(p)],
            );
        }
        let hover = state(&a);
        if mode == "cancel-repeat" {
            frame(
                &ctx,
                &mut a,
                1.,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
            );
        }
        let before_release = state(&a);
        frame(&ctx, &mut a, 1.1, vec![button(points[count - 1], false)]);
        let release = state(&a);
        if mode == "cancel-repeat" {
            frame(
                &ctx,
                &mut a,
                1.2,
                vec![egui::Event::PointerMoved(origin), button(origin, true)],
            );
            for (i, &p) in points[1..].iter().enumerate() {
                frame(
                    &ctx,
                    &mut a,
                    1.3 + i as f64 * 0.1,
                    vec![egui::Event::PointerMoved(p)],
                );
            }
            frame(&ctx, &mut a, 1.8, vec![button(origin, false)]);
        }
        let repeated = state(&a);
        a.undo();
        let undo = state(&a);
        a.redo();
        let redo = state(&a);
        out.push(json!({"mode":mode,"pixel_positions":points.iter().map(|p|[p.x,p.y]).collect::<Vec<_>>(),"expected_world":expected,"nearby_world":[near.to_array(),near_end.to_array()],"initial":initial,"press":press,"hover":hover,"before_release":before_release,"release":release,"repeated":repeated,"undo":undo,"redo":redo}));
    }
    out
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source["cases"].as_array().unwrap() {
        let recipe = &row["recipe"];
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a.set_tool(ToolType::Freehand);
        a.active_tool
            .note_cursor_plane(DVec3::ZERO, v(&recipe["normal"]));
        let mut states = vec![json!({"phase":"activated","state":state(&a)})];
        let mut last = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::Z, -DVec3::Z));
        for (i, op) in recipe["operations"].as_array().unwrap().iter().enumerate() {
            let kind = op[0].as_str().unwrap();
            match kind {
                "press" | "move" => {
                    let p = v(&op[1]);
                    let mut c = ToolContext::new(
                        p,
                        Ray::new(p + v(&recipe["normal"]) * 10., -v(&recipe["normal"])),
                    );
                    c.screen = Some(DVec2::new(
                        op[2][0].as_f64().unwrap(),
                        op[2][1].as_f64().unwrap(),
                    ));
                    a.active_tool
                        .note_cursor_plane(DVec3::ZERO, v(&recipe["normal"]));
                    if kind == "press" {
                        a.dispatch_tool_click(&c);
                    } else {
                        a.dispatch_tool_move(&c);
                    }
                    last = c;
                }
                "release" => a.dispatch_tool_release(&last, 100.),
                "cancel" => a.dispatch_tool_cancel(),
                "undo" => a.undo(),
                "redo" => a.redo(),
                _ => panic!(),
            }
            states.push(json!({"phase":format!("{i}-{kind}"),"accepted":null,"state":state(&a)}));
        }
        rows.push(json!({"recipe":recipe,"activation":"FreehandTool","states":states}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows,"egui_ingress":egui_ingress()})).unwrap(),
    )
    .unwrap();
}
