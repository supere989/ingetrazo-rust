use super::*;
fn normalized(mut value: Value, app: &IngeTrazoApp) -> Value {
    value["units"] = json!({"length":ingetrazo_core::units::model_unit(&app.scene),"precision":ingetrazo_core::units::model_precision(&app.scene)});
    fn walk(v: &mut Value, assets: &std::collections::HashMap<String, Vec<u8>>) {
        if let Some(object) = v.as_object_mut() {
            if let Some(tex) = object.get_mut("texture").and_then(Value::as_object_mut) {
                if let Some(path) = tex.get("path").and_then(Value::as_str) {
                    let bytes = assets
                        .get(path)
                        .cloned()
                        .unwrap_or_else(|| std::fs::read(path).expect("existing texture bytes"));
                    let name = std::path::Path::new(path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy();
                    let name = if name.len() > 17
                        && name.as_bytes()[16] == b'-'
                        && name[..16].bytes().all(|b| b.is_ascii_hexdigit())
                    {
                        &name[17..]
                    } else {
                        &name
                    };
                    tex.insert(
                        "path".into(),
                        json!({"name":name,"sha256":sha256(&bytes),"bytes":bytes.len()}),
                    );
                }
            }
            for v in object.values_mut() {
                walk(v, assets)
            }
        } else if let Some(a) = v.as_array_mut() {
            for v in a {
                walk(v, assets)
            }
        }
    }
    walk(&mut value, &app.scene.embedded_textures);
    value
}
fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn record(app: &IngeTrazoApp, name: &str, phase: &str, rows: &mut Vec<Value>) {
    rows.push(json!({"case":name,"phase":phase,"scene":normalized(super::nested_import::source_scene(app),app),"depth":app.component_edit_stack.len(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len()}));
}
pub fn run() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(root.join("original-obj-mtl.json")).unwrap())
            .unwrap();
    let mut rows = Vec::new();
    let mut guards = Vec::new();
    for case in original["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &root.join(format!("native-settings-{name}")),
        ));
        app.open_document(&root.join(format!("{name}-input.igz")));
        app.current_file = None;
        if case["context"] == "leaf" {
            for _ in 0..2 {
                app.scene.select(SelectionItem::Group(0));
                app.begin_component_edit();
            }
        }
        record(&app, name, "before", &mut rows);
        app.begin_unit_import(&root.join(case["file"].as_str().unwrap()))
            .unwrap();
        app.document_session.unit_import.as_mut().unwrap().code = "m".into();
        app.accept_unit_import().unwrap();
        record(&app, name, "import", &mut rows);
        let image_count = app.scene.embedded_textures.len();
        if name == "multipart-root" {
            assert_eq!(image_count, 1);
        }
        app.undo();
        record(&app, name, "undo", &mut rows);
        app.redo();
        record(&app, name, "redo", &mut rows);
        while !app.component_edit_stack.is_empty() {
            let depth = app.component_edit_stack.len();
            app.finish_component_edit();
            assert_eq!(app.component_edit_stack.len(), depth - 1);
        }
        record(&app, name, "finish", &mut rows);
        // Imported bytes remain owned after the external asset disappears.
        let external = root.join("paint with spaces.png");
        let held = root.join("temporary-held-paint.png");
        if name == "windows-options-leaf" {
            std::fs::rename(&external, &held).unwrap();
        }

        let output = root.join(format!("{name}-native.igz"));
        if output.exists() {
            std::fs::remove_file(&output).unwrap();
        }
        app.save_to_path(&output);
        assert!(output.is_file(), "{}", app.status_message);
        if name == "windows-options-leaf" {
            std::fs::rename(&held, &external).unwrap();
        }
        guards.push(json!({"case":name,"owned_assets":image_count}));
    }
    let mut resource_guard = IngeTrazoApp::new();
    resource_guard.system_clipboard = None;
    resource_guard.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &root.join("resource-cancel-settings"),
    ));
    resource_guard.open_document(&root.join("multipart-leaf-input.igz"));
    for _ in 0..2 {
        resource_guard.scene.select(SelectionItem::Group(0));
        resource_guard.begin_component_edit();
    }
    resource_guard
        .begin_unit_import(&root.join("multipart-leaf.obj"))
        .unwrap();
    resource_guard
        .document_session
        .unit_import
        .as_mut()
        .unwrap()
        .code = "m".into();
    resource_guard.accept_unit_import().unwrap();
    for depth in [1, 0] {
        resource_guard.cancel_component_edit();
        assert_eq!(resource_guard.component_edit_stack.len(), depth);
        assert_eq!(resource_guard.scene.embedded_textures.len(), 1);
    }
    assert_eq!(resource_guard.scene.groups.len(), 3);
    let output = root.join("cancel-resource-native.igz");
    if output.exists() {
        std::fs::remove_file(&output).unwrap();
    }
    resource_guard.save_to_path(&output);
    assert!(output.is_file(), "{}", resource_guard.status_message);
    resource_guard.undo();
    assert_eq!(resource_guard.scene.groups.len(), 2);
    resource_guard.redo();
    assert_eq!(resource_guard.scene.groups.len(), 3);
    assert_eq!(resource_guard.scene.embedded_textures.len(), 1);
    std::fs::write(root.join("resource-lifetime-controls.json"),serde_json::to_vec_pretty(&json!({"external_file_removed_before_save":true,"both_cancel_checkpoints_retain_root_asset":true,"root_undo_redo_restores_asset":true})).unwrap()).unwrap();
    let mut app = IngeTrazoApp::new();
    app.system_clipboard = None;
    app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &root.join("invalid-settings"),
    ));
    app.open_document(&root.join("color-leaf-input.igz"));
    for _ in 0..2 {
        app.scene.select(SelectionItem::Group(0));
        app.begin_component_edit();
    }
    let before = app.scene.clone();
    let undo = app.undo_stack.len();
    let redo = app.redo_stack.len();
    let mut errors = Vec::new();
    for control in original["errors"].as_array().unwrap() {
        let name = control["case"].as_str().unwrap();
        app.begin_unit_import(&root.join(format!("{name}.obj")))
            .unwrap();
        let refused = app.accept_unit_import().is_err();
        assert!(refused);
        assert_eq!(app.scene, before);
        assert_eq!((app.undo_stack.len(), app.redo_stack.len()), (undo, redo));
        assert_eq!(app.component_edit_stack.len(), 2);
        errors.push(
            json!({"case":name,"failed":refused,"faces_unchanged":true,"type":control["type"]}),
        );
    }
    std::fs::write(
        root.join("native-obj-mtl.json"),
        serde_json::to_vec_pretty(
            &json!({"cases":original["cases"],"states":rows,"errors":errors}),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join("native-asset-controls.json"),
        serde_json::to_vec_pretty(&guards).unwrap(),
    )
    .unwrap();
}
