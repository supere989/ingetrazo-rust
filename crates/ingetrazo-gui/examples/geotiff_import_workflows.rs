//! Actual cold GeoTIFF worker/import/history/resource workflows.
use glam::{DMat4, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::SelectionItem};
use ingetrazo_gui::IngeTrazoApp;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
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

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn image_resource(
    a: &IngeTrazoApp,
    path: &str,
) -> Result<(Vec<u8>, ingetrazo_render::source_texture_decode::Decoded), String> {
    let bytes = if let Some(v) = a.scene.embedded_textures.get(path) {
        v.clone()
    } else {
        std::fs::read(path).map_err(|e| e.to_string())?
    };
    let pixels = ingetrazo_render::source_texture_decode::decode(&bytes, Default::default())?;
    Ok((bytes, pixels))
}
pub(crate) fn scene(a: &IngeTrazoApp) -> Result<Value, String> {
    let geometry = source_scene(a);
    let mut images = Vec::new();
    for image in &a.scene.image_planes {
        let (bytes, pixels) = image_resource(a, &image.path)?;
        let mut row = image.to_json();
        row["texture"]["path"] = json!({"encoded_sha256":sha(&bytes),"rgba_sha256":sha(&pixels.rgba),"size":[pixels.width,pixels.height]});
        images.push(row);
    }
    let datum = a.scene.geo_datum.as_ref().map(|d| {
        let mut v = json!({"lat":d.lat,"lon":d.lon,"alt":d.alt});
        if d.north != 0. {
            v["north"] = json!(d.north);
        }
        v
    });
    let layers = a
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
    let mut selection = Vec::new();
    for selected in &a.scene.selection {
        let (kind, index) = match selected {
            SelectionItem::ImagePlane(i) => ("image", *i),
            SelectionItem::Group(i) => ("group", *i),
            _ => return Err(format!("Unclaimed selected geometry {selected:?}")),
        };
        selection.push(json!({"kind":kind,"index":index}));
    }
    selection.sort_by_key(|v| v.to_string());
    Ok(
        json!({"datum":datum,"images":images,"layers":layers,"geometry":{"active":geometry["active"],"loose":geometry["loose"],"groups":geometry["groups"]},"selection":selection,"units":geometry["units"]}),
    )
}
pub(crate) fn camera(a: &IngeTrazoApp) -> Value {
    json!({"target":a.camera.target.to_array(),"distance":a.camera.distance,"yaw":a.camera.yaw,"pitch":a.camera.pitch,"projection":if a.camera.perspective{"perspective"}else{"parallel"},"fov":a.camera.fov_deg})
}
pub(crate) fn initial_camera(a: &mut IngeTrazoApp, v: &Value) -> Result<(), String> {
    let coordinates = v["target"].as_array().ok_or("initial_camera target")?;
    if coordinates.len() != 3 {
        return Err("camera target count".into());
    }
    a.camera.target = DVec3::new(
        coordinates[0].as_f64().ok_or("target x")?,
        coordinates[1].as_f64().ok_or("target y")?,
        coordinates[2].as_f64().ok_or("target z")?,
    );
    a.camera.distance = v["distance"].as_f64().ok_or("initial distance")?;
    a.camera.yaw = v["yaw"].as_f64().ok_or("initial yaw")?;
    a.camera.pitch = v["pitch"].as_f64().ok_or("initial pitch")?;
    a.camera.perspective = match v["projection"].as_str() {
        Some("perspective") => true,
        Some("parallel") => false,
        _ => return Err("camera projection".into()),
    };
    a.camera.fov_deg = v["fov"].as_f64().ok_or("initial fov")?;
    Ok(())
}
fn record(a: &IngeTrazoApp, name: &str, phase: &str, outcome: &str) -> Result<Value, String> {
    Ok(
        json!({"case":name,"phase":phase,"scene":scene(a)?,"undo":a.undo_stack.len(),"redo":a.redo_stack.len(),"camera":camera(a),"outcome":outcome}),
    )
}
fn poll(a: &mut IngeTrazoApp) -> Result<(), String> {
    let start = Instant::now();
    while a.geotiff_import.pending() {
        a.poll_geotiff_import();
        if start.elapsed() > Duration::from_secs(30) {
            return Err("Actual GeoTIFF worker timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
fn picker(args: &[String]) {
    if args.first().is_some_and(|v| v == "--title") {
        let answer = std::env::var("GEOTIFF_FIXTURE_PICKER").unwrap_or_default();
        if answer.is_empty() {
            std::process::exit(1)
        }
        println!("{answer}");
        std::process::exit(0)
    }
}
fn run(source: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_slice(
        &std::fs::read(source.join("original-geotiff.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let picker_dir = out.join("picker");
    std::fs::create_dir_all(&picker_dir).map_err(|e| e.to_string())?;
    std::fs::hard_link(
        std::env::current_exe().map_err(|e| e.to_string())?,
        picker_dir.join(if cfg!(windows) {
            "kdialog.exe"
        } else {
            "kdialog"
        }),
    )
    .map_err(|e| e.to_string())?;
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![picker_dir];
    paths.extend(std::env::split_paths(&old_path));
    std::env::set_var(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    let mut decodes = Vec::new();
    let mut stats = Vec::new();
    let mut admission = true;
    for row in input["decodes"].as_array().ok_or("decodes")? {
        let info = ingetrazo_formats::geotiff::read_info(
            &source.join(row["input"].as_str().ok_or("TIFF input")?),
            Default::default(),
        )?;
        let decoded = ingetrazo_formats::geotiff::read_rgba(
            &info,
            row["max_px"].as_u64().ok_or("max_px")? as usize,
            Default::default(),
            |_| true,
        )?;
        admission &= !decoded.stats.full_resolution_fallback
            && decoded.stats.peak_logical_bytes
                <= ingetrazo_formats::geotiff::Limits::default().memory_bytes;
        let destination = out.join(row["rgba"].as_str().ok_or("decode RGBA path")?);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(destination, &decoded.rgba).map_err(|e| e.to_string())?;
        let mut observed = row.clone();
        observed["factor"] = json!(decoded.factor);
        observed["size"] = json!([decoded.width, decoded.height]);
        decodes.push(observed);
        stats.push(json!({"case":row["case"],"stats":decoded.stats}));
    }
    let cancel_info = ingetrazo_formats::geotiff::read_info(
        &source.join("strip-raw-rgba8.tif"),
        Default::default(),
    )?;
    let error =
        match ingetrazo_formats::geotiff::read_rgba(&cancel_info, 8192, Default::default(), |_| {
            false
        }) {
            Err(e) => e,
            Ok(_) => return Err("Decoder API cancellation ignored".into()),
        };
    let decoder_controls = json!({"api_cancel":{"refused":true,"message":error}});
    let tiny = cancel_info.retained_bytes()?;
    if ingetrazo_formats::geotiff::read_rgba(
        &cancel_info,
        8192,
        ingetrazo_formats::geotiff::Limits { memory_bytes: tiny },
        |_| true,
    )
    .is_ok()
    {
        return Err("Actual tiny-limit decoder admission unexpectedly succeeded".into());
    }

    let mut states = Vec::new();
    let mut resources = Vec::new();
    let mut atomic_failure = false;
    let mut chooser_cancel = false;
    for case in input["cases"].as_array().ok_or("cases")? {
        let name = case["name"].as_str().ok_or("case name")?;
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.set_language("en");
        a.attach_session_store(ingetrazo_gui::session_state::SessionStore::isolated(
            &out.join(format!("state/{name}")),
        ));
        a.open_document(&source.join(case["input"].as_str().ok_or("initial IGZ")?));
        a.current_file = None;
        initial_camera(&mut a, &case["initial_camera"])?;
        for index in case["edit_path"].as_array().ok_or("edit_path")? {
            a.scene.select(SelectionItem::Group(
                index.as_u64().ok_or("edit index")? as usize
            ));
            a.begin_component_edit();
        }
        initial_camera(&mut a, &case["initial_camera"])?;
        let mut imported = false;
        for phase in case["phases"].as_array().ok_or("phases")? {
            let phase = phase.as_str().ok_or("phase")?;
            let outcome = match phase {
                "before" => "ok",
                "import" | "cancel" | "failed" => {
                    let before = a.scene.clone();
                    let history = (a.undo_stack.len(), a.redo_stack.len());
                    let pose = a.camera.clone();
                    if name == "chooser-cancel" {
                        std::env::set_var("GEOTIFF_FIXTURE_PICKER", "");
                        a.begin_geotiff_import_dialog();
                        assert!(!a.geotiff_import.pending());
                        assert_eq!(a.scene, before);
                        assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
                        assert_eq!(a.camera, pose);
                        chooser_cancel = true;
                        "ok"
                    } else {
                        let path = source.join(case["tiff"].as_str().ok_or("case TIFF")?);
                        let ceiling =
                            case["device_ceiling"].as_u64().ok_or("device ceiling")? as usize;
                        a.set_geotiff_device_ceiling(Some(ceiling));
                        std::env::set_var(
                            "GEOTIFF_FIXTURE_PICKER",
                            path.to_string_lossy().as_ref(),
                        );
                        a.begin_geotiff_import_dialog();
                        poll(&mut a)?;
                        if a.geotiff_import.error.is_some() {
                            if name != "decode-failure-datum" {
                                return Err(format!(
                                    "Unexpected GeoTIFF import refusal {name}: {}",
                                    a.status_message
                                ));
                            }
                            assert_eq!(a.scene, before);
                            assert_eq!((a.undo_stack.len(), a.redo_stack.len()), history);
                            assert_eq!(a.camera, pose);
                            atomic_failure = true;
                            "native-atomic-refusal"
                        } else {
                            if name == "decode-failure-datum" {
                                return Err("Corrupt GeoTIFF unexpectedly published".into());
                            }
                            assert_eq!(a.scene.image_planes.len(), before.image_planes.len() + 1);
                            assert_eq!(a.undo_stack.len(), history.0 + 1);
                            let summary = a
                                .geotiff_import
                                .last
                                .as_ref()
                                .ok_or("No actual worker summary")?;
                            admission &= !summary.stats.full_resolution_fallback;
                            imported = true;
                            "ok"
                        }
                    }
                }
                "undo" => {
                    a.undo();
                    "ok"
                }
                "redo" => {
                    a.redo();
                    "ok"
                }
                "finish" => {
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
                    "ok"
                }
                _ => return Err(format!("Unclaimed phase {phase}")),
            };
            states.push(record(&a, name, phase, outcome)?);
            std::fs::write(
                out.join("native-geotiff.partial.json"),
                serde_json::to_vec_pretty(
                    &json!({"complete":false,"states":states,"status":a.status_message}),
                )
                .unwrap(),
            )
            .map_err(|e| e.to_string())?;
        }
        if imported {
            for row in input["resources"]
                .as_array()
                .ok_or("resources")?
                .iter()
                .filter(|r| r["case"] == name)
            {
                let summary = a.geotiff_import.last.as_ref().ok_or("summary")?;
                let (bytes, pixels) = image_resource(&a, &summary.resource)?;
                for (field, data) in [("file", bytes), ("rgba", pixels.rgba)] {
                    let p = out.join(row[field].as_str().ok_or("resource output")?);
                    if let Some(parent) = p.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(p, data).map_err(|e| e.to_string())?;
                }
                let mut value = row.clone();
                value["size"] = json!([pixels.width, pixels.height]);
                resources.push(value);
            }
        }
        if let Some(saved) = case["saved"].as_str() {
            a.save_to_path(&out.join(saved));
            assert!(out.join(saved).is_file());
        }
    }
    let mut output = input.clone();
    output["states"] = json!(states);
    output["decodes"] = json!(decodes);
    output["resources"] = json!(resources);
    output["decoder_controls"] = decoder_controls;
    std::fs::write(
        out.join("native-geotiff.json"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("native-decode-stats.json"),
        serde_json::to_vec_pretty(&stats).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(out.join("guards-geotiff.json"),serde_json::to_vec_pretty(&json!({"actual_ingress":true,"atomic_failure":atomic_failure,"chooser_cancel":chooser_cancel,"streaming_admission":admission,"tiny_limit_refusal":true})).unwrap()).map_err(|e|e.to_string())?;
    std::env::set_var("PATH", old_path);
    Ok(())
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    picker(&args);
    let source = PathBuf::from(args.first().expect("SOURCE"));
    let out = PathBuf::from(args.get(1).expect("OUTPUT"));
    if let Err(e) = run(&source, &out) {
        let _ = std::fs::write(
            out.join("result.json"),
            serde_json::to_vec_pretty(&json!({"ok":false,"error":e})).unwrap(),
        );
        eprintln!("{e}");
        std::process::exit(1)
    }
}
