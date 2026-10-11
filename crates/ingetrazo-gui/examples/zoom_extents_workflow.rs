//! Actual App Open/Zoom Extents, preserving model/history through camera-only actions.
use glam::DVec3;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
fn state(app: &IngeTrazoApp) -> Value {
    let c = &app.camera;
    json!({"target":c.target.to_array(),"distance":c.distance,"yaw":c.yaw,"pitch":c.pitch,"perspective":c.perspective,"two_point":c.two_point})
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
            &json!({"cases":cases,"nonfinite_camera_refused":nonfinite_camera_refused,"missing_survey_refused":true,"nonfinite_model_refused":nonfinite_model_refused}),
        )?,
    )?;
    Ok(())
}
