// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual App shape activation/dispatch/history with explicit source world recipes.
use glam::{DMat4, DVec2, DVec3};
use ingetrazo_core::{
    groups::Group,
    math::Ray,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
    screen_pick::{HoverReference, PixelViewport, ReferenceKind},
};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::{
    arc::ArcMode,
    base::{ToolContext, ToolType},
};
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
    fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, t: f64, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000., 700.),
                )),
                time: Some(t),
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
    let mut rows = vec![];
    for name in [
        "circle",
        "polygon",
        "arc",
        "arc3",
        "center_arc",
        "pie",
        "rotated_rect",
    ] {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a.camera
            .set_standard_view(ingetrazo_core::camera::StandardView::Top);
        a.camera.perspective = false;
        a.camera.target = DVec3::ZERO;
        a.camera.distance = 12.;
        a.native_gpu_viewport_enabled = true;
        match name {
            "circle" => a.set_tool(ToolType::Circle),
            "polygon" => a.set_tool(ToolType::Polygon),
            "rotated_rect" => a.set_tool(ToolType::RotatedRectangle),
            "arc" => a.activate_arc(ArcMode::TwoPoint),
            "arc3" => a.activate_arc(ArcMode::ThreePoint),
            "center_arc" => a.activate_arc(ArcMode::Center),
            _ => a.activate_arc(ArcMode::Pie),
        };
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, 0., vec![]);
        frame(&ctx, &mut a, 0.1, vec![]);
        let rect = a.native_gpu_viewport_rect.unwrap();
        let vp = PixelViewport::new(&a.camera, rect.width() as f64, rect.height() as f64);
        let pixel = |p| {
            let p = vp.project_for_snapping(p).unwrap();
            egui::pos2(rect.min.x + p.x as f32, rect.min.y + p.y as f32)
        };
        let first = pixel(DVec3::ZERO);
        let second = pixel(DVec3::new(2., 0.03, 0.));
        frame(&ctx, &mut a, 0.2, vec![egui::Event::PointerMoved(first)]);
        let cursor = state(&a);
        frame(&ctx, &mut a, 0.3, vec![button(first, true)]);
        frame(&ctx, &mut a, 0.4, vec![button(first, false)]);
        let pressed = state(&a);
        frame(&ctx, &mut a, 0.5, vec![egui::Event::PointerMoved(second)]);
        let hovered = state(&a);
        frame(&ctx, &mut a, 0.6, vec![button(second, true)]);
        frame(&ctx, &mut a, 0.7, vec![button(second, false)]);
        let second_pressed = state(&a);
        a.dispatch_tool_cancel();
        rows.push(json!({"tool":name,"cursor":cursor,"pressed":pressed,"hovered":hovered,"second_pressed":second_pressed,"cancel":state(&a)}));
    }
    rows
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source["cases"].as_array().unwrap() {
        let recipe = &row["recipe"];
        let mut a = IngeTrazoApp::new();
        a.scene = Scene::new();
        if recipe["square"] == true {
            let pts = [
                DVec3::ZERO,
                DVec3::X * 2.,
                DVec3::new(2., 2., 0.),
                DVec3::Y * 2.,
            ];
            a.scene.mesh = ingetrazo_core::drafting::add_segments_tagged_with_id(
                &Mesh::new(),
                &pts.iter()
                    .copied()
                    .zip(pts.iter().copied().cycle().skip(1))
                    .take(4)
                    .collect::<Vec<_>>(),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        match recipe["tool"].as_str().unwrap() {
            "circle" => a.set_tool(ToolType::Circle),
            "polygon" => a.set_tool(ToolType::Polygon),
            "rotated_rect" => a.set_tool(ToolType::RotatedRectangle),
            "arc" => a.activate_arc(ArcMode::TwoPoint),
            "arc3" => a.activate_arc(ArcMode::ThreePoint),
            "center_arc" => a.activate_arc(ArcMode::Center),
            "pie" => a.activate_arc(ArcMode::Pie),
            _ => panic!(),
        };
        let viewport = PixelViewport {
            matrix: DMat4::from_cols_array(&[
                0.2,
                0.,
                0.,
                0.,
                0.,
                2. / 7.,
                0.,
                0.,
                0.,
                2. / 7.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ]),
            width: 1000.,
            height: 700.,
        };
        a.active_tool
            .note_pick_viewport(viewport, DVec3::Z * 10., a.model_revision);
        a.active_tool.note_cursor_plane(DVec3::ZERO, DVec3::Z);
        let mut states = vec![json!({"phase":"activated","state":state(&a)})];
        let mut ray = None;
        let mut reference = None;
        for (i, op) in recipe["operations"].as_array().unwrap().iter().enumerate() {
            let kind = op[0].as_str().unwrap();
            let mut accepted = None;
            match kind {
                "click" | "move" | "double" => {
                    let p = v(&op[1]);
                    let r = ray.unwrap_or(Ray::new(DVec3::new(p.x, p.y + p.z, 10.), -DVec3::Z));
                    let mut c = ToolContext::with_modifiers(
                        p,
                        r,
                        op.get(2).and_then(Value::as_bool).unwrap_or(false),
                        false,
                        false,
                    );
                    c.screen = Some(DVec2::new(p.x * 100., -p.y * 100. - p.z * 100.));
                    if recipe["point_snaps"] == true {
                        let mut snap = ingetrazo_core::snap::SnapResult {
                            point: p,
                            ..Default::default()
                        };
                        snap.kind = ingetrazo_core::snap::SnapKind::Endpoint;
                        a.active_tool.note_snap_result(&snap);
                    }
                    match kind {
                        "move" => a.dispatch_tool_move(&c),
                        "double" => a.dispatch_tool_double_click(&c),
                        _ => a.dispatch_tool_click(&c),
                    }
                }
                "plane" => a.active_tool.note_cursor_plane(v(&op[1]), v(&op[2])),
                "edge" => {
                    let p = v(&op[1]);
                    let q = v(&op[2]);
                    let x = a.scene.mesh.add_vertex(p);
                    let y = a.scene.mesh.add_vertex(q);
                    a.scene.mesh.add_edge(x, y).unwrap();
                    reference = Some((p, q));
                }
                "ray" => {
                    let p = v(&op[1]);
                    ray = Some(Ray::new(
                        p,
                        (v(&op[2]) - p).as_vec3().normalize().as_dvec3(),
                    ));
                }
                "context" => {
                    let mut g = Group::new("turned");
                    let matrix = DMat4::from_rotation_y(std::f64::consts::FRAC_PI_2)
                        .as_mat4()
                        .as_dmat4();
                    g.native_document = json!({"axes":matrix.to_cols_array()});
                    a.scene.groups.push(g);
                    a.scene.select(SelectionItem::Group(0));
                    a.begin_component_edit();
                    // Original controlled recipe keeps its acquired handler; native
                    // App context entry intentionally selects Select. Reactivate the
                    // requested shape inside the admitted context, explicitly.
                    match recipe["tool"].as_str().unwrap() {
                        "circle" => a.set_tool(ToolType::Circle),
                        "polygon" => a.set_tool(ToolType::Polygon),
                        "rotated_rect" => a.set_tool(ToolType::RotatedRectangle),
                        "arc" => a.activate_arc(ArcMode::TwoPoint),
                        "arc3" => a.activate_arc(ArcMode::ThreePoint),
                        "center_arc" => a.activate_arc(ArcMode::Center),
                        _ => a.activate_arc(ArcMode::Pie),
                    }
                    a.active_tool
                        .note_pick_viewport(viewport, DVec3::Z * 10., a.model_revision);
                    a.active_tool.note_cursor_plane(DVec3::ZERO, DVec3::Z);
                }
                "key" => {
                    let axes = a.scene.drawing_axes();
                    accepted = Some(match op[1].as_str().unwrap() {
                        "Right" => a.active_tool.lock_drawing_plane(axes.x),
                        "Left" => a.active_tool.lock_drawing_plane(axes.y),
                        "Up" => a.active_tool.lock_drawing_plane(axes.z),
                        "Down" => {
                            let (p, q) = reference.unwrap();
                            a.active_tool.lock_drawing_reference(Some(HoverReference {
                                normal: (q - p).normalize(),
                                kind: ReferenceKind::Edge,
                            }))
                        }
                        _ => false,
                    });
                }
                "value" | "segments" | "radius" => {
                    let text = if kind == "segments" {
                        format!("{}s", op[1])
                    } else if kind == "radius" {
                        format!("{}r", op[1])
                    } else if let Some(values) = op[1].as_array() {
                        values
                            .iter()
                            .map(Value::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    } else {
                        op[1].to_string()
                    };
                    accepted = Some(a.dispatch_tool_vcb(&text).is_ok());
                }
                "cancel" => a.dispatch_tool_cancel(),
                "undo" => a.undo(),
                "redo" => a.redo(),
                _ => panic!("unknown operation {kind}"),
            }
            states
                .push(json!({"phase":format!("{i}-{kind}"),"accepted":accepted,"state":state(&a)}));
        }
        rows.push(json!({"recipe":recipe,"states":states}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows,"egui_ingress":egui_ingress()})).unwrap(),
    )
    .unwrap();
}
