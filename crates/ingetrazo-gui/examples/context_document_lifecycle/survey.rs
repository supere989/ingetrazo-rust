use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::Path;
fn datum(app: &IngeTrazoApp) -> Value {
    app.scene
        .geo_datum
        .as_ref()
        .map(|d| {
            let mut v = json!({"lat":d.lat,"lon":d.lon,"alt":d.alt});
            if d.north != 0. {
                v["north"] = json!(d.north);
            }
            v
        })
        .unwrap_or(Value::Null)
}
fn record(app: &IngeTrazoApp, name: &str, phase: &str, rows: &mut Vec<Value>) {
    let points: Vec<_> = app
        .scene
        .geo_points
        .iter()
        .map(|p| {
            let mut v = json!({"position":p.position.to_array()});
            if !p.name.is_empty() {
                v["name"] = json!(p.name);
            }
            if !p.desc.is_empty() {
                v["desc"] = json!(p.desc);
            }
            v
        })
        .collect();
    rows.push(json!({"case":name,"phase":phase,"points":points,"datum":datum(app),"units":{"length":ingetrazo_core::units::model_unit(&app.scene),"precision":ingetrazo_core::units::model_precision(&app.scene)},"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"depth":app.component_edit_stack.len(),"count":app.survey_point_count(),"camera":{"target":app.camera.target.to_array(),"distance":app.camera.distance}}));
}
fn import(app: &mut IngeTrazoApp, root: &Path, file: &str) {
    app.begin_survey_import(&root.join(file)).unwrap();
    if app.document_session.survey_import.is_some() {
        app.accept_survey_zone().unwrap();
        app.accept_survey_import().unwrap();
    }
}
fn enter(app: &mut IngeTrazoApp) {
    for _ in 0..2 {
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
    }
    assert_eq!(app.component_edit_stack.len(), 2);
}
fn load(root: &Path, name: &str) -> IngeTrazoApp {
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.set_language("en");
    app.open_document(&root.join(format!("{name}-input.igz")));
    app
}
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-survey.json")).unwrap()).unwrap();
    let mut rows = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut app = load(&root, name);
        if case["context"] == "nested" {
            enter(&mut app);
        }
        record(&app, name, "before", &mut rows);
        assert!(app.begin_survey_import(&root.join("bad.csv")).is_err());
        record(&app, name, "bad", &mut rows);
        if !case["existing"].as_bool().unwrap() {
            app.begin_survey_import(&root.join("comma.csv")).unwrap();
            app.cancel_survey_import();
            record(&app, name, "zone-cancel", &mut rows);
            app.begin_survey_import(&root.join("comma.csv")).unwrap();
            app.accept_survey_zone().unwrap();
            app.cancel_survey_import();
            record(&app, name, "hemisphere-cancel", &mut rows);
        }
        import(&mut app, &root, "comma.csv");
        record(&app, name, "import", &mut rows);
        app.undo();
        record(&app, name, "undo", &mut rows);
        app.redo();
        record(&app, name, "redo", &mut rows);
        import(&mut app, &root, "semicolon.csv");
        record(&app, name, "append", &mut rows);
        app.clear_survey_points().unwrap();
        record(&app, name, "clear", &mut rows);
        app.undo();
        record(&app, name, "clear-undo", &mut rows);
        app.redo();
        record(&app, name, "clear-redo", &mut rows);
        assert!(!app.clear_survey_points().unwrap());
        record(&app, name, "clear-empty", &mut rows);
        app.undo();
        record(&app, name, "restore", &mut rows);
        while !app.component_edit_stack.is_empty() {
            let camera = app.camera.clone();
            app.finish_component_edit();
            assert_eq!(app.camera.target, camera.target);
            assert_eq!(app.camera.distance, camera.distance);
        }
        record(&app, name, "finish", &mut rows);
        app.save_to_path(&root.join(format!("{name}-native.igz")));
        app.undo();
        record(&app, name, "finish-undo", &mut rows);
        app.redo();
        record(&app, name, "finish-redo", &mut rows);
    }
    std::fs::write(
        root.join("native-survey.json"),
        serde_json::to_vec_pretty(&json!({"cases":original["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
    let mut app = load(&root, "fresh");
    let mut rows = Vec::new();
    let n = "ancestor-history";
    record(&app, n, "before", &mut rows);
    import(&mut app, &root, "comma.csv");
    record(&app, n, "import", &mut rows);
    enter(&mut app);
    record(&app, n, "enter", &mut rows);
    app.undo();
    record(&app, n, "undo", &mut rows);
    app.redo();
    record(&app, n, "redo", &mut rows);
    while !app.component_edit_stack.is_empty() {
        let camera = app.camera.clone();
        app.finish_component_edit();
        assert_eq!(app.camera.target, camera.target);
        assert_eq!(app.camera.distance, camera.distance);
    }
    record(&app, n, "finish", &mut rows);
    app.save_to_path(&root.join("ancestor-history-native.igz"));
    app.undo();
    record(&app, n, "finish-undo", &mut rows);
    app.redo();
    record(&app, n, "finish-redo", &mut rows);
    std::fs::write(
        root.join("native-survey-context.json"),
        serde_json::to_vec_pretty(&json!({"states":rows})).unwrap(),
    )
    .unwrap();
    // No parser rounding touches independently authored double geometry/assets.
    let mut app = load(&root, "fresh");
    app.scene
        .mesh
        .add_vertex(glam::DVec3::new(1. + 2f64.powi(-40), 2., 3.));
    let mesh = app.scene.mesh.clone();
    let attrs = app.scene.native_document.clone();
    let count = app.undo_stack.len();
    std::fs::write(root.join("nonfinite.csv"), "A,inf,300000,2").unwrap();
    assert!(app
        .begin_survey_import(&root.join("nonfinite.csv"))
        .is_err());
    assert_eq!(app.undo_stack.len(), count);
    import(&mut app, &root, "comma.csv");
    assert_eq!(app.scene.mesh, mesh);
    assert_eq!(app.scene.native_document, attrs);
    app.undo();
    assert_eq!(app.scene.mesh, mesh);
    // Source context exit never touches camera; retain later live navigation.
    let mut camera_app = load(&root, "fresh");
    enter(&mut camera_app);
    import(&mut camera_app, &root, "comma.csv");
    camera_app.camera.target = glam::DVec3::new(11., 12., 13.);
    camera_app.camera.distance = 37.;
    while !camera_app.component_edit_stack.is_empty() {
        camera_app.finish_component_edit();
        assert_eq!(camera_app.camera.target, glam::DVec3::new(11., 12., 13.));
        assert_eq!(camera_app.camera.distance, 37.);
    }
    // A child entered after the fit inherits the parent's live-camera owner.
    let mut child_app = load(&root, "fresh");
    child_app.scene.select(SelectionItem::Group(0));
    child_app.begin_component_edit();
    import(&mut child_app, &root, "comma.csv");
    child_app.scene.select(SelectionItem::Group(0));
    child_app.begin_component_edit();
    child_app.camera.target = glam::DVec3::new(14., 15., 16.);
    child_app.camera.distance = 41.;
    while !child_app.component_edit_stack.is_empty() {
        child_app.finish_component_edit();
        assert_eq!(child_app.camera.target, glam::DVec3::new(14., 15., 16.));
        assert_eq!(child_app.camera.distance, 41.);
    }
    // Native Cancel retains its separate pre-entry camera rollback contract.
    let mut cancel_app = load(&root, "fresh");
    let original_camera = cancel_app.camera.clone();
    enter(&mut cancel_app);
    import(&mut cancel_app, &root, "comma.csv");
    while !cancel_app.component_edit_stack.is_empty() {
        cancel_app.cancel_component_edit();
    }
    assert_eq!(cancel_app.camera.target, original_camera.target);
    assert_eq!(cancel_app.camera.distance, original_camera.distance);
    std::fs::write(root.join("survey-guards.json"),serde_json::to_vec_pretty(&json!({"nonfinite_refusal_atomic":true,"native_f64_geometry_untouched":true,"unknown_document_untouched":true,"live_camera_after_import_survives_finish":true,"native_cancel_keeps_prior_camera_contract":true,"later_child_entry_inherits_live_survey_camera":true})).unwrap()).unwrap();
}
