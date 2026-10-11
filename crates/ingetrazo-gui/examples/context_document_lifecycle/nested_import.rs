use super::*;
fn source_scene(app: &IngeTrazoApp) -> Value {
    let mut groups = app.component_edit_stack.first().map_or_else(
        || app.scene.groups.clone(),
        |s| s.parked_scene().groups.clone(),
    );
    fn find(gs: &mut [Group], id: uuid::Uuid) -> Option<&mut Group> {
        for g in gs {
            if g.id == id {
                return Some(g);
            }
            if let Some(g) = find(&mut g.children, id) {
                return Some(g);
            }
        }
        None
    }
    for (index, session) in app.component_edit_stack.iter().enumerate() {
        let owner = app
            .component_edit_stack
            .get(index + 1)
            .map_or(&app.scene, |s| s.parked_scene());
        let group = find(&mut groups, session.edited_group_id()).expect("entered owner");
        group.mesh = owner.mesh.clone();
        group.children = owner.groups.clone();
        group.transform = DMat4::IDENTITY;
    }
    fn group(g: &Group, parent: DMat4) -> Value {
        let m = parent * g.transform;
        json!({"name":g.name,"layer":g.layer,"mesh":mesh_state(&g.mesh,m),"children":g.children.iter().map(|g|group(g,m)).collect::<Vec<_>>()})
    }
    let loose = app
        .component_edit_stack
        .first()
        .map_or(&app.scene.mesh, |s| &s.parked_scene().mesh);
    json!({"active":mesh_state(&app.scene.mesh,DMat4::IDENTITY),"loose":mesh_state(loose,DMat4::IDENTITY),"groups":groups.iter().map(|g|group(g,DMat4::IDENTITY)).collect::<Vec<_>>(),"layers":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"units":app.scene.model_units()})
}
fn record(app: &IngeTrazoApp, case: &str, phase: &str, rows: &mut Vec<Value>) {
    rows.push(json!({"case":case,"phase":phase,"depth":app.component_edit_stack.len(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"scene":source_scene(app)}));
}
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("inputs"));
    let original: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-nested-import.json")).unwrap())
            .unwrap();
    let mut rows = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let context = case["context"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("settings-{name}")),
        ));
        app.open_document(&root.join(format!("{name}-input.igz")));
        app.current_file = None;
        if context != "root" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        if context == "leaf" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
        }
        record(&app, name, "before", &mut rows);
        app.begin_unit_import(&root.join(case["file"].as_str().unwrap()))
            .unwrap();
        app.document_session.unit_import.as_mut().unwrap().code =
            case["unit"].as_str().unwrap().into();
        app.accept_unit_import().unwrap();
        record(&app, name, "import", &mut rows);
        app.undo();
        record(&app, name, "undo", &mut rows);
        app.redo();
        record(&app, name, "redo", &mut rows);
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
        record(&app, name, "finish", &mut rows);
        app.save_to_path(&root.join(format!("{name}-native.igz")));
        app.undo();
        record(&app, name, "finish-undo", &mut rows);
        app.redo();
        record(&app, name, "finish-redo", &mut rows);
    }
    let mut guarded = IngeTrazoApp::new();
    guarded.system_clipboard = None;
    guarded.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &root.join("settings-guards"),
    ));
    guarded.open_document(&root.join("leaf-header-input.igz"));
    guarded.current_file = None;
    for _ in 0..2 {
        guarded.scene.select(SelectionItem::Group(0));
        guarded.begin_component_edit();
    }
    guarded
        .scene
        .layers
        .push(ingetrazo_core::layers::Layer::new("Live"));
    let before = source_scene(&guarded);
    let before_version = guarded.scene.version;
    let before_history = (guarded.undo_stack.len(), guarded.redo_stack.len());
    let bad = root.join("invalid-native.obj");
    std::fs::write(&bad, "v bad 0 0\n").unwrap();
    guarded.begin_unit_import(&bad).unwrap();
    let refused = guarded.accept_unit_import().is_err();
    assert!(refused);
    assert_eq!(source_scene(&guarded), before);
    assert_eq!(guarded.scene.version, before_version);
    assert_eq!(
        (guarded.undo_stack.len(), guarded.redo_stack.len()),
        before_history
    );
    guarded.begin_unit_import(&root.join("header.dxf")).unwrap();
    guarded.cancel_unit_import();
    assert_eq!(source_scene(&guarded), before);
    guarded.begin_unit_import(&root.join("header.dxf")).unwrap();
    guarded.document_session.unit_import.as_mut().unwrap().code = "mm".into();
    guarded.accept_unit_import().unwrap();
    let imported = source_scene(&guarded);
    assert_eq!(imported["groups"].as_array().unwrap().len(), 3);
    assert!(guarded.scene.layers.iter().any(|l| l.name == "Live"));
    for depth in [1, 0] {
        guarded.cancel_component_edit();
        assert_eq!(guarded.component_edit_stack.len(), depth);
        assert_eq!(
            source_scene(&guarded)["groups"].as_array().unwrap().len(),
            3
        );
        assert_eq!(guarded.undo_stack.len(), 1);
    }
    guarded.undo();
    assert_eq!(guarded.scene.groups.len(), 2);
    assert!(guarded.scene.layers.iter().any(|l| l.name == "Live"));
    guarded.redo();
    assert_eq!(guarded.scene.groups.len(), 3);
    std::fs::write(root.join("guards.json"),serde_json::to_vec_pretty(&json!({"malformed_import_preserves_document_history_context":refused,"unit_cancel_preserves_document":true,"root_import_survives_both_cancel_checkpoints":true,"root_command_undo_redo_after_cancel":true,"live_layer_preserved":true})).unwrap()).unwrap();
    let mut thresholds = Vec::new();
    for count in [400, 401] {
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("settings-threshold-{count}")),
        ));
        app.open_document(&root.join(format!("threshold-{count}-input.igz")));
        app.current_file = None;
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
        let mut capture = |app: &IngeTrazoApp, phase: &str| {
            let shape = source_scene(app);
            thresholds.push(json!({"case":count,"phase":phase,"active_faces":app.scene.mesh.faces.len(),"groups":shape["groups"].as_array().unwrap().iter().map(|g|json!({"name":g["name"],"faces":g["mesh"]["faces"].as_array().unwrap().len(),"children":g["children"].as_array().unwrap().len()})).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()}));
        };
        capture(&app, "before");
        app.begin_unit_import(&root.join(format!("threshold-{count}.obj")))
            .unwrap();
        app.document_session.unit_import.as_mut().unwrap().code = "m".into();
        app.accept_unit_import().unwrap();
        capture(&app, "import");
        app.undo();
        capture(&app, "undo");
        app.redo();
        capture(&app, "redo");
    }
    std::fs::write(
        root.join("native-threshold.json"),
        serde_json::to_vec_pretty(&json!({"states":thresholds})).unwrap(),
    )
    .unwrap();
    let mut naming = IngeTrazoApp::new();
    naming.system_clipboard = None;
    naming.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &root.join("settings-naming"),
    ));
    naming.open_document(&root.join("naming-input.igz"));
    naming.current_file = None;
    let mut name_rows = Vec::new();
    let mut capture = |app: &IngeTrazoApp, phase: &str| {
        name_rows.push(json!({"phase":phase,"names":app.scene.groups.iter().map(|g|&g.name).collect::<Vec<_>>(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()}));
    };
    capture(&naming, "before");
    for phase in ["first", "second"] {
        naming
            .begin_unit_import(&root.join("naming-large.obj"))
            .unwrap();
        naming.document_session.unit_import.as_mut().unwrap().code = "m".into();
        naming.accept_unit_import().unwrap();
        capture(&naming, phase);
    }
    naming.undo();
    capture(&naming, "undo");
    naming.redo();
    capture(&naming, "redo");
    let original_names: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-naming.json")).unwrap()).unwrap();
    let mut piece_names = Vec::new();
    for (index, control) in original_names["piece_names"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let authored = control["authored"].as_str().unwrap();
        let path = root.join(format!("piece-name-{index}.obj"));
        std::fs::write(&path,format!("v 0 0 0\nv 1 0 0\nv 0 1 0\nv 10 0 0\nv 11 0 0\nv 10 1 0\ng {authored}\nf 1 2 3\ng sentinel\nf 4 5 6\n")).unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("settings-piece-{index}")),
        ));
        app.begin_unit_import(&path).unwrap();
        app.document_session.unit_import.as_mut().unwrap().code = "m".into();
        app.accept_unit_import().unwrap();
        piece_names
            .push(json!({"authored":authored,"result":app.scene.groups[0].children[0].name}));
    }
    std::fs::write(
        root.join("native-naming.json"),
        serde_json::to_vec_pretty(&json!({"states":name_rows,"piece_names":piece_names})).unwrap(),
    )
    .unwrap();
    // Exhaustion is a refusal, never a wrapping reuse of an automatic name.
    ingetrazo_core::groups::reserve_source_group_names(&[ingetrazo_core::groups::Group::new(
        format!("Group {}", u64::MAX),
    )]);
    let before = source_scene(&naming);
    let history = (naming.undo_stack.len(), naming.redo_stack.len());
    let version = naming.scene.version;
    naming
        .begin_unit_import(&root.join("naming-large.obj"))
        .unwrap();
    naming.document_session.unit_import.as_mut().unwrap().code = "m".into();
    let error = naming.accept_unit_import().unwrap_err();
    assert!(error.contains("exhausted"));
    assert_eq!(source_scene(&naming), before);
    assert_eq!((naming.undo_stack.len(), naming.redo_stack.len()), history);
    assert_eq!(naming.scene.version, version);
    let mut guards: Value =
        serde_json::from_slice(&std::fs::read(root.join("guards.json")).unwrap()).unwrap();
    guards["automatic_name_exhaustion_preserves_document_history"] = json!(true);
    std::fs::write(
        root.join("guards.json"),
        serde_json::to_vec_pretty(&guards).unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("native-nested-import.json"),
        serde_json::to_vec_pretty(&json!({"cases":original["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
}
