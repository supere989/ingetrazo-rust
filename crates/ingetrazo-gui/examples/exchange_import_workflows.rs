//! Actual App chooser, file worker, ownership/history and save. No viewport pixels.
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn mesh_state(
    mesh: &Mesh,
    m: DMat4,
    assets: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<Value, String> {
    let point = |i: usize| {
        let p = mesh.vertices[i].position;
        if m == DMat4::IDENTITY {
            p
        } else if m == m.as_mat4().as_dmat4() && p == p.as_vec3().as_dvec3() {
            ingetrazo_core::change_axes::map(m, p)
        } else {
            m.transform_point3(p)
        }
        .to_array()
    };
    let ring = |ids: &Vec<usize>| {
        let p = ids.iter().map(|&i| point(i)).collect::<Vec<_>>();
        (0..p.len())
            .map(|i| p[i..].iter().chain(&p[..i]).copied().collect::<Vec<_>>())
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or_default()
    };
    let mut points = (0..mesh.vertices.len()).map(point).collect::<Vec<_>>();
    points.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut edges = mesh
        .edges
        .iter()
        .map(|e| {
            let mut p = [point(e.v0), point(e.v1)];
            p.sort_by(|a, b| a.partial_cmp(b).unwrap());
            json!({"points":p,"soft":e.soft,"hidden":e.hidden})
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|e| e.to_string());
    let mut faces = Vec::new();
    for f in &mesh.faces {
        let mut attrs = f.attrs.clone();
        if let Some(t) = attrs.get_mut("texture").and_then(Value::as_object_mut) {
            if let Some(Value::String(key)) = t.remove("embed") {
                let bytes = assets.get(&key).ok_or("texture owner missing")?;
                t.insert("resource".into(), json!({"encoded_sha256":sha(bytes)}));
            }
        }
        let mut holes = f.holes.iter().map(&ring).collect::<Vec<_>>();
        holes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        faces.push(json!({"outer":ring(&f.vertices),"holes":holes,"attrs":attrs}));
    }
    faces.sort_by_key(|f| f.to_string());
    Ok(json!({"points":points,"edges":edges,"faces":faces}))
}
pub(crate) fn source_groups(app: &IngeTrazoApp) -> Vec<Group> {
    let mut groups = app.component_edit_stack.first().map_or_else(
        || app.scene.groups.clone(),
        |s| s.parked_scene().groups.clone(),
    );
    fn find(groups: &mut [Group], id: uuid::Uuid) -> Option<&mut Group> {
        for g in groups {
            if g.id == id {
                return Some(g);
            }
            if let Some(child) = find(&mut g.children, id) {
                return Some(child);
            }
        }
        None
    }
    for (i, session) in app.component_edit_stack.iter().enumerate() {
        let owner = app
            .component_edit_stack
            .get(i + 1)
            .map_or(&app.scene, |s| s.parked_scene());
        let g = find(&mut groups, session.edited_group_id()).unwrap();
        g.mesh = owner.mesh.clone();
        g.mesh_share = None;
        if let Some(axes)=session.edited_container_axes(){g.native_document["axes"]=json!(axes.to_cols_array());}
        g.children = owner.groups.clone();
        g.transform = DMat4::IDENTITY;
    }
    groups
}
fn scene(app: &IngeTrazoApp) -> Result<Value, String> {
    fn group(
        g: &Group,
        parent: DMat4,
        assets: &std::collections::HashMap<String, Vec<u8>>,
    ) -> Result<Value, String> {
        let m = parent * g.transform;
        let billboard = if g.billboard {
            g.native_document
                .get("billboard")
                .cloned()
                .unwrap_or(json!(true))
        } else {
            json!(false)
        };
        Ok(
            json!({"name":g.name,"layer":g.layer,"local_matrix":g.transform.to_cols_array(),"billboard":billboard,"mesh":mesh_state(&g.mesh,m,assets)?,"children":g.children.iter().map(|c|group(c,m,assets)).collect::<Result<Vec<_>,_>>()?}),
        )
    }
    let loose = app
        .component_edit_stack
        .first()
        .map_or(&app.scene.mesh, |s| &s.parked_scene().mesh);
    let assets = &app.scene.embedded_textures;
    let datum = app.scene.geo_datum.as_ref().map(|d| {
        let mut v = json!({"lat":d.lat,"lon":d.lon,"alt":d.alt});
        if d.north != 0. {
            v["north"] = json!(d.north);
        }
        v
    });
    Ok(
        json!({"active":mesh_state(&app.scene.mesh,DMat4::IDENTITY,assets)?,"loose":mesh_state(loose,DMat4::IDENTITY,assets)?,"groups":source_groups(app).iter().map(|g|group(g,DMat4::IDENTITY,assets)).collect::<Result<Vec<_>,_>>()?,"layers":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"units":app.scene.model_units(),"datum":datum}),
    )
}
fn record(app: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Result<Value, String> {
    Ok(
        json!({"case":name,"phase":phase,"scene":scene(app)?,"undo":app.undo_stack.len(),"redo":app.redo_stack.len(),"depth":app.component_edit_stack.len(),"outcome":outcome,"camera":{"distance":app.camera.distance}}),
    )
}
pub(crate) fn frame(app: &mut IngeTrazoApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1100., 800.),
    ));
    raw.events = events;
    let _ = ctx.run(raw, |c| app.update_ui(c));
}
pub(crate) fn click(app: &mut IngeTrazoApp, ctx: &egui::Context, id: &str) -> Result<(), String> {
    for _ in 0..3 {
        frame(app, ctx, vec![]);
    }
    let p = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or("missing UI control")?
        .center();
    frame(app, ctx, vec![egui::Event::PointerMoved(p)]);
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }],
        );
    }
    Ok(())
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-exchange.json")).map_err(|e| e.to_string())?,
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
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut total = 0;
    let mut faults = Vec::new();
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let ctx = egui::Context::default();
        let mut app = IngeTrazoApp::new();
        app.system_clipboard = None;
        app.set_language("en");
        app.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        ));
        app.open_document(&source.join(case["input"].as_str().ok_or("input")?));
        app.current_file = None;
        app.native_gpu_viewport_enabled = true;
        if case["context"] == "leaf" {
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            app.scene.select(SelectionItem::Group(0));
            app.begin_component_edit();
            assert_eq!(app.component_edit_stack.len(), 2);
        }
        if name == "dae-combined-leaf" {
            app.save_undo_state();
            let a = app.scene.mesh.add_vertex(DVec3::new(20., 1., 0.));
            let b = app.scene.mesh.add_vertex(DVec3::new(21., 1., 0.));
            app.scene.mesh.add_edge(a, b);
        }
        states.push(record(&app, name, "before", "idle")?);
        let before = scene(&app)?;
        let history = (app.undo_stack.len(), app.redo_stack.len());
        let camera = (
            app.camera.target,
            app.camera.distance,
            app.camera.yaw,
            app.camera.pitch,
        );
        std::env::set_var(
            "EXCHANGE_FIXTURE_PICKER",
            if case["outcome"] == "cancel" {
                String::new()
            } else {
                source
                    .join(case["file"].as_str().ok_or("file")?)
                    .to_string_lossy()
                    .into_owned()
            },
        );
        if case["kind"] == "dae" {
            app.choose_dae_import();
        } else {
            app.choose_glb_import();
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while app.exchange_import_running() {
            frame(&mut app, &ctx, vec![]);
            if Instant::now() > deadline {
                return Err(format!("{name}: worker deadline"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let outcome = case["outcome"].as_str().ok_or("outcome")?;
        states.push(record(&app, name, "import", outcome)?);
        if (
            app.camera.target,
            app.camera.distance,
            app.camera.yaw,
            app.camera.pitch,
        ) != camera
        {
            return Err("Import changed camera".into());
        }
        let saved = case["saved"].clone();
        if outcome == "accepted" {
            if app.exchange_import.error.is_some()
                || app.undo_stack.len() != history.0 + 1
                || scene(&app)? == before
            {
                return Err(format!(
                    "{name}: import/history missing: {}",
                    app.status_message
                ));
            }
            let parsed = app.exchange_import_stats().0;
            if parsed != 1 {
                return Err("file parsed more than once".into());
            }
            total += parsed;
            app.undo();
            states.push(record(&app, name, "undo", "accepted")?);
            app.redo();
            states.push(record(&app, name, "redo", "accepted")?);
            while !app.component_edit_stack.is_empty() {
                let depth = app.component_edit_stack.len();
                app.finish_component_edit();
                if app.component_edit_stack.len() == depth {
                    return Err(app.status_message.clone());
                }
            }
            states.push(record(&app, name, "finish", "accepted")?);
            if name == "dae-combined-leaf" {
                let finished = scene(&app)?;
                app.undo();
                states.push(record(&app, name, "finish-undo", "accepted")?);
                app.redo();
                states.push(record(&app, name, "finish-redo", "accepted")?);
                if scene(&app)? != finished {
                    return Err("Finished combined Redo changed geometry".into());
                }
            }
            let path = out.join(saved.as_str().ok_or("saved")?);
            app.save_to_path(&path);
            if !path.is_file() {
                return Err(app.status_message.clone());
            }
        } else {
            if scene(&app)? != before || (app.undo_stack.len(), app.redo_stack.len()) != history {
                return Err("Failed/cancel import mutated document/history".into());
            }
            if outcome == "failed" {
                if app.exchange_import.error.is_none() {
                    return Err("Missing actual failure modal".into());
                }
                click(&mut app, &ctx, "exchange-import-error-ok")?;
                if app.exchange_import.owns_input() {
                    return Err("Failure modal not dismissed".into());
                }
                faults.push(json!({"case":name,"native_atomic_datum":true}));
                total += app.exchange_import_stats().0;
            } else if app.exchange_import_stats().0 != 0 {
                return Err("Cancel launched parser".into());
            }
        }
        let stable = scene(&app)?;
        let history = (app.undo_stack.len(), app.redo_stack.len());
        let parses = app.exchange_import_stats();
        for _ in 0..8 {
            frame(&mut app, &ctx, vec![]);
        }
        if scene(&app)? != stable
            || (app.undo_stack.len(), app.redo_stack.len()) != history
            || app.exchange_import_stats() != parses
        {
            return Err("Idle mutated import state".into());
        }
        cases.push(json!({"name":name,"saved":saved,"phases":case["phases"]}));
        std::fs::write(
            out.join("native-exchange.partial.json"),
            serde_json::to_vec_pretty(
                &json!({"cases":cases,"states":states,"status":app.status_message}),
            )
            .unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    // Native asynchronous safety control: source parsing is synchronous, so
    // this is a declared native admission guard, not a source UI cancellation.
    let mut retired = IngeTrazoApp::new();
    retired.system_clipboard = None;
    retired.native_gpu_viewport_enabled = true;
    retired.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
        &out.join("state/context-retired"),
    ));
    retired.open_document(&source.join(input["cases"][0]["input"].as_str().ok_or("input")?));
    retired.begin_exchange_import(
        &source.join(input["cases"][0]["file"].as_str().ok_or("file")?),
        ingetrazo_gui::exchange_import::Kind::Dae,
    )?;
    retired.scene.select(SelectionItem::Group(0));
    retired.begin_component_edit();
    if retired.component_edit_stack.is_empty() {
        return Err("Context retirement fixture did not enter".into());
    }
    let stable = scene(&retired)?;
    let history = (retired.undo_stack.len(), retired.redo_stack.len());
    let deadline = Instant::now() + Duration::from_secs(30);
    while retired.exchange_import_running() {
        retired.poll_exchange_import();
        if Instant::now() > deadline {
            return Err("Context retirement deadline".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if scene(&retired)? != stable
        || (retired.undo_stack.len(), retired.redo_stack.len()) != history
        || retired.exchange_import_stats() != (1, 0)
    {
        return Err("Stale context completion published".into());
    }
    std::env::set_var("PATH", old);
    std::fs::write(out.join("native-exchange.json"),serde_json::to_vec_pretty(&json!({"source_pin":input["source_pin"],"cases":cases,"states":states,"controls":{"actual_chooser":true,"worker_file_only":true,"parse_once":true,"camera_unchanged":true,"atomic_failure":true,"idle_readonly":true,"context_retired":true},"parses":total,"context_retired_parses":retired.exchange_import_stats().0,"native_fault_corrections":faults})).unwrap()).map_err(|e|e.to_string())?;
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
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        eprintln!("usage: exchange_import_workflows SOURCE OUTPUT");
        std::process::exit(2);
    }
    if let Err(e) = run(Path::new(&args[1]), Path::new(&args[2])) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
