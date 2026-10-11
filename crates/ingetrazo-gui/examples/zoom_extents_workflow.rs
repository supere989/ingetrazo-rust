//! Actual App Open/Zoom Extents, preserving model/history through camera-only actions.
use glam::DVec3;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
fn state(app: &IngeTrazoApp) -> Value {
    let c = &app.camera;
    json!({"target":c.target.to_array(),"distance":c.distance,"yaw":c.yaw,"pitch":c.pitch,"perspective":c.perspective,"two_point":c.two_point})
}
fn frame(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400., 1000.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| app.update_ui(ctx),
    )
}
fn find(shape: &egui::epaint::Shape, name: &str) -> Option<egui::Pos2> {
    match shape {
        egui::epaint::Shape::Text(t) if t.galley.text().split("\t").next() == Some(name) => {
            Some(t.pos + t.galley.rect.center().to_vec2())
        }
        egui::epaint::Shape::Vec(v) => v.iter().find_map(|s| find(s, name)),
        _ => None,
    }
}
fn click(
    ctx: &egui::Context,
    app: &mut IngeTrazoApp,
    out: &egui::FullOutput,
    time: &mut f64,
    name: &str,
) -> egui::FullOutput {
    let p = out
        .shapes
        .iter()
        .find_map(|s| find(&s.shape, name))
        .unwrap_or_else(|| panic!("Menu item missing: {name}"));
    *time += 0.1;
    frame(
        ctx,
        app,
        *time,
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    *time += 0.1;
    frame(
        ctx,
        app,
        *time,
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    )
}
fn mode_session(source: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let mut app = IngeTrazoApp::new();
    let i = &source["mode_session"]["initial"];
    app.camera.target = DVec3::from_array(serde_json::from_value(i["target"].clone())?);
    app.camera.distance = i["distance"].as_f64().unwrap();
    app.camera.yaw = i["yaw"].as_f64().unwrap();
    app.camera.pitch = i["pitch"].as_f64().unwrap();
    app.camera.perspective = i["perspective"].as_bool().unwrap();
    app.camera.two_point = i["two_point"].as_bool().unwrap();
    let model = app.scene.clone();
    let mut phases = vec![json!({"phase":"before","camera":state(&app)})];
    let ctx = egui::Context::default();
    let mut time = 0.;
    let mut out = frame(&ctx, &mut app, time, vec![]);
    for (phase, name) in [
        ("two-point", "Two-Point Perspective"),
        ("parallel", "Toggle Perspective / Parallel"),
        ("perspective", "Toggle Perspective / Parallel"),
    ] {
        out = click(&ctx, &mut app, &out, &mut time, "Camera");
        time += 0.1;
        out = frame(&ctx, &mut app, time, vec![]);
        out = click(&ctx, &mut app, &out, &mut time, name);
        phases.push(json!({"phase":phase,"camera":state(&app)}));
        time += 0.1;
        out = frame(&ctx, &mut app, time, vec![]);
    }
    assert_eq!(app.scene, model);
    assert!(app.undo_stack.is_empty() && app.redo_stack.is_empty());
    Ok(json!({"phases":phases,"model_history_unchanged":true}))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("Input directory required")?;
    let dir = Path::new(&dir);
    let source: Value = serde_json::from_slice(&std::fs::read(dir.join("source.json"))?)?;
    let mut cases = vec![];
    for row in source["cases"].as_array().ok_or("Cases absent")? {
        let name = row["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.open_document(&dir.join(format!("{name}.igz")));
        if app.current_file.as_deref() != Some(dir.join(format!("{name}.igz")).as_path()) {
            return Err(format!("Open failed {name}: {}", app.status_message).into());
        }
        let init = &row["initial"];
        let c = &mut app.camera;
        c.target = DVec3::from_array(serde_json::from_value(init["target"].clone())?);
        c.distance = init["distance"].as_f64().unwrap();
        c.yaw = init["yaw"].as_f64().unwrap();
        c.pitch = init["pitch"].as_f64().unwrap();
        c.perspective = init["perspective"].as_bool().unwrap();
        c.two_point = init["two_point"].as_bool().unwrap();
        c.aspect = init["aspect"].as_f64().unwrap();
        c.fov_deg = init["fov_deg"].as_f64().unwrap();
        c.znear = init["znear"].as_f64().unwrap();
        c.zfar = init["zfar"].as_f64().unwrap();
        let before = app.scene.clone();
        let stacks = (app.undo_stack.len(), app.redo_stack.len());
        let rev = app.model_revision;
        app.zoom_extents();
        let once = state(&app);
        app.zoom_extents();
        assert_eq!(app.scene, before);
        assert_eq!((app.undo_stack.len(), app.redo_stack.len()), stacks);
        assert_eq!(app.model_revision, rev);
        cases.push(
            json!({"name":name,"once":once,"twice":state(&app),"model_history_unchanged":true}),
        );
    }
    let mut app = IngeTrazoApp::new();
    app.scene.mesh.add_vertex(DVec3::splat(f64::INFINITY));
    let a = app.scene.mesh.add_vertex(DVec3::ZERO);
    let b = app.scene.mesh.add_vertex(DVec3::X);
    app.scene.mesh.add_edge(a, b);
    app.camera.distance = f64::NAN;
    let before = app.camera.target;
    app.zoom_extents();
    assert_eq!(app.camera.target, before);
    assert!(app.camera.distance.is_nan());
    let nonfinite_camera_refused = app.status_message.contains("Zoom Extents");
    assert!(nonfinite_camera_refused);
    app.camera.distance = 17.;
    app.scene.mesh.edges[0].v0 = 0;
    let before = state(&app);
    let model = app.scene.clone();
    app.zoom_extents();
    assert_eq!(state(&app), before);
    assert_eq!(app.scene, model);
    assert!(app.status_message.contains("nonfinite source coordinates"));
    let nonfinite_model_refused = true;
    app.scene.native_document = json!({"photo_mesh":{"geometry":"absent.npz","visible":true}});
    app.scene.mesh = Default::default();
    app.camera.distance = 17.;
    let before = state(&app);
    app.zoom_extents();
    assert_eq!(state(&app), before);
    assert!(app.status_message.contains("Survey geometry resource"));
    std::fs::write(
        dir.join("native.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":cases,"mode_session":mode_session(&source)?,"nonfinite_camera_refused":nonfinite_camera_refused,"missing_survey_refused":true,"nonfinite_model_refused":nonfinite_model_refused}),
        )?,
    )?;
    Ok(())
}
