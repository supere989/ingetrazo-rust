use glam::DVec3;
use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
fn setup(c: &mut OrbitCamera, v: &Value) {
    c.target = DVec3::from_array(serde_json::from_value(v["target"].clone()).unwrap());
    c.yaw = v["yaw"].as_f64().unwrap();
    c.pitch = v["pitch"].as_f64().unwrap();
    c.distance = v["distance"].as_f64().unwrap();
    c.perspective = v["perspective"].as_bool().unwrap();
    c.aspect = v["aspect"].as_f64().unwrap();
}
fn state(c: &OrbitCamera) -> Value {
    json!({"target":c.target.to_array(),"yaw":c.yaw,"pitch":c.pitch,"distance":c.distance})
}
fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, time: f64, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        )),
        time: Some(time),
        events,
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| ingetrazo_gui::viewport::render_viewport(a, ui));
    });
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let source: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let mut core = vec![];
    for row in source["core"].as_array().unwrap() {
        let mut c = OrbitCamera::new();
        setup(&mut c, &row["initial"]);
        let v = &row["args"];
        let accepted = match row["kind"].as_str().unwrap() {
            "orbit" => c.orbit_about(
                DVec3::new(3., 2., 1.),
                v[0].as_f64().unwrap(),
                v[1].as_f64().unwrap(),
                v[2].as_f64().unwrap(),
            ),
            "zoom" => c.zoom_to(
                v[0].as_f64().unwrap(),
                DVec3::from_array(serde_json::from_value(v[1].clone())?),
                v[2].as_f64().unwrap(),
            ),
            "pan" => c.pan_pixels(
                v[0].as_f64().unwrap(),
                v[1].as_f64().unwrap(),
                v[2].as_f64().unwrap(),
                Some(v[3].as_f64().unwrap()),
            ),
            _ => panic!(),
        };
        assert!(accepted);
        core.push(state(&c));
    }
    let mut sessions = vec![];
    for row in source["sessions"].as_array().unwrap() {
        let mut a = IngeTrazoApp::new();
        a.native_gpu_viewport_enabled = true;
        setup(&mut a.camera, &row["initial"]);
        if !row["empty"].as_bool().unwrap() {
            let ids = row["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    a.scene.mesh.add_vertex(DVec3::from_array(
                        serde_json::from_value(p.clone()).unwrap(),
                    ))
                })
                .collect();
            a.scene.mesh.add_face(ids, vec![]).unwrap();
            a.scene.bump_version();
        }
        let original_version = a.scene.version;
        let original_revision = a.model_revision;
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, 0., vec![]);
        let mut rows = vec![];
        let mut time = 0.1;
        let modifiers = egui::Modifiers {
            shift: row["pan"].as_bool().unwrap(),
            ..Default::default()
        };
        for e in row["events"].as_array().unwrap() {
            let kind = e[0].as_str().unwrap();
            let pos = egui::pos2(e[1].as_f64().unwrap() as f32, e[2].as_f64().unwrap() as f32);
            let mut events = vec![egui::Event::PointerMoved(pos)];
            events.push(match kind {
                "wheel" => egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0., e[3].as_f64().unwrap() as f32),
                    modifiers,
                },
                "press" | "release" => egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Middle,
                    pressed: kind == "press",
                    modifiers,
                },
                "move" => egui::Event::PointerMoved(pos),
                _ => panic!(),
            });
            frame(&ctx, &mut a, time, events);
            time += 0.1;
            assert_eq!(a.scene.version, original_version);
            assert_eq!(a.model_revision, original_revision);
            assert!(a.undo_stack.is_empty() && a.redo_stack.is_empty());
            let (calls, pivot) = a.cursor_navigation_probe();
            rows.push(json!({"camera":state(&a.camera),"acquisitions":calls,"pivot":pivot}));
        }
        sessions.push(json!({"name":row["name"],"rows":rows}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({"core":core,"sessions":sessions}))?,
    )?;
    Ok(())
}
