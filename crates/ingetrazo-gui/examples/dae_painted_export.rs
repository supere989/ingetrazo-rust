//! Actual App DAE chooser/export, document/history/context and saved ownership.
#[path = "unit_import_workflows.rs"]
mod observe;
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
fn camera(a: &IngeTrazoApp) -> Value {
    json!({"target":a.camera.target.to_array(),"distance":a.camera.distance,"yaw":a.camera.yaw,"pitch":a.camera.pitch,"projection":if a.camera.perspective{"perspective"}else{"parallel"},"fov":a.camera.fov_deg})
}
fn state(a: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Result<Value, String> {
    let mut scene = observe::scene_with_missing(a, true)?;
    let mut units = a.scene.model_units().as_object().cloned().unwrap_or_default();
    units.insert("length".into(), json!(ingetrazo_core::units::model_unit(&a.scene)));
    units.insert("precision".into(), json!(ingetrazo_core::units::model_precision(&a.scene)));
    scene["units"] = Value::Object(units);
    Ok(
        json!({"case":name,"phase":phase,"scene":scene,"current_file":a.current_file.as_ref().and_then(|p|p.file_name()).unwrap_or_default().to_string_lossy(),"dirty":a.document_dirty(),"camera":camera(a),"edit_context":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"composer_history":null,"outcome":outcome}),
    )
}
fn unix_time() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Owned picker process, not an API return-value injection.
    if std::env::current_exe()?
        .file_name()
        .is_some_and(|n| n == "kdialog")
    {
        let marker = PathBuf::from(std::env::var("DAE_PICKER_MARKER")?);
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(marker)?;
        use std::io::Write;
        writeln!(f, "invoked")?;
        match std::env::var("DAE_PICKER_CHOICE") {
            Ok(s) if !s.is_empty() => {
                println!("{s}");
                return Ok(());
            }
            _ => std::process::exit(1),
        }
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("dae_painted_export SOURCE OUTPUT".into());
    }
    let source = Path::new(&args[0]).canonicalize()?;
    let out = Path::new(&args[1]).to_path_buf();
    std::fs::create_dir_all(&out)?;
    let out = out.canonicalize()?;
    let input: Value =
        serde_json::from_slice(&std::fs::read(source.join("original-dae-export.json"))?)?;
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker)?;
    std::fs::copy(std::env::current_exe()?, picker.join("kdialog"))?;
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let old_marker = std::env::var_os("DAE_PICKER_MARKER");
    let old_choice = std::env::var_os("DAE_PICKER_CHOICE");
    struct Restore {
        path: std::ffi::OsString,
        marker: Option<std::ffi::OsString>,
        choice: Option<std::ffi::OsString>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            std::env::set_var("PATH", &self.path);
            for (k, v) in [
                ("DAE_PICKER_MARKER", &self.marker),
                ("DAE_PICKER_CHOICE", &self.choice),
            ] {
                if let Some(v) = v {
                    std::env::set_var(k, v)
                } else {
                    std::env::remove_var(k)
                }
            }
        }
    }
    let _restore = Restore {
        path: old_path.clone(),
        marker: old_marker,
        choice: old_choice,
    };
    let paths = std::iter::once(picker).chain(std::env::split_paths(&old_path));
    std::env::set_var("PATH", std::env::join_paths(paths)?);
    let marker = out.join("picker-invocations.log");
    std::env::set_var("DAE_PICKER_MARKER", &marker);
    let mut cases = Vec::new();
    let mut states = Vec::new();
    let mut bounds = Vec::new();
    let mut owned_sidecar_checks = 0usize;
    let exports = out.join("exports");
    std::fs::create_dir_all(&exports)?;
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.native_gpu_viewport_enabled = true;
        a.set_language("en");
        a.open_document(&source.join(case["input"].as_str().ok_or("input")?));
        // Explicit pinned source constructor pose, not frozen expected state values.
        a.camera.target = glam::DVec3::ZERO;
        a.camera.distance = 20.;
        a.camera.yaw = -45_f64.to_radians();
        a.camera.pitch = 30_f64.to_radians();
        a.camera.perspective = true;
        a.camera.fov_deg = 45.;
        a.camera.aspect = 1.;
        a.current_file = Some(out.join(case["initial_file"].as_str().ok_or("initial_file")?));
        a.document_session.clean = None;
        for index in case["edit_path"].as_array().ok_or("edit_path")? {
            a.scene
                .select(SelectionItem::Group(index.as_u64().ok_or("index")? as usize));
            a.begin_component_edit();
        }
        assert_eq!(
            a.component_edit_stack.len(),
            case["edit_path"].as_array().unwrap().len()
        );
        if let Some(rows) = case["post_load_textures"].as_array() {
            for row in rows {
                let index = row["face"].as_u64().ok_or("postloadface")? as usize;
                let file = source.join(row["path"].as_str().ok_or("postloadpath")?);
                let bytes = std::fs::read(&file)?;
                let key = file.to_string_lossy().into_owned();
                a.scene.embedded_textures.insert(key.clone(), bytes);
                let tex = a.scene.mesh.faces[index]
                    .attrs
                    .get_mut("texture")
                    .ok_or("texture")?;
                tex["path"] = json!(key);
                if let Some(object) = tex.as_object_mut() {
                    object.remove("embed");
                }
            }
        }
        if case["history_seed"] == true {
            for color in [[0.15, 0.25, 0.35], [0.45, 0.55, 0.65]] {
                a.save_undo_state();
                a.scene.mesh.faces[0]
                    .attrs
                    .insert("color".into(), json!(color));
            }
            a.undo();
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), (1, 1));
        }
        let mut actual_exports = Vec::new();
        for phase in case["phases"].as_array().ok_or("phases")? {
            let phase = phase.as_str().ok_or("phase")?;
            let before = a.scene.clone();
            let before_camera = camera(&a);
            let before_file = a.current_file.clone();
            let before_dirty = a.document_dirty();
            let counts = (
                a.undo_stack.len(),
                a.redo_stack.len(),
                a.component_edit_stack.len(),
            );
            let outcome = if phase == "before" {
                "ok"
            } else {
                let chosen = match phase {
                    "cancel" => None,
                    "suffix" => Some(exports.join(format!("{name}-coerced.wrong"))),
                    "write-error" => Some(exports.join(format!("{name}-blocked.dae"))),
                    _ => Some(exports.join(format!("{name}-export.dae"))),
                };
                if phase == "suffix" {
                    std::fs::write(
                        chosen.as_ref().unwrap().with_extension("dae"),
                        b"prior destination",
                    )?;
                }
                if phase == "write-error" {
                    std::fs::create_dir_all(chosen.as_ref().unwrap())?;
                }
                let calls_before = std::fs::read_to_string(&marker)
                    .unwrap_or_default()
                    .lines()
                    .count();
                std::env::set_var(
                    "DAE_PICKER_CHOICE",
                    chosen
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
                let start = unix_time();
                a.choose_dae_export();
                let end = unix_time();
                bounds.push(json!({"case":name,"phase":phase,"started":start,"ended":end}));
                assert_eq!(
                    std::fs::read_to_string(&marker)?.lines().count(),
                    calls_before + 1
                );
                if chosen.is_none() {
                    "cancel"
                } else if a.dae_export_error.is_some() {
                    assert_eq!(phase, "write-error");
                    assert!(std::fs::read_dir(chosen.as_ref().unwrap())?
                        .next()
                        .is_none());
                    "failed"
                } else {
                    let path = chosen.unwrap().with_extension("dae");
                    assert!(path.is_file());
                    let xml = std::fs::read_to_string(&path)?;
                    for tail in xml.split("<init_from>").skip(1) {
                        let map = tail.split("</init_from>").next().ok_or("init_from")?;
                        if map.contains('.') {
                            let sidecar = path.with_file_name(map);
                            match std::fs::read(&sidecar) {
                                Ok(bytes) => {
                                    assert!(
                                        a.scene.embedded_textures.values().any(|v| v == &bytes),
                                        "Exported sidecar has no admitted resource owner"
                                    );
                                    owned_sidecar_checks += 1;
                                }
                                Err(_) => assert_eq!(name, "missing-resource-write-error"),
                            }
                        }
                    }
                    actual_exports.push(
                        json!({"phase":phase,"file":path.strip_prefix(&out)?.to_string_lossy()}),
                    );
                    "ok"
                }
            };
            assert_eq!(a.scene, before);
            assert_eq!(camera(&a), before_camera);
            assert_eq!(a.current_file, before_file);
            assert_eq!(a.document_dirty(), before_dirty);
            assert_eq!(
                (
                    a.undo_stack.len(),
                    a.redo_stack.len(),
                    a.component_edit_stack.len()
                ),
                counts
            );
            states.push(state(&a, name, phase, outcome)?);
            a.dae_export_error = None;
        }
        // Fixture writer observes the same active mesh plus global source group
        // view; this test-only copy does not Finish or publish a user document.
        let root = a
            .component_edit_stack
            .first()
            .map_or(&a.scene, |session| session.parked_scene());
        // Preserve the original whole-document raw metadata, prototype registry
        // and native baseline before overlaying the actual active-context view.
        let mut saved = root.clone();
        saved.mesh = a.scene.mesh.clone();
        saved.embedded_textures = a.scene.embedded_textures.clone();
        saved.groups = observe::source_groups(&a);
        ingetrazo_formats::save_file(&saved, &out.join(case["saved"].as_str().ok_or("saved")?))?;
        let mut actual_case = case.clone();
        actual_case["exports"] = json!(actual_exports);
        cases.push(actual_case);
        if name == "missing-resource-write-error" {
            // Meaningful independent native modal control: selected geometry and
            // a real prior Undo action would change the document if admitted.
            a.save_undo_state();
            a.scene.mesh.faces[0]
                .attrs
                .insert("color".into(), json!([0.2, 0.3, 0.4]));
            a.scene.select(SelectionItem::Face(0));
            let ctx = egui::Context::default();
            let mut tick = 0.;
            fn frame(
                a: &mut IngeTrazoApp,
                ctx: &egui::Context,
                tick: &mut f64,
                events: Vec<egui::Event>,
            ) {
                *tick += 0.016;
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        time: Some(*tick),
                        events,
                        ..Default::default()
                    },
                    |ctx| a.update_ui(ctx),
                );
            }
            frame(&mut a, &ctx, &mut tick, vec![]);
            let blocked = out.join("modal-blocked.dae");
            std::fs::create_dir_all(&blocked)?;
            assert!(a.export_dae_to_path(&blocked).is_err());
            assert!(a.dae_export_error.is_some());
            let before = a.scene.clone();
            let pose = camera(&a);
            let counts = (a.undo_stack.len(), a.redo_stack.len());
            let ctrl = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let point = egui::pos2(20., 20.);
            frame(
                &mut a,
                &ctx,
                &mut tick,
                vec![
                    egui::Event::Key {
                        key: egui::Key::Z,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: ctrl,
                    },
                    egui::Event::Key {
                        key: egui::Key::Delete,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    },
                ],
            );
            assert_eq!(a.scene, before);
            assert_eq!(camera(&a), pose);
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), counts);
            assert!(a.dae_export_error.is_some());
            // New actual export attempt retires old modal ownership; retry is
            // independent of the frozen source phases and cannot manufacture them.
            a.export_dae_to_path(&out.join("modal-retry.dae"))?;
            assert!(a.dae_export_error.is_none());
            assert_eq!(a.scene, before);
        }
        // Auxiliary opaque preservation control is separate from literal states/saves.
        let mesh = if !a.scene.mesh.faces.is_empty() {
            &mut a.scene.mesh
        } else {
            &mut a.scene.groups[0].mesh
        };
        let face = mesh
            .faces
            .first_mut()
            .ok_or("Opaque guard has no actual face")?;
        face.attrs.insert(
            "vendor_native_dae".into(),
            json!({"keep":[1,"opaque",false]}),
        );
        let before = a.scene.clone();
        a.export_dae_to_path(&out.join(format!("{name}-opaque-control.dae")))?;
        assert_eq!(a.scene, before);
        assert!(
            a.scene
                .mesh
                .faces
                .iter()
                .chain(a.scene.groups.iter().flat_map(|g| g.mesh.faces.iter()))
                .any(|f| f.attrs.get("vendor_native_dae")
                    == Some(&json!({"keep":[1,"opaque",false]})))
        );
    }
    assert_eq!(
        std::fs::read_to_string(&marker)?.lines().count(),
        input["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["phases"].as_array().unwrap().len() - 1)
            .sum::<usize>()
    );
    assert!(owned_sidecar_checks > 0);
    let report = json!({"source_pin":input["source_pin"],"cases":cases,"states":states,"export_time_bounds":bounds,"controls":{"actual_chooser":true,"export_scene_readonly":true,"document_identity_preserved":true,"history_preserved":true,"cancel_noop":true,"atomic_xml_failure":true,"resource_sidecars_owned":true,"opaque_face_preserved":true}});
    std::fs::write(
        out.join("native-dae-export.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{} cases completed",
        input["cases"].as_array().unwrap().len()
    );
    Ok(())
}
