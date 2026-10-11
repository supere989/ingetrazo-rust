use super::*;
fn flags(app: &IngeTrazoApp) -> Value {
    fn mesh(m: &ingetrazo_core::mesh::Mesh) -> Value {
        let mut rows = m
            .edges
            .iter()
            .map(|e| {
                let mut ends = vec![
                    m.vertices[e.v0].position.to_array(),
                    m.vertices[e.v1].position.to_array(),
                ];
                ends.sort_by(|a, b| a.partial_cmp(b).unwrap());
                json!({"ends":ends,"soft":e.soft})
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| a["ends"].to_string().cmp(&b["ends"].to_string()));
        json!(rows)
    }
    fn group(g: &ingetrazo_core::groups::Group) -> Value {
        json!({"mesh":mesh(&g.mesh),"children":g.children.iter().map(group).collect::<Vec<_>>()})
    }
    json!({"active":mesh(&app.scene.mesh),"groups":app.scene.groups.iter().map(group).collect::<Vec<_>>()})
}
fn observe(app: &IngeTrazoApp, name: &str, phase: &str) -> Value {
    json!({"case":name,"phase":phase,"scene":super::obj_mtl::normalized(super::nested_import::source_scene(app),app),"flags":flags(app),"depth":app.component_edit_stack.len(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()})
}
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-obj-fusion.json")).unwrap())
            .unwrap();
    let mut rows = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("settings-{name}")),
        ));
        app.open_document(&root.join(format!("{name}-input.igz")));
        app.current_file = None;
        if case["context"] == "leaf" {
            for _ in 0..2 {
                app.scene.select(SelectionItem::Group(0));
                app.begin_component_edit();
            }
        }
        rows.push(observe(&app, name, "before"));
        app.begin_unit_import(&root.join(case["file"].as_str().unwrap()))
            .unwrap();
        app.document_session.unit_import.as_mut().unwrap().code = "m".into();
        app.accept_unit_import().unwrap();
        rows.push(observe(&app, name, "import"));
        app.undo();
        rows.push(observe(&app, name, "undo"));
        app.redo();
        rows.push(observe(&app, name, "redo"));
        while !app.component_edit_stack.is_empty() {
            let depth = app.component_edit_stack.len();
            app.finish_component_edit();
            assert_eq!(app.component_edit_stack.len(), depth - 1);
        }
        rows.push(observe(&app, name, "finish"));
        let path = root.join(format!("{name}-native.igz"));
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
        app.save_to_path(&path);
        assert!(path.is_file(), "{}", app.status_message);
    }
    let mut protected = IngeTrazoApp::new();
    protected.system_clipboard = None;
    let x = 100. + 2_f64.powi(-40);
    let ids = [
        DVec3::new(x, 0., 0.),
        DVec3::new(x + 2., 0., 0.),
        DVec3::new(x, 2., 0.),
    ]
    .map(|p| protected.scene.mesh.add_vertex(p));
    let id = protected.scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
    protected.scene.mesh.faces[id].layer = Some("native-layer".into());
    protected.scene.mesh.faces[id]
        .attrs
        .insert("vendor".into(), json!({"opaque":[1,"native"]}));
    protected.scene.mesh.edges[0]
        .attrs
        .insert("vendor".into(), json!({"edge":true}));
    protected.scene.mesh.edges[0].curve_id = Some(777);
    protected.scene.mesh.edges[0].hidden = true;
    let face = protected.scene.mesh.faces[0].clone();
    let edge = protected.scene.mesh.edges[0].clone();
    let vertex = protected.scene.mesh.vertices[0].clone();
    protected
        .begin_unit_import(&root.join("closed-cube-root.obj"))
        .unwrap();
    protected
        .document_session
        .unit_import
        .as_mut()
        .unwrap()
        .code = "m".into();
    protected.accept_unit_import().unwrap();
    let held = protected
        .scene
        .mesh
        .faces
        .iter()
        .find(|f| f.history_id == face.history_id)
        .unwrap();
    assert_eq!(held.attrs, face.attrs);
    assert_eq!(held.layer, face.layer);
    let held = protected
        .scene
        .mesh
        .edges
        .iter()
        .find(|e| e.history_id == edge.history_id)
        .unwrap();
    assert_eq!(held.attrs, edge.attrs);
    assert_eq!(held.curve_id, edge.curve_id);
    assert_eq!(held.hidden, edge.hidden);
    let held = protected
        .scene
        .mesh
        .vertices
        .iter()
        .find(|v| v.uid == vertex.uid)
        .unwrap();
    assert_eq!(held.position, vertex.position);
    let before = protected.scene.clone();
    let undo = protected.undo_stack.len();
    let redo = protected.redo_stack.len();
    let invalid = root.join("nonfinite-fusion.obj");
    std::fs::write(&invalid, "v inf 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    protected.begin_unit_import(&invalid).unwrap();
    protected
        .document_session
        .unit_import
        .as_mut()
        .unwrap()
        .code = "m".into();
    assert!(protected.accept_unit_import().is_err());
    assert_eq!(protected.scene, before);
    assert_eq!(
        (protected.undo_stack.len(), protected.redo_stack.len()),
        (undo, redo)
    );
    std::fs::write(
        root.join("native-fusion-controls.json"),
        serde_json::to_vec_pretty(
            &json!({"native_f64_and_owner_metadata_preserved":true,"invalid_import_atomic":true}),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("native-obj-fusion.json"),
        serde_json::to_vec_pretty(&json!({"cases":original["cases"],"states":rows})).unwrap(),
    )
    .unwrap();
}
