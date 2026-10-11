//! Missing original keyboard transitions through actual egui App ingress.
use glam::DVec3;
use ingetrazo_core::{math::Ray, scene::Scene};
use ingetrazo_gui::app::IngeTrazoApp;
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
fn coord(p: DVec3) -> Vec<f64> {
    p.to_array()
        .into_iter()
        .map(|x| (x * 1e5).round() / 1e5)
        .collect()
}
fn state(a: &IngeTrazoApp) -> Value {
    let m = &a.scene.mesh;
    let mut v: Vec<_> = m.vertices.iter().map(|v| coord(v.position)).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut e: Vec<_> = m
        .edges
        .iter()
        .map(|e| {
            let mut v = vec![
                coord(m.vertices[e.v0].position),
                coord(m.vertices[e.v1].position),
            ];
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v
        })
        .collect();
    e.sort_by(|a, b| a.partial_cmp(b).unwrap());
    json!({"buffer":a.vcb_text,"tool":match a.active_tool_type{ToolType::Line=>"line",ToolType::Rectangle=>"rectangle",ToolType::Move=>"move",ToolType::Select=>"select",_=>"other"},"anchor":a.active_tool.snap_anchor().map(coord),"vertices":v,"edges":e,"faces":m.faces.len(),"history":[a.undo_stack.len(),a.redo_stack.len()]})
}
fn acquire(a: &mut IngeTrazoApp) {
    let c = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::Z * 10., -DVec3::Z));
    a.dispatch_tool_click(&c);
    let point = if a.active_tool_type == ToolType::Rectangle {
        DVec3::new(1., 1., 0.)
    } else {
        DVec3::X
    };
    a.dispatch_tool_move(&ToolContext::new(
        point,
        Ray::new(point + DVec3::Z * 10., -DVec3::Z),
    ));
}
fn mods(s: &str) -> egui::Modifiers {
    let mut m = egui::Modifiers::default();
    m.ctrl = s.contains("ctrl");
    m.alt = s.contains("alt");
    m.shift = s.contains("shift");
    m.mac_cmd = s.contains("meta");
    m.command = m.ctrl || m.mac_cmd;
    m
}
fn key(k: egui::Key, m: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: m,
    }
}
fn frame(
    ctx: &egui::Context,
    a: &mut IngeTrazoApp,
    time: f64,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000., 800.),
            )),
            time: Some(time),
            events,
            modifiers,
            ..Default::default()
        },
        |ctx| a.update_ui(ctx),
    );
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut rows = vec![];
    for row in source["cases"].as_array().unwrap() {
        let r = &row["recipe"];
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene = Scene::new();
        a.native_gpu_viewport_enabled = true;
        a.set_tool(match r["tool"].as_str().unwrap() {
            "rectangle" => ToolType::Rectangle,
            "move" => ToolType::Move,
            _ => ToolType::Line,
        });
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, 0., vec![], Default::default());
        if r["acquire"] == true {
            acquire(&mut a);
        }
        let mut states = vec![json!({"phase":"before","state":state(&a)})];
        for (i, op) in r["operations"].as_array().unwrap().iter().enumerate() {
            let kind = op[0].as_str().unwrap();
            let m = mods(
                op[if kind == "text" { 2 } else { 1 }]
                    .as_str()
                    .unwrap_or(""),
            );
            let mut events = vec![];
            match kind {
                "text" => {
                    let text = op[1].as_str().unwrap();
                    let k = match text.to_ascii_lowercase().as_str() {
                        "m" => Some(egui::Key::M),
                        "r" => Some(egui::Key::R),
                        " " => Some(egui::Key::Space),
                        _ => None,
                    };
                    if let Some(k) = k {
                        events.push(key(k, m));
                    }
                    events.push(egui::Event::Text(text.into()));
                }
                "enter" => events.push(key(egui::Key::Enter, m)),
                "backspace" => events.push(key(egui::Key::Backspace, m)),
                "escape" => events.push(key(egui::Key::Escape, m)),
                "select" => events.push(key(egui::Key::Space, m)),
                "repeat" => events.push(key(egui::Key::R, mods("shift"))),
                "undo" => events.push(key(egui::Key::Z, mods("ctrl"))),
                "redo" => events.push(key(egui::Key::Y, mods("ctrl"))),
                "acquire" => acquire(&mut a),
                _ => panic!("unknown operation"),
            };
            frame(&ctx, &mut a, (i + 1) as f64 * 0.1, events, m);
            states.push(json!({"phase":format!("{}-{}",i,kind),"state":state(&a)}));
        }
        rows.push(json!({"name":r["name"],"states":states}));
    }
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.scene = Scene::new();
    a.native_gpu_viewport_enabled = true;
    a.set_tool(ToolType::Line);
    acquire(&mut a);
    let ctx = egui::Context::default();
    let mut events = vec![egui::Event::Text("1".into())];
    for (text, m) in [
        ("2", "ctrl"),
        ("3", "alt"),
        ("4", "meta"),
        ("5", "ctrl-alt"),
        ("6", "shift"),
    ] {
        events.push(key(egui::Key::Num2, mods(m)));
        events.push(egui::Event::Text(text.into()));
    }
    events.push(key(egui::Key::Enter, mods("ctrl")));
    frame(&ctx, &mut a, 0.1, events, Default::default());
    let one_frame = state(&a);
    a.dispatch_tool_cancel();
    a.set_tool(ToolType::Line);
    frame(
        &ctx,
        &mut a,
        0.2,
        vec![egui::Event::Text("2.5".into())],
        Default::default(),
    );
    let mixed_payload = a.vcb_text.clone();
    frame(
        &ctx,
        &mut a,
        0.3,
        vec![egui::Event::Text("12".into())],
        Default::default(),
    );
    let digits_payload = a.vcb_text.clone();
    a.vcb_text.clear();
    a.show_preferences_dialog = true;
    frame(
        &ctx,
        &mut a,
        0.4,
        vec![egui::Event::Text("5".into())],
        Default::default(),
    );
    let modal_buffer = a.vcb_text.clone();
    a.show_preferences_dialog = false;
    let editor = egui::Id::new("unrelated-keyboard-editor");
    let mut editor_text = String::new();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut editor_text).id(editor))
        });
    });
    ctx.memory_mut(|m| m.request_focus(editor));
    let mut editor_event_retained=false;
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Text("7".into())],
            ..Default::default()
        },
        |ctx| {
            a.update_ui(ctx);
            editor_event_retained=ctx.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Text(t) if t=="7")));
            egui::Window::new("focused-editor-proof").show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut editor_text).id(editor))
            });
        },
    );
    let editor_buffer = a.vcb_text.clone();
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"cases":rows,"one_frame_chords":one_frame,"payload_guards":{"mixed":mixed_payload,"digits":digits_payload,"modal":modal_buffer,"editor_buffer":editor_buffer,"editor_event_retained":editor_event_retained}})).unwrap(),
    )
    .unwrap();
}
