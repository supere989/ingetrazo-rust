#[path="context_document_lifecycle/dxf_entities.rs"]
mod dxf_entities;
#[path = "context_document_lifecycle/appearance.rs"]
mod appearance;
use glam::{DMat4, DVec3};
use ingetrazo_core::{
    groups::Group,
    math::Ray,
    mesh::Mesh,
    scene::{Scene, SelectionItem},
};
use ingetrazo_gui::{document_session::DocumentAction, IngeTrazoApp};
use ingetrazo_tools::base::{ToolContext, ToolType};
use serde_json::{json, Value};
use std::path::Path;
fn mesh_state(mesh: &Mesh, m: DMat4) -> Value {
    let point = |i: usize| m.transform_point3(mesh.vertices[i].position).to_array();
    let ring = |ids: &Vec<usize>| {
        let p = ids.iter().map(|i| point(*i)).collect::<Vec<_>>();
        (0..p.len())
            .map(|i| p[i..].iter().chain(&p[..i]).copied().collect::<Vec<_>>())
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    };
    let mut points = (0..mesh.vertices.len()).map(point).collect::<Vec<_>>();
    points.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut edges = mesh
        .edges
        .iter()
        .map(|e| {
            let mut p = [point(e.v0), point(e.v1)];
            p.sort_by(|a, b| a.partial_cmp(b).unwrap());
            p
        })
        .collect::<Vec<_>>();
    edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut faces = mesh
        .faces
        .iter()
        .map(|f| {
            let mut holes = f.holes.iter().map(&ring).collect::<Vec<_>>();
            holes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            json!({"outer":ring(&f.vertices),"holes":holes,"attrs":f.attrs})
        })
        .collect::<Vec<_>>();
    faces.sort_by_key(|f| f.to_string());
    json!({"points":points,"edges":edges,"faces":faces})
}
fn state(s: &Scene) -> Value {
    fn visit(g: &Group, parent: DMat4, rows: &mut Vec<Value>) {
        let m = parent * g.transform;
        rows.push(json!({"name":g.name,"uid":g.native_uid(),"mesh":mesh_state(&g.mesh,m),"ext":g.native_document.get("ext").cloned().unwrap_or(Value::Null)}));
        for c in &g.children {
            visit(c, m, rows);
        }
    }
    let mut rows = vec![];
    for g in &s.groups {
        visit(g, DMat4::IDENTITY, &mut rows);
    }
    json!({"root_mesh":mesh_state(&s.mesh,DMat4::IDENTITY),"groups":rows})
}
fn setup(out: &Path) -> IngeTrazoApp {
    let mut a = IngeTrazoApp::new();
    a.system_clipboard = None;
    a.set_language("en");
    a.open_document(&out.join("input.igz"));
    a.current_file = None;
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    assert_eq!(a.component_edit_stack.len(), 2);
    a.set_tool(ToolType::Rectangle);
    a.save_undo_state();
    for v in &mut a.scene.mesh.vertices {
        v.position.z += 2.;
    }
    a.camera.target = DVec3::new(7., 8., 9.);
    a.camera.distance = 23.;
    a
}
fn snapshot(a: &IngeTrazoApp, phase: &str) -> Value {
    json!({"phase":phase,"state":state(&a.scene),"depth":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"tool":format!("{:?}",a.active_tool_type).to_lowercase(),"camera_target":a.camera.target.to_array(),"camera_distance":a.camera.distance})
}
fn frame(a: &mut IngeTrazoApp, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1000., 700.),
    ));
    input.events = events;
    ctx.run(input, |ctx| {
        a.document_session_ui(ctx);
        ingetrazo_gui::dialogs::render_dialogs(a, ctx);
    })
}
fn click(a: &mut IngeTrazoApp, ctx: &egui::Context, label: &str) {
    frame(a, ctx, vec![]);
    let output = frame(a, ctx, vec![]);
    let ps = output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                if t.galley.text() == label {
                    return Some(t.pos + t.galley.rect.center().to_vec2());
                }
                None
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(ps.len(), 1, "button {label}");
    let pos = ps[0];
    for pressed in [true, false] {
        frame(
            a,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
#[path="context_document_lifecycle/nested_import.rs"]
mod nested_import;
#[path="context_document_lifecycle/obj_mtl.rs"]
mod obj_mtl;
#[path="context_document_lifecycle/obj_fusion.rs"]
mod obj_fusion;
fn main() {
    if std::env::args().any(|a|a=="--obj-raw-fusion") {obj_fusion::run_raw();return;}
    if std::env::args().any(|a|a=="--obj-fusion") {obj_fusion::run();return;}
    if std::env::args().any(|a|a=="--obj-mtl") {obj_mtl::run();return;}
    if std::env::args().any(|a|a=="--dxf-blocks-addendum"){dxf_entities::run_blocks_addendum();return;}
    if std::env::args().any(|a|a=="--dxf-blocks"){dxf_entities::run_blocks();return;}
    if std::env::args().any(|a|a=="--dxf-splines"){dxf_entities::run_splines();return;}
    if std::env::args().any(|a|a=="--dxf-curves"){dxf_entities::run_curves();return;}
    if std::env::args().any(|a|a=="--dxf-entities"){dxf_entities::run();return;}
    if std::env::args().any(|a|a=="--nested-import"){nested_import::run();return;}

    if std::env::args().any(|a| a == "--appearance") {
        appearance::run();
        return;
    }
    if std::env::args().any(|a| a == "--import-preferences") {
        import_preferences_acceptance();
        return;
    }
    if std::env::args().any(|a| a == "--units") {
        units_acceptance();
        return;
    }
    if std::env::args().any(|a| a == "--preferences") {
        preferences_acceptance();
        return;
    }
    if std::env::args().any(|a| a == "--recent") {
        recent_acceptance();
        return;
    }
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut rows = vec![];
    for case in [
        "new_cancel",
        "open_cancel",
        "open_picker_cancel",
        "open_discard",
        "open_failed",
        "new_discard",
        "close_cancel",
        "close_discard",
        "close_save",
        "close_save_cancel",
        "close_save_failed",
    ] {
        let mut a = setup(&out);
        let ctx = egui::Context::default();
        let path = out.join(format!("{case}.native.igz"));
        if case == "close_save" {
            a.current_file = Some(path.clone());
        } else if case == "close_save_failed" {
            a.current_file = Some(out.join("missing/failed.igz"));
        }
        if case.starts_with("new") {
            a.new_document();
        } else if case.starts_with("open") {
            a.begin_open_document();
        } else {
            a.request_document_action(DocumentAction::Close);
        }
        assert!(a.document_session.confirm_unsaved, "{case}");
        let prompts = vec![a.component_edit_stack.len()];
        let mut dialogs = vec![];
        let choice = if ["new_cancel", "open_cancel", "close_cancel"].contains(&case) {
            "Cancel"
        } else if case.starts_with("close_save") {
            "Save"
        } else {
            "Discard"
        };
        click(&mut a, &ctx, choice);
        if a.show_open_dialog {
            dialogs.push(a.component_edit_stack.len());
            if case == "open_picker_cancel" {
                click(&mut a, &ctx, "Cancel");
            } else {
                a.dialog_path_input = out
                    .join(if case == "open_failed" {
                        "corrupt.igz"
                    } else {
                        "replacement.igz"
                    })
                    .display()
                    .to_string();
                click(&mut a, &ctx, "Open");
            }
        }
        if a.show_save_dialog {
            assert_eq!(case, "close_save_cancel");
            dialogs.push(a.component_edit_stack.len());
            click(&mut a, &ctx, "Cancel");
        }
        let error = a.status_message.starts_with("Failed to open")
            || a.status_message.starts_with("Save failed");
        let closed = a.document_session.close_allowed;
        let prompt_open = a.document_session.confirm_unsaved;
        let mut phases = vec![snapshot(&a, "after")];
        if !closed {
            a.undo();
            phases.push(snapshot(&a, "undo"));
            a.redo();
            phases.push(snapshot(&a, "redo"));
        }
        let final_document = if !a.component_edit_stack.is_empty() {
            a.save_document_as();
            a.show_save_dialog = false;
            Some(state(&a.scene))
        } else {
            None
        };
        rows.push(json!({"case":case,"phases":phases,"prompt_depths":prompts,"dialog_depths":dialogs,"closed":closed,"error":error,"file_exists":path.exists(),"prompt_open":prompt_open,"final_document":final_document}));
    }
    std::fs::write(
        out.join("native.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .unwrap();
    let mut pending = vec![];
    for (tool, chain) in [
        (ToolType::Line, false),
        (ToolType::Line, true),
        (ToolType::Rectangle, false),
    ] {
        let mut a = setup(&out);
        a.set_tool(tool);
        let context = |p| ToolContext::new(p, Ray::new(p + DVec3::Z * 10., -DVec3::Z));
        a.dispatch_tool_click(&context(DVec3::new(30., 40., 50.)));
        if chain {
            a.dispatch_tool_click(&context(DVec3::new(31., 40., 50.)));
        }
        a.dispatch_tool_move(&context(DVec3::new(32., 41., 50.)));
        let name = format!(
            "{}-{}",
            format!("{tool:?}").to_lowercase(),
            if chain { "chain" } else { "first" }
        );
        a.current_file = Some(out.join(format!("{name}.native.igz")));
        a.save_document();
        let mut phases = vec![snapshot(&a, "after")];
        let anchor = a.active_tool.snap_anchor().map(|p| p.to_array());
        a.dispatch_tool_click(&context(DVec3::new(32., 41., 50.)));
        phases.push(snapshot(&a, "continued"));
        a.undo();
        phases.push(snapshot(&a, "undo"));
        a.redo();
        phases.push(snapshot(&a, "redo"));
        pending.push(json!({"case":name,"anchor_after":anchor,"phases":phases}));
    }
    std::fs::write(
        out.join("native-pending.json"),
        serde_json::to_vec_pretty(&json!({"rows":pending})).unwrap(),
    )
    .unwrap();
}

fn recent_acceptance() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let store = root.join("native-settings");
    let mut a = IngeTrazoApp::new();
    a.set_language("en");
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    a.clear_recent_files();
    let mut rows = Vec::new();
    let record = |a: &IngeTrazoApp, phase: &str| json!({"phase":phase,"recent":a.recent_files.iter().map(|p|p.file_name().unwrap().to_str().unwrap()).collect::<Vec<_>>()});
    for i in 0..12 {
        a.save_to_path(&root.join(format!("doc-{i}.igz")));
    }
    assert!(a
        .recent_files
        .iter()
        .all(|p| p.is_absolute() && std::fs::canonicalize(p).unwrap() == *p));
    rows.push(record(&a, "cap"));
    a.open_recent_document(&root.join("./doc-5.igz"));
    rows.push(record(&a, "alias"));
    let obj = root.join("model.obj");
    std::fs::write(&obj, "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
    a.open_document(&obj);
    assert_eq!(a.current_file.as_ref(), Some(&obj));
    rows.push(record(&a, "import"));
    rows.push(record(&a, "none"));
    // Missing stale entry is checked before active-context finish or dirty prompt.
    let mut group = Group::new("working");
    group.name = "working".into();
    group.mesh.add_vertex(DVec3::X);
    let id = a.scene.groups.len();
    a.scene.groups.push(group);
    a.scene.select(SelectionItem::Group(id));
    a.begin_component_edit();
    assert_eq!(a.component_edit_stack.len(), 1);
    a.scene.mesh.add_vertex(DVec3::Y);
    a.save_undo_state();
    let before = a.scene.clone();
    let depth = a.component_edit_stack.len();
    let history = (a.undo_stack.len(), a.redo_stack.len());
    std::fs::remove_file(root.join("doc-5.igz")).unwrap();
    a.open_recent_document(&root.join("doc-5.igz"));
    assert_eq!(a.scene, before);
    assert_eq!(a.component_edit_stack.len(), depth);
    assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
    assert!(!a.document_session.confirm_unsaved);
    assert!(a.document_session.recent_missing.is_some());
    rows.push(record(&a, "missing-filter-retains"));
    a.open_recent_document(&root.join("doc-11.igz"));
    assert!(a.component_edit_stack.is_empty());
    assert!(a.document_session.confirm_unsaved);
    let completed = a.scene.clone();
    a.cancel_document_action();
    assert_eq!(a.scene, completed);
    a.open_recent_document(&root.join("doc-11.igz"));
    a.discard_document_action();
    assert_eq!(a.current_file.as_ref(), Some(&root.join("doc-11.igz")));
    assert!(a.undo_stack.is_empty());
    let before = a.scene.clone();
    let history = (a.undo_stack.len(), a.redo_stack.len());
    a.clear_recent_files();
    assert_eq!(a.scene, before);
    assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
    rows.push(record(&a, "clear"));
    let mut restarted = IngeTrazoApp::new();
    restarted.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    assert!(restarted.recent_files.is_empty());
    std::fs::write(
        root.join("native-recent.json"),
        serde_json::to_vec_pretty(
            &json!({"rows":rows,"restart":restarted.recent_files,"app_preservation":true}),
        )
        .unwrap(),
    )
    .unwrap();
}

fn preferences_acceptance() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let store = root.join("native-settings");
    let mut a = IngeTrazoApp::new();
    a.set_language("en");
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    a.document_session.preferences = Default::default();
    a.document_session
        .preferences
        .extra
        .insert("proof/opaque".into(), json!("vendor-preserved"));
    a.persist_preferences();
    for _ in 0..3 {
        a.save_undo_state();
    }
    let mut rows = vec![];
    let mut arms = 0;
    let record = |a: &IngeTrazoApp, phase: &str, arms: usize| {
        let p = &a.document_session.preferences;
        json!({"phase":phase,"autosave":p.autosave,"minutes":p.autosave_minutes,"undo_cap":p.undo_steps,"look":p.look_sensitivity,"wheel":p.extra.get("nav/invert_wheel").and_then(Value::as_bool).unwrap_or(false),"orbit":p.extra.get("nav/invert_orbit_y").and_then(Value::as_bool).unwrap_or(false),"section":p.extra.get("section_ask_name").and_then(Value::as_bool).unwrap_or(true),"opaque":p.extra.get("proof/opaque"),"undo":a.undo_stack.len(),"arms":arms})
    };
    let configure = |a: &mut IngeTrazoApp, cap| {
        let p = &mut a
            .document_session
            .preferences_draft
            .as_mut()
            .unwrap()
            .preferences;
        p.autosave = false;
        p.autosave_minutes = 7;
        p.undo_steps = cap;
        p.look_sensitivity = 47;
        for (key, value) in [
            ("nav/invert_wheel", true),
            ("nav/invert_orbit_y", true),
            ("section_ask_name", false),
        ] {
            p.extra.insert(key.into(), json!(value));
        }
    };
    rows.push(record(&a, "before", arms));
    let before = a.scene.clone();
    let settings = std::fs::read(store.join("config/settings.json")).unwrap();
    let timer = a.document_session.last_autosave;
    a.begin_preferences();
    configure(&mut a, 0);
    rows.push(record(&a, "draft", arms));
    a.cancel_preferences();
    assert_eq!(a.scene, before);
    assert_eq!(
        std::fs::read(store.join("config/settings.json")).unwrap(),
        settings
    );
    assert_eq!(a.document_session.last_autosave, timer);
    rows.push(record(&a, "cancel", arms));
    a.begin_preferences();
    configure(&mut a, 0);
    a.accept_preferences();
    arms += usize::from(a.document_session.last_autosave != timer);
    rows.push(record(&a, "accept", arms));
    for _ in 0..2 {
        a.save_undo_state();
    }
    rows.push(record(&a, "unlimited", arms));
    let timer = a.document_session.last_autosave;
    a.begin_preferences();
    a.document_session
        .preferences_draft
        .as_mut()
        .unwrap()
        .preferences
        .undo_steps = 2;
    a.accept_preferences();
    arms += usize::from(a.document_session.last_autosave != timer);
    rows.push(record(&a, "lower-no-prune", arms));
    a.save_undo_state();
    rows.push(record(&a, "next-command-pruned", arms));
    let mut restarted = IngeTrazoApp::new();
    restarted.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    restarted.begin_preferences();
    configure(&mut restarted, 0);
    restarted.cancel_preferences();
    rows.push(record(&restarted, "restart-cancel", 0));
    std::fs::write(
        root.join("native-preferences.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .unwrap();
}

fn units_acceptance() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let store = root.join("native-unit-settings");
    let mut a = IngeTrazoApp::new();
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    a.open_document(&root.join("input.igz"));
    assert!(!a.document_session.confirm_unsaved);
    // A raw unknown unit key survives no-op, but explicit Apply replaces the pair,
    // including an away-and-back change equal to the imported baseline.
    let mut vendor =
        serde_json::from_slice::<Value>(&std::fs::read(root.join("input.igz")).unwrap()).unwrap();
    vendor["scene"]["units"]["vendor"] = json!("original-unknown");
    std::fs::write(
        root.join("vendor-input.igz"),
        serde_json::to_vec(&vendor).unwrap(),
    )
    .unwrap();
    let mut v = IngeTrazoApp::new();
    v.open_document(&root.join("vendor-input.igz"));
    v.save_to_path(&root.join("vendor-noop-native.igz"));
    v.apply_model_units(json!({"length":"cm","precision":6}));
    v.apply_model_units(json!({"length":"ft-in","precision":4}));
    v.save_to_path(&root.join("away-back-native.igz"));
    let mut rows = Vec::new();
    let record = |a: &mut IngeTrazoApp, phase: &str| {
        a.begin_preferences();
        let checkbox = a
            .document_session
            .preferences_draft
            .as_ref()
            .unwrap()
            .units_for_new;
        a.cancel_preferences();
        json!({"phase":phase,"units":{"length":ingetrazo_core::units::model_unit(&a.scene),"precision":ingetrazo_core::units::model_precision(&a.scene)},"style":{"units":a.scene.dimension_style()["units"],"decimals":a.scene.dimension_style()["decimals"],"vendor":a.scene.dimension_style()["vendor"]},"default":a.document_session.preferences.new_document_units(),"checkbox":checkbox,"undo":a.undo_stack.len()})
    };
    let choose = |a: &mut IngeTrazoApp, code: &str, precision: i64, new: bool| {
        a.begin_preferences();
        let d = a.document_session.preferences_draft.as_mut().unwrap();
        d.unit_code = code.into();
        d.precision = precision;
        d.units_for_new = new;
    };
    rows.push(record(&mut a, "loaded-composite"));
    let before = a.scene.clone();
    let version = a.scene.version;
    choose(&mut a, "m", 0, true);
    a.cancel_preferences();
    assert_eq!(a.scene, before);
    rows.push(record(&mut a, "cancel"));
    choose(&mut a, "ft-in", 4, false);
    a.accept_preferences();
    assert_eq!(a.scene.version, version);
    rows.push(record(&mut a, "unchanged"));
    for (i, code) in ingetrazo_core::units::MODEL_UNIT_CODES.iter().enumerate() {
        let version = a.scene.version;
        choose(&mut a, code, (i % 7) as i64, i == 7);
        a.accept_preferences();
        assert_eq!(a.scene.version, version + 1);
        rows.push(record(&mut a, &format!("apply-{code}")));
        a.save_to_path(&root.join(format!("{code}-native.igz")));
        assert!(root.join(format!("{code}-native.igz")).exists());
    }
    let mut a = IngeTrazoApp::new();
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    rows.push(record(&mut a, "restart-new"));
    assert!(!a.document_dirty());
    a.open_document(&root.join("input.igz"));
    rows.push(record(&mut a, "open-retains-document"));
    choose(&mut a, "cm", 6, false);
    a.accept_preferences();
    rows.push(record(&mut a, "unchecked-default"));
    a.document_session
        .preferences
        .extra
        .insert("units/new_document".into(), json!("{invalid"));
    a.persist_preferences();
    let mut a = IngeTrazoApp::new();
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    rows.push(record(&mut a, "malformed-new"));
    assert!(!a.document_dirty());
    // Real New uses the persisted pair without dirtying; Open ignores that pair.
    a.document_session.preferences.extra.insert(
        "units/new_document".into(),
        json!("{\"length\":\"in-frac\",\"precision\":3}"),
    );
    a.new_document();
    assert_eq!(ingetrazo_core::units::model_unit(&a.scene), "in-frac");
    assert!(!a.document_dirty());
    std::fs::write(
        root.join("native-units.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .unwrap();
}

fn import_preferences_acceptance() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let store = root.join("native-import-settings");
    if store.exists() {
        std::fs::remove_dir_all(&store).unwrap();
    }
    let mut a = IngeTrazoApp::new();
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    let mut prefs = Vec::new();
    let pref = |a: &IngeTrazoApp, phase: &str| json!({"phase":phase,"obj":ingetrazo_gui::import_units::preference(&a.document_session.preferences,"import/obj_unit","m"),"dxf":ingetrazo_gui::import_units::preference(&a.document_session.preferences,"import/dxf_unit","mm")});
    prefs.push(pref(&a, "default"));
    a.begin_preferences();
    {
        let d = a.document_session.preferences_draft.as_mut().unwrap();
        d.obj_unit = "ft".into();
        d.dxf_unit = "in".into();
    }
    prefs.push(pref(&a, "draft"));
    a.cancel_preferences();
    prefs.push(pref(&a, "cancel"));
    a.begin_preferences();
    {
        let d = a.document_session.preferences_draft.as_mut().unwrap();
        d.obj_unit = "ft".into();
        d.dxf_unit = "in".into();
    }
    a.accept_preferences();
    prefs.push(pref(&a, "accept"));
    let mut a = IngeTrazoApp::new();
    a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
    prefs.push(pref(&a, "restart"));
    fn points(scene: &Scene) -> Vec<[f64; 3]> {
        let mut all = scene
            .mesh
            .vertices
            .iter()
            .map(|v| v.position.to_array())
            .collect::<Vec<_>>();
        fn walk(gs: &[Group], x: DMat4, out: &mut Vec<[f64; 3]>) {
            for g in gs {
                let x = x * g.transform;
                out.extend(
                    g.mesh
                        .vertices
                        .iter()
                        .map(|v| x.transform_point3(v.position).to_array()),
                );
                walk(&g.children, x, out);
            }
        }
        walk(&scene.groups, DMat4::IDENTITY, &mut all);
        all.sort_by(|a, b| {
            a[0].total_cmp(&b[0])
                .then(a[1].total_cmp(&b[1]))
                .then(a[2].total_cmp(&b[2]))
        });
        all
    }
    let cases = [
        ("obj-cancel", "ambiguous.obj", None),
        ("obj-stored", "ambiguous.obj", Some("ft")),
        ("obj-size", "large.obj", Some("cm")),
        ("obj-huge", "huge.obj", Some("mm")),
        ("obj-explicit", "ambiguous.obj", Some("in")),
        ("dxf-header", "header.dxf", Some("mm")),
        ("dxf-lying", "lying.dxf", Some("cm")),
        ("dxf-stored", "tiny.dxf", Some("ft")),
    ];
    let mut rows = Vec::new();
    for (case, file, chosen) in cases {
        let mut a = IngeTrazoApp::new();
        a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(&store));
        let x = a.scene.mesh.add_vertex(DVec3::new(-1., 0., 0.));
        let y = a.scene.mesh.add_vertex(DVec3::ZERO);
        a.scene.mesh.add_edge(x, y);
        let before = a.scene.clone();
        a.begin_unit_import(&root.join(file)).unwrap();
        let suggested = a
            .document_session
            .unit_import
            .as_ref()
            .unwrap()
            .code
            .clone();
        let confirmed = if let Some(code) = chosen {
            a.document_session.unit_import.as_mut().unwrap().code = code.into();
            a.confirm_unit_import().unwrap()
        } else {
            a.cancel_unit_import();
            None
        };
        let factor = confirmed.as_ref().map(|c| c.factor);
        let record = |a: &IngeTrazoApp, phase: &str| json!({"case":case,"phase":phase,"suggested":suggested,"factor":factor,"obj":ingetrazo_gui::import_units::preference(&a.document_session.preferences,"import/obj_unit","m"),"dxf":ingetrazo_gui::import_units::preference(&a.document_session.preferences,"import/dxf_unit","mm"),"points":points(&a.scene),"undo":a.undo_stack.len(),"redo":a.redo_stack.len()});
        rows.push(record(&a, "confirmed"));
        if let Some(c) = confirmed {
            a.commit_unit_import(c).unwrap();
            assert_eq!(a.undo_stack.len(), 1);
            rows.push(record(&a, "import"));
            a.undo();
            assert_eq!(a.scene.mesh, before.mesh);
            rows.push(record(&a, "undo"));
            a.redo();
            rows.push(record(&a, "redo"));
        } else {
            assert_eq!(a.scene, before);
        }
    }
    // Invalid accepted content preserves document/history, but source unit choice persists.
    std::fs::write(root.join("invalid.obj"), "v 0 0 0\nv 1 0 0\nf 1 2 9\n").unwrap();
    let before = a.scene.clone();
    let depth = a.undo_stack.len();
    a.begin_unit_import(&root.join("invalid.obj")).unwrap();
    a.document_session.unit_import.as_mut().unwrap().code = "cm".into();
    assert!(a.accept_unit_import().is_err());
    assert_eq!(a.scene, before);
    assert_eq!(a.undo_stack.len(), depth);
    assert_eq!(
        ingetrazo_gui::import_units::preference(
            &a.document_session.preferences,
            "import/obj_unit",
            "m"
        ),
        "cm"
    );
    let mut g = Group::new("nested");
    let x = g.mesh.add_vertex(DVec3::ZERO);
    let y = g.mesh.add_vertex(DVec3::X);
    g.mesh.add_edge(x, y);
    a.scene.groups.push(g);
    a.scene.select(SelectionItem::Group(0));
    a.begin_component_edit();
    let before = a.scene.clone();
    let depth = a.undo_stack.len();
    a.begin_unit_import(&root.join("ambiguous.obj")).unwrap();
    a.cancel_unit_import();
    assert_eq!(a.scene, before);
    assert_eq!(a.undo_stack.len(), depth);
    assert_eq!(a.component_edit_stack.len(), 1);
    std::fs::write(
        root.join("native-import-preferences.json"),
        serde_json::to_vec_pretty(&json!({"preferences":prefs,"rows":rows})).unwrap(),
    )
    .unwrap();
}
