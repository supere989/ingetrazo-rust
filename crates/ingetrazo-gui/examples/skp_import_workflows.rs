//! Actual App SKP chooser/worker/history/save; no viewport rendering claim.
#[path = "exchange_import_workflows.rs"]
mod exchange;
use exchange::{click, frame, mesh_state, source_groups};
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};
fn paint_recipe(mut value: Value, assets: &HashMap<String, Vec<u8>>) -> Result<Value, String> {
    if let Some(t) = value.get_mut("texture").and_then(Value::as_object_mut) {
        if let Some(Value::String(key)) = t.remove("embed") {
            let bytes = assets.get(&key).ok_or("material texture owner missing")?;
            t.insert(
                "resource".into(),
                json!({"encoded_sha256":format!("{:x}",Sha256::digest(bytes))}),
            );
        }
    }
    if let Some(back) = value.get_mut("back").filter(|v| v.is_object()) {
        *back = paint_recipe(back.clone(), assets)?;
    }
    Ok(value)
}
fn mesh(
    mesh: &ingetrazo_core::mesh::Mesh,
    m: DMat4,
    assets: &HashMap<String, Vec<u8>>,
) -> Result<Value, String> {
    let mut value = mesh_state(mesh, m, assets)?;
    for face in value["faces"].as_array_mut().unwrap() {
        face["attrs"] = paint_recipe(face["attrs"].take(), assets)?;
    }
    value["faces"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(Value::to_string);
    Ok(value)
}
fn scene(app: &IngeTrazoApp) -> Result<Value, String> {
    fn group(
        g: &Group,
        parent: DMat4,
        owners: &mut HashMap<uuid::Uuid, usize>,
        next: &mut usize,
        assets: &HashMap<String, Vec<u8>>,
    ) -> Result<Value, String> {
        let owner = if g.mesh.vertices.is_empty() {
            None
        } else if let Some(id) = g.mesh_share {
            Some(*owners.entry(id).or_insert_with(|| {
                let id = *next;
                *next += 1;
                id
            }))
        } else {
            let id = *next;
            *next += 1;
            Some(id)
        };
        let component = g
            .native_document
            .get("component")
            .and_then(Value::as_bool)
            .unwrap_or(g.component_definition.is_some());
        let billboard = if g.billboard {
            g.native_document
                .get("billboard")
                .cloned()
                .unwrap_or(json!(true))
        } else {
            json!(false)
        };
        let world = parent * g.transform;
        Ok(
            json!({"name":g.name,"layer":g.layer,"local_matrix":g.transform.to_cols_array(),"axes":g.axes_matrix().unwrap_or(DMat4::IDENTITY).to_cols_array(),"component":component,"billboard":billboard,"mesh_owner":owner,"local_mesh":mesh(&g.mesh,DMat4::IDENTITY,assets)?,"world_mesh":mesh(&g.mesh,world,assets)?,"children":g.children.iter().map(|c|group(c,world,owners,next,assets)).collect::<Result<Vec<_>,_>>()?}),
        )
    }
    let assets = &app.scene.embedded_textures;
    let mut owners = HashMap::new();
    let mut next = 0;
    let groups = source_groups(app)
        .iter()
        .map(|g| group(g, DMat4::IDENTITY, &mut owners, &mut next, assets))
        .collect::<Result<Vec<_>, _>>()?;
    let materials = app
        .scene
        .materials
        .iter()
        .map(|(name, m)| Ok((name.clone(), paint_recipe(m.registry_recipe(), assets)?)))
        .collect::<Result<serde_json::Map<_, _>, String>>()?;
    let doc = ingetrazo_formats::igz::scene_to_igz_document(&app.scene);
    let loose = app
        .component_edit_stack
        .first()
        .map_or(&app.scene.mesh, |s| &s.parked_scene().mesh);
    let layers = app
        .scene
        .layers
        .iter()
        .map(|l| {
            let mut v = json!({"name":l.name});
            if !l.visible {
                v["visible"] = json!(false);
            }
            if l.locked {
                v["locked"] = json!(true);
            }
            v
        })
        .collect::<Vec<_>>();
    let dimensions = app
        .scene
        .dimensions
        .iter()
        .map(|d| {
            json!([
                d.start.to_array(),
                d.end.to_array(),
                d.offset.to_array(),
                d.layer,
                d.text_override,
                d.axis,
                d.anchor_a.is_some(),
                d.anchor_b.is_some()
            ])
        })
        .collect::<Vec<_>>();
    let texts = app
        .scene
        .text_labels
        .iter()
        .map(|t| {
            let mut v =
                json!({"anchor":t.anchor.to_array(),"offset":t.offset.to_array(),"text":t.text});
            if let Some(layer) = &t.layer {
                v["layer"] = json!(layer);
            }
            v
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"active":mesh(&app.scene.mesh,DMat4::IDENTITY,assets)?,"loose":mesh(loose,DMat4::IDENTITY,assets)?,"groups":groups,"materials":materials,"layers":layers,"views":doc["saved_views"],"dimensions":dimensions,"texts":texts,"back_face_color":doc.get("back_face_color").cloned().unwrap_or(Value::Null),"units":app.scene.model_units()}),
    )
}
fn observe(app: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Result<Value, String> {
    Ok(
        json!({"case":name,"phase":phase,"scene":scene(app)?,"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"depth":app.component_edit_stack.len(),"outcome":outcome,"camera":{"distance":app.camera.distance}}),
    )
}
fn wait(app: &mut IngeTrazoApp, ctx: &egui::Context) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while app.exchange_import_running() {
        frame(app, ctx, vec![]);
        if Instant::now() > deadline {
            return Err("worker deadline".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-skp.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker).map_err(|e| e.to_string())?;
    std::fs::hard_link(
        std::env::current_exe().map_err(|e| e.to_string())?,
        picker.join("kdialog"),
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
    let mut cases = Vec::new();
    let mut states = Vec::new();
    let mut parses = 0;
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let outcome = case["outcome"].as_str().ok_or("outcome")?;
        let ctx = egui::Context::default();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.native_gpu_viewport_enabled = true;
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        ));
        app.open_document(&source.join(case["input"].as_str().ok_or("input")?));
        app.current_file = None;
        if case["context"] == "leaf" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            if app.component_edit_stack.len() != 2 {
                return Err("nested admission failed".into());
            }
            app.save_undo_state();
            app.scene
                .text_labels
                .push(ingetrazo_core::textlabel::TextLabel::new(
                    DVec3::new(1., 2., 3.),
                    DVec3::new(0., 1., 0.),
                    "Prior nested label",
                ));
        }
        let before = scene(&app)?;
        let history = (app.undo_stack.len(), app.redo_stack.len());
        let camera = (
            app.camera.target,
            app.camera.distance,
            app.camera.yaw,
            app.camera.pitch,
        );
        states.push(observe(&app, name, "before", "idle")?);
        std::env::set_var(
            "EXCHANGE_FIXTURE_PICKER",
            if outcome == "cancel" {
                String::new()
            } else {
                source
                    .join(case["file"].as_str().ok_or("file")?)
                    .to_string_lossy()
                    .into_owned()
            },
        );
        app.choose_skp_import();
        wait(&mut app, &ctx)?;
        states.push(observe(&app, name, "import", outcome)?);
        if (
            app.camera.target,
            app.camera.distance,
            app.camera.yaw,
            app.camera.pitch,
        ) != camera
        {
            return Err("camera changed".into());
        }
        if outcome == "accepted" || outcome == "empty" {
            if app.exchange_import.error.is_some()
                || app.undo_stack.len() != history.0 + 1
                || app.exchange_import_stats() != (1, 1)
            {
                return Err(format!("{name}: import failed {}", app.status_message));
            }
            let imported = scene(&app)?;
            if outcome == "accepted" && imported["groups"] == before["groups"] {
                return Err("root publication no-op".into());
            }
            app.undo();
            states.push(observe(&app, name, "undo", outcome)?);
            let mut expected = before.clone();
            expected["materials"] = imported["materials"].clone();
            expected["back_face_color"] = imported["back_face_color"].clone();
            if scene(&app)? != expected {
                return Err(format!("{name}: Undo ownership changed"));
            }
            app.redo();
            states.push(observe(&app, name, "redo", outcome)?);
            if name == "repeat-import" {
                app.choose_skp_import();
                wait(&mut app, &ctx)?;
                states.push(observe(&app, name, "repeat", outcome)?);
                if app.exchange_import_stats() != (2, 2) {
                    return Err("repeat parse/publication failed".into());
                }
                app.undo();
                states.push(observe(&app, name, "repeat-undo", outcome)?);
                app.redo();
                states.push(observe(&app, name, "repeat-redo", outcome)?);
            }
            while !app.component_edit_stack.is_empty() {
                let depth = app.component_edit_stack.len();
                app.finish_component_edit();
                if app.component_edit_stack.len() == depth {
                    return Err(app.status_message.clone());
                }
            }
            states.push(observe(&app, name, "finish", outcome)?);
            if name == "nested-root-ownership" {
                let finished = scene(&app)?;
                app.undo();
                states.push(observe(&app, name, "finish-undo", outcome)?);
                app.redo();
                states.push(observe(&app, name, "finish-redo", outcome)?);
                if scene(&app)? != finished {
                    return Err("finished Redo changed state".into());
                }
            }
            let saved = out.join(case["saved"].as_str().ok_or("saved")?);
            app.save_to_path(&saved);
            if !saved.is_file() {
                return Err(app.status_message.clone());
            }
        } else {
            if scene(&app)? != before || (app.undo_stack.len(), app.redo_stack.len()) != history {
                return Err("cancel/refusal mutated model/history".into());
            }
            if outcome == "unreadable" {
                if !app.exchange_import.warning || app.exchange_import.error.is_none() {
                    return Err("actual warning missing".into());
                }
                click(&mut app, &ctx, "exchange-import-error-ok")?;
                if app.exchange_import.owns_input() {
                    return Err("warning not dismissed".into());
                }
            } else if app.exchange_import_stats() != (0, 0) {
                return Err("Cancel launched parser".into());
            }
        }
        parses += app.exchange_import_stats().0;
        let stable = scene(&app)?;
        let history = (app.undo_stack.len(), app.redo_stack.len());
        let stats = app.exchange_import_stats();
        for _ in 0..8 {
            frame(&mut app, &ctx, vec![]);
        }
        if stable != scene(&app)?
            || history != (app.undo_stack.len(), app.redo_stack.len())
            || stats != app.exchange_import_stats()
        {
            return Err("idle changed state".into());
        }
        cases.push(json!({"name":name,"saved":case["saved"],"phases":case["phases"]}));
        std::fs::write(
            out.join("native-skp.partial.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"states":states,"status":app.status_message}),
            )
            .unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    let mut retired = IngeTrazoApp::new();
    retired.system_clipboard = None;
    retired.native_gpu_viewport_enabled = true;
    retired.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("state/context-retired"),
    ));
    retired.open_document(&source.join(input["cases"][0]["input"].as_str().ok_or("input")?));
    retired.begin_exchange_import(
        &source.join(input["cases"][0]["file"].as_str().ok_or("file")?),
        ingetrazo_gui::exchange_import::Kind::Skp,
    )?;
    retired.scene.select(SelectionItem::Group(0));
    retired.begin_component_edit();
    let before = scene(&retired)?;
    let history = (retired.undo_stack.len(), retired.redo_stack.len());
    let ctx = egui::Context::default();
    wait(&mut retired, &ctx)?;
    if before != scene(&retired)?
        || history != (retired.undo_stack.len(), retired.redo_stack.len())
        || retired.exchange_import_stats() != (1, 0)
    {
        return Err("stale context published".into());
    }
    std::env::set_var("PATH", old);
    std::fs::write(out.join("native-skp.json"),serde_json::to_vec_pretty(&json!({"source_pin":input["source_pin"],"cases":cases,"states":states,"parses":parses,"context_retired_parses":1,"controls":{"actual_chooser":true,"file_worker":true,"parse_once":true,"camera_unchanged":true,"atomic_refusal":true,"idle_readonly":true,"context_retired":true,"registry_undo":true}})).unwrap()).map_err(|e|e.to_string())?;
    Ok(())
}
fn main() {
    if std::env::args()
        .next()
        .is_some_and(|p| Path::new(&p).file_stem().and_then(|s| s.to_str()) == Some("kdialog"))
    {
        let path = std::env::var("EXCHANGE_FIXTURE_PICKER").unwrap_or_default();
        if path.is_empty() {
            std::process::exit(1);
        }
        println!("{path}");
        return;
    }
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: skp_import_workflows SOURCE OUTPUT");
        std::process::exit(2);
    }
    if let Err(e) = run(Path::new(&args[1]), Path::new(&args[2])) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
