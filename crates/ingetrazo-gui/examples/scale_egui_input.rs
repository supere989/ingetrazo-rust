// SPDX-License-Identifier: GPL-3.0-or-later
//! Actual egui viewport input; no renderer/OS gesture claim.
use glam::DVec3;
use ingetrazo_core::{camera::StandardView, scene::SelectionItem, screen_pick::PixelViewport};
use ingetrazo_gui::IngeTrazoApp;
use ingetrazo_tools::base::ToolType;
use serde_json::{json, Value};
fn frame(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    t: f64,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 700.),
            )),
            time: Some(t),
            events,
            modifiers,
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
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
fn setup() -> (egui::Context, IngeTrazoApp, egui::Pos2, egui::Pos2) {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    let vs = [[0., 0., 0.], [2., 0., 0.], [2., 3., 0.], [0., 3., 0.]]
        .map(|v| app.scene.mesh.add_vertex(DVec3::from_array(v)));
    app.scene.mesh.add_face(vs.to_vec(), vec![]);
    app.scene.select(SelectionItem::Face(0));
    app.camera.set_standard_view(StandardView::Top);
    app.camera.perspective = false;
    app.camera.target = DVec3::new(1., 1.5, 0.);
    app.camera.distance = 12.;
    app.set_tool(ToolType::Scale);
    app.native_gpu_viewport_enabled = true;
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, 0., vec![], Default::default());
    frame(&ctx, &mut app, 0.1, vec![], Default::default());
    let r = app.native_gpu_viewport_rect.unwrap();
    let v = PixelViewport::new(&app.camera, r.width() as f64, r.height() as f64);
    let p = |world| {
        let p = v.project_for_snapping(world).unwrap();
        egui::pos2(r.min.x + p.x as f32, r.min.y + p.y as f32)
    };
    (
        ctx,
        app,
        p(DVec3::new(2., 3., 0.)),
        p(DVec3::new(4., 6., 0.)),
    )
}
fn state(app: &IngeTrazoApp) -> Value {
    json!({"active":app.active_tool.pending_model_preview(),"positions":app.scene.mesh.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"label":app.active_tool.vcb_label()})
}
fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let mut rows = vec![];
    for offset in [7.75, 8.25, 9.] {
        let (ctx, mut app, grip, _) = setup();
        let before = state(&app);
        let pos = grip + egui::vec2(offset, 0.);
        frame(
            &ctx,
            &mut app,
            0.2,
            vec![egui::Event::PointerMoved(pos), button(pos, true)],
            Default::default(),
        );
        rows.push(
            json!({"name":format!("threshold-{offset}"),"before":before,"press":state(&app)}),
        );
    }
    {
        let (ctx, mut app, grip, end) = setup();
        let before = state(&app);
        frame(
            &ctx,
            &mut app,
            0.2,
            vec![
                egui::Event::PointerMoved(grip),
                button(grip, true),
                egui::Event::PointerMoved(end),
            ],
            Default::default(),
        );
        rows.push(json!({"name":"coalesced-grab-move","before":before,"press":state(&app)}));
    }
    {
        let (ctx, mut app, grip, end) = setup();
        let before = state(&app);
        frame(
            &ctx,
            &mut app,
            0.2,
            vec![egui::Event::PointerMoved(grip), button(grip, true)],
            Default::default(),
        );
        let press = state(&app);
        frame(
            &ctx,
            &mut app,
            0.3,
            vec![button(grip, false)],
            Default::default(),
        );
        let release = state(&app);
        frame(
            &ctx,
            &mut app,
            0.4,
            vec![egui::Event::PointerMoved(end)],
            Default::default(),
        );
        let preview = state(&app);
        frame(
            &ctx,
            &mut app,
            0.5,
            vec![button(end, true)],
            Default::default(),
        );
        let commit = state(&app);
        frame(
            &ctx,
            &mut app,
            0.6,
            vec![button(end, false)],
            Default::default(),
        );
        app.undo();
        let undo = state(&app);
        app.redo();
        rows.push(json!({"name":"click-move-click","before":before,"press":press,"release":release,"preview":preview,"commit":commit,"undo":undo,"redo":state(&app)}));
    }
    {
        let (ctx, mut app, grip, end) = setup();
        frame(
            &ctx,
            &mut app,
            0.2,
            vec![egui::Event::PointerMoved(grip), button(grip, true)],
            Default::default(),
        );
        frame(
            &ctx,
            &mut app,
            0.3,
            vec![egui::Event::PointerMoved(end)],
            Default::default(),
        );
        let preview = state(&app);
        frame(
            &ctx,
            &mut app,
            0.4,
            vec![button(end, false)],
            Default::default(),
        );
        rows.push(json!({"name":"drag-release","preview":preview,"commit":state(&app)}));
    }
    {
        let (ctx, mut app, grip, end) = setup();
        let before = state(&app);
        frame(
            &ctx,
            &mut app,
            0.2,
            vec![egui::Event::PointerMoved(grip), button(grip, true)],
            Default::default(),
        );
        frame(
            &ctx,
            &mut app,
            0.3,
            vec![egui::Event::PointerMoved(end)],
            Default::default(),
        );
        frame(
            &ctx,
            &mut app,
            0.4,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            Default::default(),
        );
        rows.push(json!({"name":"escape","before":before,"cancel":state(&app)}));
    }
    std::fs::write(arg,serde_json::to_vec_pretty(&json!({"clip_cases":clip_cases(),"cases":rows,"scope":"Actual app.update_ui egui RawInput; no OS/rendered capture"})).unwrap()).unwrap();
}

fn clip_cases() -> Value {
    let input: Value = serde_json::from_slice(
        &std::fs::read(std::env::args().nth(2).expect("source clip recipe")).unwrap(),
    )
    .unwrap();
    let values: Vec<f64> = input["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let matrix = glam::DMat4::from_cols_array(values.as_slice().try_into().unwrap());
    Value::Array(input["cases"].as_array().unwrap().iter().map(|row|{let point=|key:&str|DVec3::from_array(std::array::from_fn(|i|row[key][i].as_f64().unwrap()));json!({"name":row["name"],"result":ingetrazo_gui::scale_overlay::clip_segment_front(matrix,point("a"),point("b")).map(|(a,b)|[a.to_array(),b.to_array()])})}).collect())
}
