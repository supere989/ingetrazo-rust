//! Actual native DWG chooser, unit modal, history and saved-document workflows.
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
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
    input.events = events;
    let _ = c.run(input, |ctx| {
        a.document_session_ui(ctx);
        ingetrazo_gui::dialogs::render_dialogs(a, ctx);
    });
}
fn click(a: &mut IngeTrazoApp, c: &egui::Context, id: &str) -> Result<(), String> {
    frame(a, c, vec![]);
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
fn temps() -> BTreeSet<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .map(|v| v.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ingetrazo-dwg-")
        })
        .collect()
}
fn record(a: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Value {
    json!({"case":name,"phase":phase,"scene":source_scene(a),"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"outcome":outcome,"camera":{"distance":a.camera.distance}})
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
fn satellite(args: &[String]) -> bool {
    if args.first().is_some_and(|s| s == "--title") {
        let value = std::env::var("DWG_FIXTURE_PICKER").unwrap_or_default();
        if value.is_empty() {
            std::process::exit(1)
        }
        println!("{value}");
        std::process::exit(0)
    }
    if args.first().is_some_and(|s| s == "-y") && args.get(1).is_some_and(|s| s == "-o") {
        let mode = Path::new(&args[3]).file_stem().unwrap().to_string_lossy();
        match mode.as_ref() {
            "warning-output" => {
                std::fs::write(&args[2], b"0\nEOF").unwrap();
                use std::io::Write;
                std::io::stderr().write_all(b"CA\xd1ERIAS\n").unwrap();
                std::process::exit(7)
            }
            "no-output" => {
                eprintln!("first\nsecond\nthird\nfourth");
                std::process::exit(3)
            }
            "timeout" => {
                std::thread::sleep(Duration::from_secs(1));
                std::process::exit(0)
            }
            _ => std::process::exit(2),
        }
    }
    false
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-dwg.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let picker = out.join("picker");
    std::fs::create_dir_all(&picker).map_err(|e| e.to_string())?;
    let picker_name = if cfg!(windows) {
        "kdialog.exe"
    } else {
        "kdialog"
    };
    std::fs::hard_link(&exe, picker.join(picker_name)).map_err(|e| e.to_string())?;
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let bundle = source.join("converter-bundle");
    let mut paths = vec![picker.clone(), bundle.join("vendor/libredwg/bin")];
    paths.extend(std::env::split_paths(&original_path));
    std::env::set_var(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    std::env::set_var("INGETRAZO_PDF_UI_OBSERVE", "1");
    let before_temps = temps();
    let mut states = Vec::new();
    let mut fit = true;
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("name")?;
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.set_language("en");
        a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        ));
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
        let before = source_scene(&a);
        let history = (a.undo_stack.len(), a.redo_stack.len());
        let camera_before = (a.camera.target, a.camera.distance);
        std::env::set_var(
            "DWG_FIXTURE_PICKER",
            if name == "chooser-cancel" {
                String::new()
            } else {
                source
                    .join(case["dwg"].as_str().ok_or("dwg")?)
                    .to_string_lossy()
                    .into_owned()
            },
        );
        a.choose_dwg_import();
        let ctx = egui::Context::default();
        if name != "chooser-cancel" {
            assert!(
                a.document_session.unit_import.is_some(),
                "{}",
                a.status_message
            );
            if case["cancel"] == true {
                click(&mut a, &ctx, "import-unit-cancel")?;
            } else {
                click(&mut a, &ctx, "import-unit-combo")?;
                click(&mut a, &ctx, "import-unit-option-m")?;
                click(&mut a, &ctx, "import-unit-accept")?;
            }
            assert!(
                a.document_session.unit_import.is_none(),
                "{}",
                a.status_message
            );
        }
        let cancelled = name == "chooser-cancel" || case["cancel"] == true;
        states.push(record(
            &a,
            name,
            "import",
            if cancelled { "cancel" } else { "accepted" },
        ));
        if cancelled {
            assert_eq!(source_scene(&a), before);
            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
        } else {
            assert_ne!(source_scene(&a), before);
            assert_eq!(a.undo_stack.len(), history.0 + 1);
            fit &= a.camera.distance.is_finite()
                && a.camera.distance > 0.
                && (a.camera.target, a.camera.distance) != camera_before;
            a.undo();
            states.push(record(&a, name, "undo", "accepted"));
            a.redo();
            states.push(record(&a, name, "redo", "accepted"));
            while !a.component_edit_stack.is_empty() {
                let depth = a.component_edit_stack.len();
                a.finish_component_edit();
                assert_eq!(
                    a.component_edit_stack.len(),
                    depth - 1,
                    "{}",
                    a.status_message
                );
            }
            a.save_to_path(&out.join(case["saved"].as_str().ok_or("saved")?));
            assert!(out.join(case["saved"].as_str().unwrap()).is_file());
        }
    }
    let mut repairs = Vec::new();
    for row in input["repairs"].as_array().ok_or("repairs")? {
        let bytes = unhex(row["input_hex"].as_str().ok_or("repair input")?)?;
        let (fixed, _, duplicates) = ingetrazo_formats::dwg::repair(&bytes)?;
        std::fs::write(out.join(row["file"].as_str().ok_or("repair file")?), fixed)
            .map_err(|e| e.to_string())?;
        repairs.push(json!({"name":row["name"],"input_hex":row["input_hex"],"file":row["file"],"duplicates":duplicates}));
    }
    let mut controls = Vec::new();
    for row in input["converter_controls"].as_array().ok_or("controls")? {
        let name = row["name"].as_str().ok_or("control name")?;
        let file = out.join(format!("{name}.fixture"));
        std::fs::write(&file, b"controlled process fixture").map_err(|e| e.to_string())?;
        let timeout = if name == "timeout" {
            Duration::from_millis(50)
        } else {
            ingetrazo_formats::dwg::TIMEOUT
        };
        match ingetrazo_formats::dwg::convert(&exe,&file,timeout){Ok(v)=>controls.push(json!({"name":name,"outcome":"accepted","warnings":v.warnings,"output_hex":hex(&v.bytes)})),Err(e)=>controls.push(json!({"name":name,"outcome":if name=="timeout"{"timeout"}else{"no-output"},"native_diagnostic":e,"deadline_seconds":if name=="timeout"{json!(0.05)}else{json!(300)}}))}
    }
    let discovery_root = out.join("discovery/checkouts/app");
    let vendor = discovery_root.join("vendor/libredwg/bin");
    let sibling = out.join("discovery/ingecad/vendor/libredwg/bin");
    let system = out.join("discovery/path");
    for p in [&vendor, &sibling, &system] {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        let f = p.join("dwg2dxf");
        std::fs::write(&f, b"fixture").map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(f, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }
    let active_path = std::env::var_os("PATH").unwrap();
    std::env::set_var("PATH", &system);
    let mut discovery = Vec::new();
    for (name, expected) in [
        ("vendor", Some(vendor.join("dwg2dxf"))),
        ("sibling", Some(sibling.join("dwg2dxf"))),
        ("PATH", Some(system.join("dwg2dxf"))),
        ("missing", None),
    ] {
        let found =
            ingetrazo_formats::dwg::find_tool_for_platform(&discovery_root, "dwg2dxf", false);
        assert_eq!(found, expected);
        discovery.push(json!({"name":name,"result":found.as_ref().map(|_|name)}));
        if let Some(p) = found {
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(vendor.join("dwg2dxf.exe"), b"MZ").map_err(|e| e.to_string())?;
    let found = ingetrazo_formats::dwg::find_tool_for_platform(&discovery_root, "dwg2dxf", true)
        .ok_or("Windows suffix discovery")?;
    assert_eq!(found, vendor.join("dwg2dxf.exe"));
    discovery.push(json!({"name":"windows-exe","result":"dwg2dxf.exe"}));
    assert!(
        ingetrazo_formats::dwg::find_tool_for_platform(&discovery_root, "dwg2dxf", false).is_none()
    );
    discovery.push(json!({"name":"linux-no-exe","result":null}));
    let mut guarded = IngeTrazoApp::new();
    guarded.open_document(&source.join(input["cases"][0]["input"].as_str().ok_or("guard input")?));
    let before = source_scene(&guarded);
    let history = (guarded.undo_stack.len(), guarded.redo_stack.len());
    let empty = out.join("empty-bundle");
    std::fs::create_dir_all(&empty).map_err(|e| e.to_string())?;
    std::env::set_var("PATH", "");
    assert!(guarded
        .begin_dwg_import_from_root(&source.join("entities-2d.dwg"), &empty)
        .is_err());
    assert_eq!(source_scene(&guarded), before);
    assert_eq!(
        (guarded.undo_stack.len(), guarded.redo_stack.len()),
        history
    );
    assert!(guarded.document_session.unit_import.is_none());
    std::env::set_var("PATH", active_path);
    let mut result = input.clone();
    result["states"] = json!(states);
    result["repairs"] = json!(repairs);
    result["discovery"] = json!(discovery);
    result["converter_controls"] = json!(controls);
    assert_eq!(
        temps(),
        before_temps,
        "owned temporary conversion files leaked"
    );
    std::fs::write(
        out.join("native-dwg.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(out.join("guards-dwg.json"),serde_json::to_vec_pretty(&json!({"actual_ingress":true,"atomic_failure":true,"temporary_cleanup":true,"successful_fit":fit})).unwrap()).map_err(|e|e.to_string())?;
    std::env::set_var("PATH", original_path);
    Ok(())
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    satellite(&args);
    let source = PathBuf::from(args.first().expect("SOURCE"));
    let out = PathBuf::from(args.get(1).expect("OUTPUT"));
    if let Err(e) = run(&source, &out) {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
