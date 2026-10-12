//! Actual native DWG chooser, unit modal, history and saved-document workflows.
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
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

fn qt_observed_mesh(mesh: &Mesh, m: DMat4) -> Value {
    let point = |i: usize| {
        let p = mesh.vertices[i].position;
        if m == m.as_mat4().as_dmat4() && p == p.as_vec3().as_dvec3() {
            ingetrazo_core::change_axes::map(m, p)
        } else {
            m.transform_point3(p)
        }
        .to_array()
    };
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
fn source_groups(app: &IngeTrazoApp) -> Vec<Group> {
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
    groups
}
fn source_scene(app: &IngeTrazoApp) -> Value {
    source_scene_from_groups(app, &source_groups(app))
}
fn source_scene_from_groups(app: &IngeTrazoApp, groups: &[Group]) -> Value {
    fn group(g: &Group, parent: DMat4) -> Value {
        let m = parent * g.transform;
        json!({"name":g.name,"layer":g.layer,"mesh":qt_observed_mesh(&g.mesh,m),"children":g.children.iter().map(|g|group(g,m)).collect::<Vec<_>>()})
    }
    let loose = app
        .component_edit_stack
        .first()
        .map_or(&app.scene.mesh, |s| &s.parked_scene().mesh);
    json!({"active":mesh_state(&app.scene.mesh,DMat4::IDENTITY),"loose":mesh_state(loose,DMat4::IDENTITY),"groups":groups.iter().map(|g|group(g,DMat4::IDENTITY)).collect::<Vec<_>>(),"layers":app.scene.layers.iter().map(|l|&l.name).collect::<Vec<_>>(),"units":app.scene.model_units()})
}

fn frame(a: &mut IngeTrazoApp, c: &egui::Context, events: Vec<egui::Event>) {
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1100., 800.),
    ));
    let tick = c.data_mut(|d| {
        let id = egui::Id::new("stl-ui-clock");
        let tick = d.get_temp::<u64>(id).unwrap_or(0) + 1;
        d.insert_temp(id, tick);
        tick
    });
    input.time = Some(tick as f64 * 0.1);
    input.events = events;
    let _ = c.run(input, |ctx| {
        a.update_ui(ctx);
    });
}
fn click(a: &mut IngeTrazoApp, c: &egui::Context, id: &str) -> Result<(), String> {
    // Modal/popup layout is measured on its first frame and positioned next frame.
    // Warm the actual UI before reading its observed rectangle, then hover first.
    for _ in 0..3 {
        frame(a, c, vec![]);
    }
    let initial = c
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or_else(|| format!("Missing actual unit UI {id}"))?;
    frame(a, c, vec![egui::Event::PointerMoved(initial.center())]);
    let rect = c
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
        .ok_or_else(|| format!("Missing actual unit UI {id}"))?;
    for pressed in [true, false] {
        frame(
            a,
            c,
            vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    Ok(())
}

fn prefs(a: &IngeTrazoApp) -> Value {
    let extra = &a.document_session.preferences.extra;
    json!({"import/stl_unit":extra.get("import/stl_unit"),"import/stl_simplify_mode":extra.get("import/stl_simplify_mode")})
}
fn record(a: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Value {
    json!({"case":name,"phase":phase,"scene":source_scene(a),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"preferences":prefs(a),"outcome":outcome,"camera":{"distance":a.camera.distance}})
}
fn checkpoint(out: &Path, a: &IngeTrazoApp, name: &str, states: &[Value]) -> Result<(), String> {
    std::fs::write(out.join("native-stl.partial.json"),serde_json::to_vec_pretty(&json!({"case":name,"states":states,"status":a.status_message,"error":a.stl_import.error})).unwrap()).map_err(|e|e.to_string())
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-stl.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::hard_link(
        exe,
        picker.join(if cfg!(windows) {
            "kdialog.exe"
        } else {
            "kdialog"
        }),
    )
    .map_err(|e| e.to_string())?;
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![picker];
    paths.extend(std::env::split_paths(&original_path));
    std::env::set_var(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let mut states = Vec::new();
    let mut cases = Vec::new();
    let mut total_parses = 0;
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.set_language("en");
        let mut store = ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        );
        store.config_dir = out.join("shared-config");
        a.attach_session_store(store);
        a.open_document(&source.join(case["input"].as_str().ok_or("input")?));
        a.current_file = None;
        if case["context"] == "leaf" {
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            a.scene.select(SelectionItem::Group(0));
            a.begin_component_edit();
            assert_eq!(a.component_edit_stack.len(), 2);
        }
        states.push(record(&a, name, "before", "idle"));
        checkpoint(out, &a, name, &states)?;
        let before = source_scene(&a);
        let beforeprefs = prefs(&a);
        let history = (a.undo_stack.len(), a.redo_stack.len());
        let camera = (a.camera.target, a.camera.distance);
        std::env::set_var(
            "STL_FIXTURE_PICKER",
            if name == "chooser-cancel" {
                String::new()
            } else {
                source
                    .join(case["stl"].as_str().ok_or("stl")?)
                    .to_string_lossy()
                    .into_owned()
            },
        );
        a.choose_stl_import();
        let ctx = egui::Context::default();
        if name != "chooser-cancel" {
            assert!(a.stl_import.dialog.is_some(), "{}", a.status_message);
            if name == "unit-cancel" {
                click(&mut a, &ctx, "stl-ui-cancel")?;
            } else {
                let unit = case["unit"].as_str().ok_or("unit")?;
                let mode = case["simplify"].as_str().ok_or("simplify")?;
                click(&mut a, &ctx, "stl-ui-unit-combo")?;
                click(&mut a, &ctx, &format!("stl-ui-unit-{unit}"))?;
                click(&mut a, &ctx, "stl-ui-mode-combo")?;
                click(&mut a, &ctx, &format!("stl-ui-mode-{mode}"))?;
                assert_eq!(a.stl_import.dialog.as_ref().unwrap().unit, unit);
                assert_eq!(a.stl_import.dialog.as_ref().unwrap().simplify, mode);
                click(&mut a, &ctx, "stl-ui-import")?;
                let deadline = std::time::Instant::now() + Duration::from_secs(30);
                while a.stl_import.dialog.is_none()
                    && a.stl_import.owns_input()
                    && a.stl_import.error.is_none()
                {
                    if std::time::Instant::now() > deadline {
                        return Err(format!("STL worker timed out: {name}"));
                    }
                    frame(&mut a, &ctx, vec![]);
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        }
        let outcome = if name == "chooser-cancel" || name == "unit-cancel" {
            "cancel"
        } else if a.stl_import.error.is_some() {
            "failed"
        } else if source_scene(&a) != before {
            "accepted"
        } else {
            "missing"
        };
        states.push(record(&a, name, "import", outcome));
        checkpoint(out, &a, name, &states)?;
        assert_eq!(
            (a.camera.target, a.camera.distance),
            camera,
            "STL changed camera"
        );
        let (parses, completed) = a.stl_import_stats();
        total_parses += parses;
        let saved = if outcome == "accepted" {
            assert_eq!(parses, 1);
            assert_eq!(completed, 1);
            assert_eq!(a.undo_stack.len(), history.0 + 1);
            a.undo();
            states.push(record(&a, name, "undo", "accepted"));
            assert_eq!(source_scene(&a), before);
            a.redo();
            states.push(record(&a, name, "redo", "accepted"));
            while !a.component_edit_stack.is_empty() {
                a.finish_component_edit();
            }
            let name = case["saved"].as_str().ok_or("saved")?;
            a.save_to_path(&out.join(name));
            if !out.join(name).is_file() {
                return Err(format!("STL App Save failed: {}", a.status_message));
            }
            Some(name.to_string())
        } else {
            assert_ne!(outcome, "missing", "worker did not publish or report error");
            assert_eq!(source_scene(&a), before);
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
            if outcome == "cancel" {
                assert_eq!(parses, 0);
                assert_eq!(prefs(&a), beforeprefs);
            } else {
                assert_eq!(parses, 1);
                assert_eq!(completed, 0);
            }
            None
        };
        cases.push(json!({"name":name,"saved":saved,"phases":states.iter().filter(|v|v["case"]==name).map(|v|v["phase"].clone()).collect::<Vec<_>>() }));
        checkpoint(out, &a, name, &states)?;
    }
    let mut controls = Vec::new();
    for control in input["decoder_controls"]
        .as_array()
        .ok_or("decoder_controls")?
    {
        let name = control["name"].as_str().ok_or("control.name")?;
        let file = control["file"].as_str().ok_or("control.file")?;
        let scale = control["scale"].as_f64().ok_or("scale")?;
        let key = control["simplify"].as_str().ok_or("simplify")?;
        let result = ingetrazo_formats::stl::Simplification::parse(key).and_then(|mode| {
            ingetrazo_formats::stl::parse_stl_path(&source.join(file), scale, mode)
        });
        match result{Ok(plan)=>controls.push(json!({"name":name,"file":file,"scale":control["scale"],"simplify":key,"outcome":"accepted","mesh":qt_observed_mesh(&plan.mesh,DMat4::IDENTITY)})),Err(e)=>controls.push(json!({"name":name,"file":file,"scale":control["scale"],"simplify":key,"outcome":"refused","native_diagnostic":e.to_string()}))}
    }
    std::env::set_var("PATH", original_path);
    std::fs::write(out.join("native-stl.json"),serde_json::to_vec_pretty(&json!({"source_pin":input["source_pin"],"cases":cases,"states":states,"decoder_controls":controls,"guards":{"actual_ingress":true,"atomic_failure":true,"parse_once":true,"worker_mesh_only":true,"camera_unchanged":true},"parse_count":total_parses})).unwrap()).map_err(|e|e.to_string())?;
    Ok(())
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "--title") {
        let value = std::env::var("STL_FIXTURE_PICKER").unwrap_or_default();
        if value.is_empty() {
            std::process::exit(1);
        }
        println!("{value}");
        return;
    }
    if args.len() != 2 {
        eprintln!("usage: stl_import_workflows SOURCE OUTPUT");
        std::process::exit(2);
    }
    if let Err(e) = run(Path::new(&args[0]), Path::new(&args[1])) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
