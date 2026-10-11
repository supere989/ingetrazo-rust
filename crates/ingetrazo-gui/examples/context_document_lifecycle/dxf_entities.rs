use super::*;
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("inputs"));
    let source: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-dxf-entities.json")).unwrap())
            .unwrap();
    let mut rows = Vec::new();
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("settings-{name}")),
        ));
        app.open_document(&root.join(format!("{name}-input.igz")));
        app.current_file = None;
        if case["context"] == "leaf" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        let record = |app: &IngeTrazoApp, phase: &str, rows: &mut Vec<Value>| {
            rows.push(json!({"case":name,"phase":phase,"depth":app.component_edit_stack.len(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"scene":super::nested_import::source_scene(app)}))
        };
        record(&app, "before", &mut rows);
        app.begin_unit_import(&root.join(case["file"].as_str().unwrap()))
            .unwrap();
        app.document_session.unit_import.as_mut().unwrap().code = "m".into();
        app.accept_unit_import().unwrap();
        record(&app, "import", &mut rows);
        app.undo();
        record(&app, "undo", &mut rows);
        app.redo();
        record(&app, "redo", &mut rows);
        while !app.component_edit_stack.is_empty() {
            let n = app.component_edit_stack.len();
            app.finish_component_edit();
            assert_eq!(
                app.component_edit_stack.len(),
                n - 1,
                "{}",
                app.status_message
            );
        }
        record(&app, "finish", &mut rows);
        app.save_to_path(&root.join(format!("{name}-native.igz")));
        app.undo();
        record(&app, "finish-undo", &mut rows);
        app.redo();
        record(&app, "finish-redo", &mut rows);
    }
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &root.join("settings-guards"),
    ));
    app.open_document(&root.join("2d-open-input.igz"));
    app.current_file = None;
    app.scene.mesh.vertices[0].position.x = 1. + 2f64.powi(-40);
    app.scene.mesh.edges[0]
        .attrs
        .insert("opaque".into(), json!({"preserve":[1,2]}));
    let protected = app.scene.mesh.clone();
    let before = super::nested_import::source_scene(&app);
    let history = (app.undo_stack.len(), app.redo_stack.len());
    let version = app.scene.version;
    app.begin_unit_import(&root.join("2d-open.dxf")).unwrap();
    app.cancel_unit_import();
    assert_eq!(super::nested_import::source_scene(&app), before);
    assert_eq!(app.scene.version, version);
    assert_eq!((app.undo_stack.len(), app.redo_stack.len()), history);
    let bad = std::fs::read_to_string(root.join("2d-open.dxf"))
        .unwrap()
        .replace("SEQEND", "BROKEN");
    std::fs::write(root.join("malformed.dxf"), bad).unwrap();
    app.begin_unit_import(&root.join("malformed.dxf")).unwrap();
    app.document_session.unit_import.as_mut().unwrap().code = "m".into();
    assert!(app.accept_unit_import().is_err());
    assert_eq!(super::nested_import::source_scene(&app), before);
    assert_eq!(app.scene.version, version);
    assert_eq!((app.undo_stack.len(), app.redo_stack.len()), history);
    app.cancel_unit_import();
    app.begin_unit_import(&root.join("2d-open.dxf")).unwrap();
    app.document_session.unit_import.as_mut().unwrap().code = "m".into();
    app.accept_unit_import().unwrap();
    assert_eq!(app.scene.mesh, protected);
    app.undo();
    assert_eq!(app.scene.mesh, protected);
    app.redo();
    assert_eq!(app.scene.mesh, protected);
    std::fs::write(root.join("native-dxf-guards.json"),serde_json::to_vec_pretty(&json!({"cancel_preserves_document_history_revision":true,"malformed_sequence_preserves_document_history_revision":true,"native_f64_and_opaque_payload_preserved":true})).unwrap()).unwrap();
    std::fs::write(
        root.join("native-dxf-entities.json"),
        serde_json::to_vec_pretty(&json!({"cases":source["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
}
