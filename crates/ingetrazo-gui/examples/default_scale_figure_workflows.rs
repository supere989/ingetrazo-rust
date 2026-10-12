//! Actual new-document defaults/preferences, resource bytes and saved ownership.
use ingetrazo_gui::{
    IngeTrazoApp,
    session_state::{Preferences, SessionStore},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn resource(
    app: &IngeTrazoApp,
    key: &str,
    out: &Path,
    resources: &mut serde_json::Map<String, Value>,
) -> Result<Value, String> {
    let encoded = app
        .scene
        .embedded_textures
        .get(key)
        .ok_or("Actual owned figure resource missing")?;
    let image = ingetrazo_render::source_texture_decode::decode(encoded, Default::default())?;
    let hash = digest(encoded);
    let rgba = digest(&image.rgba);
    let file = format!("assets/{hash}.png");
    let raw = format!("assets/{hash}.rgba");
    if !resources.contains_key(&hash) {
        std::fs::write(out.join(&file), encoded).map_err(|e| e.to_string())?;
        std::fs::write(out.join(&raw), &image.rgba).map_err(|e| e.to_string())?;
        resources.insert(hash.clone(),json!({"file":file,"rgba":raw,"encoded_sha256":hash,"rgba_sha256":rgba,"size":[image.width,image.height]}));
    }
    Ok(json!({"encoded_sha256":hash,"rgba_sha256":rgba,"size":[image.width,image.height]}))
}
fn observe(
    app: &IngeTrazoApp,
    out: &Path,
    resources: &mut serde_json::Map<String, Value>,
) -> Result<Value, String> {
    let mut groups = Vec::new();
    for g in &app.scene.groups {
        let mut faces = Vec::new();
        for f in &g.mesh.faces {
            let mut attrs = json!(f.attrs);
            if let Some(texture) = attrs.get_mut("texture") {
                if let Some(key) = texture["path"].as_str().map(str::to_owned) {
                    texture["path"] = resource(app, &key, out, resources)?;
                }
            }
            faces.push(json!({"outer":f.vertices.iter().map(|i|g.mesh.vertices[*i].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|i|g.mesh.vertices[*i].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),"attrs":attrs}));
        }
        let component = g.native_document["component"].as_bool().unwrap_or(true);
        let placement = !g.children.is_empty()
            || g.transform != glam::DMat4::IDENTITY
            || g.component_definition.is_some()
            || g.native_document.get("xform").is_some_and(|v| !v.is_null());
        groups.push(json!({"name":g.name,"component":component,"is_component":placement&&component,"billboard":g.billboard,"hidden":g.hidden,"layer":g.layer,"children":g.children.len(),"faces":faces}));
    }
    Ok(
        json!({"groups":groups,"loose_faces":app.scene.mesh.faces.len(),"images":app.scene.image_planes.len()}),
    )
}
fn setup(setting: &Value, root: &Path) -> Result<IngeTrazoApp, String> {
    let store = SessionStore::isolated(root);
    let mut prefs = Preferences::default();
    if !setting.is_null() {
        prefs
            .extra
            .insert("new_document/scale_figure".into(), setting.clone());
    }
    store.save_preferences(&prefs)?;
    let mut app = IngeTrazoApp::new();
    app.attach_session_store(store);
    Ok(app)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(std::env::args().nth(1).ok_or("SOURCE required")?).canonicalize()?;
    let out = PathBuf::from(std::env::args().nth(2).ok_or("OUTPUT required")?);
    std::fs::create_dir_all(out.join("assets"))?;
    let original: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-default-figure.json"))?)?;
    assert_eq!(
        original["source_pin"],
        "6be29fe437a7cd992d4e079f71720821b25216b9"
    );
    let mut cases = Vec::new();
    let mut resources = serde_json::Map::new();
    for recipe in original["cases"].as_array().ok_or("cases required")? {
        let name = recipe["name"].as_str().ok_or("name required")?;
        let mut app = setup(&recipe["setting"], &out.join("settings").join(name))?;
        assert!(!app.document_dirty());
        assert!(app.undo_stack.is_empty() && app.redo_stack.is_empty());
        let state = observe(&app, &out, &mut resources)?;
        app.engineering_python_enabled = true;
        let history_before = (app.undo_stack.len(), app.redo_stack.len());
        let dirty_before = app.document_dirty();
        let log_start = app.console_logs.len();
        app.run_script("print([(g.component, g.is_component()) for g in groups])");
        let query_logs = app.console_logs[log_start..].to_vec();
        assert!(
            !query_logs.iter().any(|line| line.contains("refused")),
            "Actual component query refused: {query_logs:?}"
        );
        assert_eq!((app.undo_stack.len(), app.redo_stack.len()), history_before);
        assert_eq!(
            app.document_dirty(),
            dirty_before,
            "Read-only component query changed clean state"
        );
        let expected_query = if app.scene.groups.is_empty() {
            "[]\n"
        } else {
            "[(True, False)]\n"
        };
        assert!(
            query_logs.iter().any(|line| line == expected_query),
            "Actual component query: {query_logs:?}"
        );
        let saved = format!("{name}-saved.igz");
        app.save_to_path(&out.join(&saved));
        assert!(out.join(&saved).is_file());
        let mut reopened = IngeTrazoApp::new();
        reopened.open_document(&out.join(&saved));
        assert!(reopened.current_file.is_some());
        let reopen = observe(&reopened, &out, &mut resources)?;
        cases.push(json!({"name":name,"setting":recipe["setting"],"state":state,"clean":!app.document_dirty(),"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"saved":saved,"reopen":reopen,"component_query":query_logs,"identity_diagnostic":app.scene.groups.iter().map(|g|g.id.to_string()).collect::<Vec<_>>()}));
    }
    // Native controls use actual constructor, durable preference reload and New.
    let mut edited = IngeTrazoApp::new();
    edited.scene.mesh.add_vertex(glam::DVec3::X);
    let before = edited.scene.clone();
    let store = SessionStore::isolated(&out.join("settings/edited"));
    let mut disabled = Preferences::default();
    disabled
        .extra
        .insert("new_document/scale_figure".into(), json!("0"));
    store.save_preferences(&disabled)?;
    edited.attach_session_store(store);
    assert_eq!(edited.scene.groups, before.groups);
    assert_eq!(edited.scene.mesh, before.mesh);
    assert_eq!(edited.scene.embedded_textures, before.embedded_textures);
    let mut changing = setup(&Value::Null, &out.join("settings/new"))?;
    changing
        .document_session
        .preferences
        .extra
        .insert("new_document/scale_figure".into(), json!("0"));
    changing.new_document();
    assert!(changing.scene.groups.is_empty());
    assert!(!changing.document_dirty());
    assert!(changing.undo_stack.is_empty() && changing.redo_stack.is_empty());
    changing
        .document_session
        .preferences
        .extra
        .insert("new_document/scale_figure".into(), json!("1"));
    changing.new_document();
    assert_eq!(changing.scene.groups.len(), 1);
    assert!(!changing.document_dirty());
    assert!(changing.undo_stack.is_empty() && changing.redo_stack.is_empty());
    let mut loaded = IngeTrazoApp::new();
    loaded.open_document(&out.join("default-saved.igz"));
    let loaded_before = loaded.scene.clone();
    let store = SessionStore::isolated(&out.join("settings/loaded"));
    store.save_preferences(&disabled)?;
    loaded.attach_session_store(store);
    assert_eq!(loaded.scene.groups, loaded_before.groups);
    assert_eq!(
        loaded.scene.embedded_textures,
        loaded_before.embedded_textures
    );
    let report = json!({"source_pin":original["source_pin"],"cases":cases,"resources":resources,"controls":{"startup_clean":true,"history_empty":true,"owned_asset":true,"raw_false_enabled":true,"disabled_pristine_reconcile":true,"new_document_preference":true,"edited_startup_not_removed":true,"loaded_document_marker_retired":true}});
    std::fs::write(
        out.join("native-default-figure.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
