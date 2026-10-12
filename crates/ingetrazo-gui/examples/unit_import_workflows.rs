//! Missing actual OBJ/DXF chooser, unit, face cleanup and saved-document workflow.
#[path = "exchange_import_workflows.rs"]
mod base;
#[path = "dwg_import_workflows.rs"]
mod dialog_ui;
#[path = "skp_import_workflows.rs"]
mod source_json;
use glam::DVec3;
use ingetrazo_core::scene::SelectionItem;
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn camera(a: &IngeTrazoApp) -> Value {
    json!({"target":a.camera.target.to_array(),"distance":a.camera.distance,"yaw":a.camera.yaw,"pitch":a.camera.pitch,"projection":if a.camera.perspective{"perspective"}else{"parallel"},"fov":a.camera.fov_deg})
}
fn scene(a: &IngeTrazoApp) -> Result<Value, String> {
    fn walk(
        v: &mut Value,
        assets: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        match v {
            Value::Object(object) => {
                if let Some(texture) = object.get_mut("texture").and_then(Value::as_object_mut) {
                    if let Some(Value::String(path)) = texture.get("path") {
                        // OBJ uses an owned portable path key; resolve its actual
                        // admitted bytes, preserving every UV/style field.
                        let bytes = assets
                            .get(path)
                            .ok_or_else(|| format!("Owned texture path missing: {path}"))?;
                        let digest = format!("{:x}", Sha256::digest(bytes));
                        texture.remove("path");
                        texture.insert("resource".into(), json!({"encoded_sha256":digest}));
                    }
                }
                for value in object.values_mut() {
                    walk(value, assets)?;
                }
                if let Some(Value::Array(faces)) = object.get_mut("faces") {
                    faces.sort_by_cached_key(source_json::source_json_key);
                }
            }
            Value::Array(values) => {
                for value in values {
                    walk(value, assets)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut value = base::scene(a)?;
    walk(&mut value, &a.scene.embedded_textures)?;
    Ok(value)
}
fn record(a: &IngeTrazoApp, name: &str, phase: &str) -> Result<Value, String> {
    if !a.camera.aspect.is_finite() || a.camera.aspect != 1. {
        return Err("Dialog-only source pose lost aspect1".into());
    }
    Ok(
        json!({"case":name,"phase":phase,"scene":scene(a)?,"camera":camera(a),"depth":a.component_edit_stack.len(),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"preferences":{"obj":a.document_session.preferences.extra.get("import/obj_unit"),"dxf":a.document_session.preferences.extra.get("import/dxf_unit")}}),
    )
}
fn write(out: &Path, input: &Value, states: &[Value], controls: Value) -> Result<(), String> {
    let cases = input["cases"]
        .as_array()
        .ok_or("cases")?
        .iter()
        .map(|c| json!({"name":c["name"],"saved":c["saved"],"phases":c["phases"]}))
        .collect::<Vec<_>>();
    std::fs::write(out.join("native-unit-import.partial.json"),serde_json::to_vec_pretty(&json!({"source_pin":input["source_pin"],"cases":cases,"states":states,"controls":controls,"source_pose":{"aspect":1.0,"law":"Pinned OrbitCamera default; offscreen GL resize failed","frames":"actual unit modal only","viewport_pixel_claim":false}})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-unit-import.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker).map_err(|e| e.to_string())?;
    std::fs::hard_link(
        std::env::current_exe().map_err(|e| e.to_string())?,
        picker.join(if cfg!(windows) {
            "kdialog.exe"
        } else {
            "kdialog"
        }),
    )
    .map_err(|e| e.to_string())?;
    let old = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![picker];
    paths.extend(std::env::split_paths(&old));
    std::env::set_var(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let mut states = Vec::new();
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let mode = case["mode"].as_str().ok_or("mode")?;
        let kind = case["kind"].as_str().ok_or("kind")?;
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.native_gpu_viewport_enabled = true;
        a.set_language("en");
        let mut store = ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        );
        store.config_dir = out.join("shared-config");
        a.attach_session_store(store);
        a.open_document(&source.join(case["input"].as_str().ok_or("input")?));
        a.current_file = None;
        let c = &case["initial_camera"];
        let t = c["target"].as_array().ok_or("target")?;
        a.camera.target = DVec3::new(
            t[0].as_f64().ok_or("x")?,
            t[1].as_f64().ok_or("y")?,
            t[2].as_f64().ok_or("z")?,
        );
        a.camera.distance = c["distance"].as_f64().ok_or("distance")?;
        a.camera.yaw = c["yaw"].as_f64().ok_or("yaw")?;
        a.camera.pitch = c["pitch"].as_f64().ok_or("pitch")?;
        a.camera.perspective = c["projection"] == "perspective";
        a.camera.fov_deg = c["fov"].as_f64().ok_or("fov")?;
        a.camera.aspect = 1.;
        if case["context"] == "leaf" {
            for _ in 0..2 {
                a.scene.select(SelectionItem::Group(0));
                a.begin_component_edit();
            }
            if a.component_edit_stack.len() != 2 {
                return Err("actual nested context missing".into());
            }
        }
        let before_status = a.status_message.clone();
        let before = record(&a, name, "before")?;
        states.push(before.clone());
        write(out, &input, &states, json!({}))?;
        std::env::set_var(
            "UNIT_FIXTURE_PICKER",
            if mode == "chooser-cancel" {
                String::new()
            } else {
                source
                    .join(case["file"].as_str().ok_or("file")?)
                    .to_string_lossy()
                    .into_owned()
            },
        );
        if kind == "obj" {
            a.choose_obj_import()
        } else {
            a.choose_dxf_import()
        }
        let ctx = egui::Context::default();
        dialog_ui::frame(&mut a, &ctx, vec![]);
        let expects_unit = mode != "chooser-cancel" && !(kind == "dxf" && mode == "failure");
        if a.document_session.unit_import.is_some() != expects_unit {
            return Err(format!(
                "Actual unit ownership missing/unexpected {name}: {}",
                a.status_message
            ));
        }
        if a.document_session.unit_import.is_some() {
            if mode == "unit-cancel" {
                dialog_ui::click(&mut a, &ctx, "import-unit-cancel")?;
            } else {
                dialog_ui::click(&mut a, &ctx, "import-unit-combo")?;
                dialog_ui::click(&mut a, &ctx, "import-unit-option-m")?;
                dialog_ui::click(&mut a, &ctx, "import-unit-accept")?;
            }
        }
        if a.document_session.unit_import.is_some() {
            return Err(format!("Actual accept/cancel retained unit modal {name}"));
        }
        let imported = record(&a, name, "import")?;
        states.push(imported.clone());
        write(out, &input, &states, json!({}))?;
        if mode == "accept" {
            if imported["scene"] == before["scene"]
                || a.undo_stack.len() != before["undo"].as_u64().ok_or("undo")? as usize + 1
            {
                return Err(format!("actual import absent {name}: {}", a.status_message));
            }
            if kind == "obj" && imported["camera"] != before["camera"] {
                return Err("OBJ fitted camera".into());
            }
            if kind == "dxf" && imported["camera"] == before["camera"] {
                return Err("DXF fit missing".into());
            }
            a.undo();
            states.push(record(&a, name, "undo")?);
            a.redo();
            states.push(record(&a, name, "redo")?);
            while !a.component_edit_stack.is_empty() {
                a.finish_component_edit();
            }
            states.push(record(&a, name, "finish")?);
            a.save_to_path(&out.join(case["saved"].as_str().ok_or("saved")?));
            if !out.join(case["saved"].as_str().unwrap()).exists() {
                return Err("actual saved document missing".into());
            }
        } else {
            for key in ["scene", "camera", "undo", "redo", "depth"] {
                if imported[key] != before[key] {
                    return Err(format!("refusal/cancel mutated {key} {name}"));
                }
            }
            if mode == "failure"
                && (a.status_message.is_empty()
                    || a.status_message == before_status
                    || a.status_message.starts_with("Imported ")
                    || a.document_session.unit_import.is_some())
            {
                return Err("failure witness missing".into());
            }
        }
        let idle = record(&a, name, "idle")?;
        for _ in 0..8 {
            dialog_ui::frame(&mut a, &ctx, vec![]);
        }
        if record(&a, name, "idle")? != idle {
            return Err("idle mutated document/history/camera".into());
        }
        write(out, &input, &states, json!({}))?;
    }
    write(
        out,
        &input,
        &states,
        json!({"actual_chooser":true,"actual_unit_buttons":true,"cancel_failure_preserved":true,"idle_readonly":true,"camera_policy":true,"preferences_reopened":true}),
    )?;
    std::fs::rename(
        out.join("native-unit-import.partial.json"),
        out.join("native-unit-import.json"),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    if std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_owned()))
        .is_some_and(|n| n == "kdialog")
    {
        println!(
            "{}",
            std::env::var("UNIT_FIXTURE_PICKER").unwrap_or_default()
        );
        return;
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("unit_import_workflows SOURCE OUTPUT");
        std::process::exit(2)
    }
    if let Err(e) = run(&PathBuf::from(&args[0]), &PathBuf::from(&args[1])) {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
