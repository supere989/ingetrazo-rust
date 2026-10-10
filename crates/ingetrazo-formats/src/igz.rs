// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Native IngeTrazo document format (.igz).
//!
//! Format 1: Plain JSON
//! Format 2: ZIP container with document.json and textures/

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use ingetrazo_core::dimensions::Dimension;
use ingetrazo_core::groups::Group;
use ingetrazo_core::guides::Guide;
use ingetrazo_core::layers::Layer;
use ingetrazo_core::materials::Material;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_core::scene::Scene;
use ingetrazo_core::units::LengthUnit;

pub const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

#[derive(thiserror::Error, Debug)]
pub enum IgzError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Invalid document structure: {0}")]
    InvalidDocument(String),
}

#[derive(Serialize, Deserialize)]
pub struct IgzEnvelope {
    pub igz_format: u32,
    pub app_version: String,
    pub scene: serde_json::Value,
}

fn mesh_to_json(mesh: &Mesh) -> serde_json::Value {
    let mut edges_json = Vec::new();
    for e in &mesh.edges {
        let p0 = mesh.vertices[e.v0].position;
        let p1 = mesh.vertices[e.v1].position;
        let mut obj = serde_json::json!({
            "a": [p0.x, p0.y, p0.z],
            "b": [p1.x, p1.y, p1.z],
        });
        if e.soft {
            obj["soft"] = serde_json::Value::Bool(true);
        }
        if e.hidden {
            obj["hidden"] = serde_json::Value::Bool(true);
        }
        if let Some(ref l) = e.layer {
            obj["layer"] = serde_json::Value::String(l.clone());
        }
        for (key, value) in &e.attrs {
            if !["a", "b", "soft", "hidden", "layer"].contains(&key.as_str()) {
                obj[key] = value.clone();
            }
        }
        edges_json.push(obj);
    }

    let mut faces_json = Vec::new();
    for f in &mesh.faces {
        let verts: Vec<Vec<f64>> = f.vertices.iter().map(|&v| {
            let p = mesh.vertices[v].position;
            vec![p.x, p.y, p.z]
        }).collect();

        let holes: Vec<Vec<Vec<f64>>> = f.holes.iter().map(|h| {
            h.iter().map(|&v| {
                let p = mesh.vertices[v].position;
                vec![p.x, p.y, p.z]
            }).collect()
        }).collect();

        let mut obj = serde_json::json!({
            "vertices": verts,
        });
        if !holes.is_empty() {
            obj["holes"] = serde_json::json!(holes);
        }
        if let Some(ref l) = f.layer {
            obj["layer"] = serde_json::Value::String(l.clone());
        }
        for (k, v) in &f.attrs {
            obj[k] = v.clone();
        }
        faces_json.push(obj);
    }

    serde_json::json!({
        "edges": edges_json,
        "faces": faces_json,
    })
}

fn json_to_mesh(val: &serde_json::Value) -> Mesh {
    let mut mesh = Mesh::new();
    if let Some(edges) = val.get("edges").and_then(|e| e.as_array()) {
        for e in edges {
            if let (Some(a), Some(b)) = (e.get("a").and_then(|v| v.as_array()), e.get("b").and_then(|v| v.as_array())) {
                if a.len() >= 3 && b.len() >= 3 {
                    let pa = DVec3::new(
                        a[0].as_f64().unwrap_or(0.0),
                        a[1].as_f64().unwrap_or(0.0),
                        a[2].as_f64().unwrap_or(0.0),
                    );
                    let pb = DVec3::new(
                        b[0].as_f64().unwrap_or(0.0),
                        b[1].as_f64().unwrap_or(0.0),
                        b[2].as_f64().unwrap_or(0.0),
                    );
                    let va = mesh.add_vertex(pa);
                    let vb = mesh.add_vertex(pb);
                    if let Some(edge_id) = mesh.add_edge(va, vb) {
                        if let Some(obj) = e.as_object() {
                            for (key,value) in obj {
                                if !["a","b","soft","hidden","layer"].contains(&key.as_str()) {
                                    mesh.edges[edge_id].attrs.insert(key.clone(),value.clone());
                                }
                            }
                        }
                        if e.get("soft").and_then(|s| s.as_bool()).unwrap_or(false) {
                            mesh.edges[edge_id].soft = true;
                        }
                        if e.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false) {
                            mesh.edges[edge_id].hidden = true;
                        }
                        if let Some(layer) = e.get("layer").and_then(|l| l.as_str()) {
                            mesh.edges[edge_id].layer = Some(layer.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Some(faces) = val.get("faces").and_then(|f| f.as_array()) {
        for f in faces {
            if let Some(verts_arr) = f.get("vertices").and_then(|v| v.as_array()) {
                let mut loop_verts = Vec::new();
                for pt in verts_arr {
                    if let Some(coords) = pt.as_array() {
                        if coords.len() >= 3 {
                            let p = DVec3::new(
                                coords[0].as_f64().unwrap_or(0.0),
                                coords[1].as_f64().unwrap_or(0.0),
                                coords[2].as_f64().unwrap_or(0.0),
                            );
                            loop_verts.push(mesh.add_vertex(p));
                        }
                    }
                }

                let mut holes_verts = Vec::new();
                if let Some(holes_arr) = f.get("holes").and_then(|h| h.as_array()) {
                    for hole in holes_arr {
                        if let Some(h_pts) = hole.as_array() {
                            let mut h_loop = Vec::new();
                            for pt in h_pts {
                                if let Some(coords) = pt.as_array() {
                                    if coords.len() >= 3 {
                                        let p = DVec3::new(
                                            coords[0].as_f64().unwrap_or(0.0),
                                            coords[1].as_f64().unwrap_or(0.0),
                                            coords[2].as_f64().unwrap_or(0.0),
                                        );
                                        h_loop.push(mesh.add_vertex(p));
                                    }
                                }
                            }
                            if h_loop.len() >= 3 {
                                holes_verts.push(h_loop);
                            }
                        }
                    }
                }

                if loop_verts.len() >= 3 {
                    if let Some(face_id) = mesh.add_face(loop_verts, holes_verts) {
                        if let Some(layer) = f.get("layer").and_then(|l| l.as_str()) {
                            mesh.faces[face_id].layer = Some(layer.to_string());
                        }
                        if let Some(obj) = f.as_object() {
                            for (k, v) in obj {
                                if k != "vertices" && k != "holes" && k != "layer" {
                                    mesh.faces[face_id].attrs.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    mesh
}

/// Overlay changed modeled fields only. Unknown and unmodeled native data
/// remain owned by the document, rather than discarded during deserialization.
fn overlay(original: &serde_json::Value, baseline: &serde_json::Value,
           current: serde_json::Value) -> serde_json::Value {
    if !original.is_object() { return current; }
    let mut result = original.clone();
    for (key, value) in current.as_object().unwrap() {
        if baseline.get(key) != Some(value) {
            // Geometry metadata is carried by MeshEdge/Face identity. Overlaying
            // by array position would transfer deleted entities' attrs to new ones.
            result[key] = if key == "edges" || key == "faces" {
                value.clone()
            } else { merge_changed(result.get(key), baseline.get(key), value) };

        }
    }
    result
}

fn merge_changed(original: Option<&serde_json::Value>, baseline: Option<&serde_json::Value>,
                 current: &serde_json::Value) -> serde_json::Value {
    if baseline == Some(current) {
        return original.unwrap_or(current).clone();
    }
    if let (Some(o), Some(b)) = (original, baseline) {
        if current.is_object() && o.is_object() && b.is_object() {
            return overlay(o, b, current.clone());
        }
        if let (Some(oa), Some(ba), Some(ca)) = (o.as_array(), b.as_array(), current.as_array()) {
            if ca.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some())
                && ba.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some())
                && oa.iter().all(|v| v.get("name").and_then(|v| v.as_str()).is_some()) {
                return serde_json::json!(ca.iter().map(|c| {
                    let name = c.get("name");
                    let o = oa.iter().find(|v| v.get("name") == name);
                    let b = ba.iter().find(|v| v.get("name") == name);
                    merge_changed(o,b,c)
                }).collect::<Vec<_>>());
            }
            if oa.len() == ba.len() && ba.len() == ca.len() {
                return serde_json::json!(ca.iter().enumerate().map(|(i,c)|
                    merge_changed(Some(&oa[i]),Some(&ba[i]),c)).collect::<Vec<_>>());
            }
        }
    }
    current.clone()
}

fn group_model(g: &Group) -> serde_json::Value {
    let mut obj = mesh_to_json(&g.mesh);
    obj["name"] = serde_json::json!(g.name);
    obj["xform"] = serde_json::json!(g.transform.to_cols_array());
    obj["layer"] = serde_json::json!(g.layer);
    obj["ifc"] = serde_json::json!(g.ifc_tag);
    obj["hidden"] = serde_json::json!(g.hidden);
    obj["billboard"] = serde_json::json!(g.billboard);
    obj["children"] = serde_json::json!(g.children.iter().map(group_model).collect::<Vec<_>>());
    obj
}

fn group_to_json(g: &Group, protos: &mut Vec<serde_json::Value>) -> serde_json::Value {
    let model = group_model(g);
    let mut obj = overlay(&g.native_document, &g.native_baseline, model.clone());
    // Rust meshes are independent copies. Editing an imported instance makes a
    // new definition; never mutate a retained prototype used by other instances.
    let geometry_changed = ["edges", "faces"].iter()
        .any(|k| g.native_baseline.get(*k) != model.get(*k));
    if g.native_document.get("proto").is_some() && geometry_changed {
        obj["proto"] = serde_json::json!(protos.len());
        let original_mesh = g.native_document.get("proto").and_then(|v| v.as_u64())
            .and_then(|i| protos.get(i as usize)).unwrap_or(&g.native_document);
        let baseline_mesh = serde_json::json!({
            "edges": g.native_baseline.get("edges"), "faces": g.native_baseline.get("faces")
        });
        let preserved_mesh = overlay(original_mesh, &baseline_mesh, mesh_to_json(&g.mesh));
        protos.push(preserved_mesh);
        obj.as_object_mut().unwrap().remove("edges");
        obj.as_object_mut().unwrap().remove("faces");
    } else if g.native_document.is_null()
        || (g.native_document.get("proto").is_none()
            && g.native_baseline.get("xform") != model.get("xform")) {
        // Python only applies xform to prototype instances.
        obj["proto"] = serde_json::json!(protos.len());
        let baseline_mesh = serde_json::json!({
            "edges": g.native_baseline.get("edges"), "faces": g.native_baseline.get("faces")
        });
        protos.push(overlay(&g.native_document, &baseline_mesh, mesh_to_json(&g.mesh)));
        obj.as_object_mut().unwrap().remove("edges");
        obj.as_object_mut().unwrap().remove("faces");
    }
    let children = g.children.iter().map(|c| group_to_json(c, protos)).collect::<Vec<_>>();
    if g.native_baseline.get("children") != model.get("children") {
        obj["children"] = serde_json::json!(children);
    }
    obj
}

fn material_model(mat: &Material) -> serde_json::Value {
    serde_json::json!({
        "name": mat.name, "color": mat.color, "opacity": mat.opacity,
        "roughness": mat.roughness,
        "texture": {"path": mat.texture_path, "embed": mat.texture_embed}
    })
}

fn point_model(point: &ingetrazo_core::georef::GeoPoint) -> serde_json::Value {
    serde_json::json!({"position":point.position,"name":point.name,"desc":point.desc})
}

fn path_model(path: &ingetrazo_core::georef::GeoPath) -> serde_json::Value {
    use ingetrazo_core::georef::SurfaceFillMode;
    let surface = path.surface.as_ref().map(|mode| match mode {
        SurfaceFillMode::Flat => "flat", SurfaceFillMode::Draped => "draped"
    });
    serde_json::json!({"points":path.points,"closed":path.closed,"name":path.name,"surface":surface})
}

fn scene_model(scene: &Scene) -> serde_json::Value {
    let loose = mesh_to_json(&scene.mesh);
    let groups_json: Vec<_> = scene.groups.iter().map(group_model).collect();
    let mut layers_json = Vec::new();
    for l in &scene.layers {
        layers_json.push(serde_json::json!({
            "name": l.name,
            "visible": l.visible,
            "locked": l.locked,
            "color": l.color,
        }));
    }

    let mut doc = loose;
    doc["groups"] = serde_json::json!(groups_json);
    doc["layers"] = serde_json::json!(layers_json);
    let mut names: Vec<_> = scene.materials.keys().collect();
    names.sort_by_key(|name| {
        scene.native_document.get("materials").and_then(|v| v.as_array())
            .and_then(|a| a.iter().position(|v| v.get("name").and_then(|v| v.as_str()) == Some(name.as_str())))
            .map(|i| (i, (*name).clone())).unwrap_or((usize::MAX, (*name).clone()))
    });
    doc["materials"] = serde_json::json!(names.iter().map(|name| {
        let mat = &scene.materials[*name];
        overlay(&mat.native_document, &mat.native_baseline, material_model(mat))
    }).collect::<Vec<_>>());
    doc["camera"] = serde_json::json!({
        "target": [scene.camera.target.x, scene.camera.target.y, scene.camera.target.z],
        "distance": scene.camera.distance,
        "yaw": scene.camera.yaw,
        "pitch": scene.camera.pitch,
        "perspective": scene.camera.perspective,
        "fov_deg": scene.camera.fov_deg,
    });
    doc["dimensions"] = serde_json::to_value(&scene.dimensions).unwrap_or(serde_json::json!([]));
    doc["guides"] = serde_json::to_value(&scene.guides).unwrap_or(serde_json::json!([]));
    doc["text_labels"] = serde_json::to_value(&scene.text_labels).unwrap_or(serde_json::json!([]));
    let unit = match scene.unit {
        LengthUnit::Meters => "m", LengthUnit::Centimeters => "cm",
        LengthUnit::Millimeters => "mm", LengthUnit::Inches => "in", LengthUnit::Feet => "ft"
    };
    doc["units"] = serde_json::json!({"length":unit,"precision":2});

    doc["saved_views"] = serde_json::json!(scene.saved_views.iter().map(|v| serde_json::json!({
        "name":v.name, "target":v.target, "distance":v.distance, "yaw":v.yaw,
        "pitch":v.pitch, "fov":v.fov_deg, "parallel":!v.perspective || v.parallel,
        "hidden_layers":v.hidden_layers
    })).collect::<Vec<_>>());
    doc["section_planes"] = serde_json::json!(scene.section_planes.iter().map(|v| serde_json::json!({
        "point":v.point, "normal":v.normal, "name":v.name, "symbol":v.symbol,
        "active":v.active, "uid":v.id.as_u128() as u64
    })).collect::<Vec<_>>());
    doc["section_view"] = serde_json::json!({
        "planes_hidden": !scene.show_section_planes, "cuts_hidden": !scene.show_section_cuts,
        "fill_hidden": !scene.show_section_fill
    });
    doc["georef"] = serde_json::json!({"datum": scene.geo_datum.as_ref().map(|d|
        serde_json::json!({"lat":d.lat,"lon":d.lon,"alt":d.alt,"north":d.north}))});
    doc["geo_points"] = serde_json::json!(scene.geo_points.iter().map(|p|
        overlay(&p.native_document,&p.native_baseline,point_model(p))).collect::<Vec<_>>());
    doc["geo_paths"] = serde_json::json!(scene.geo_paths.iter().map(|p|
        overlay(&p.native_document,&p.native_baseline,path_model(p))).collect::<Vec<_>>());
    doc["terrain"] = serde_json::json!(scene.terrain);
    doc["compositions"] = serde_json::json!(scene.sheets);
    doc["active_sheet_index"] = serde_json::json!(scene.active_sheet_index);
    doc
}

pub fn scene_to_igz_document(scene: &Scene) -> serde_json::Value {
    let current = scene_model(scene);
    let mut doc = overlay(&scene.native_document, &scene.native_baseline, current.clone());
    if scene.native_baseline.get("groups") != current.get("groups") {
        let mut protos = doc.get("protos").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        doc["groups"] = serde_json::json!(scene.groups.iter().map(|g| group_to_json(g, &mut protos)).collect::<Vec<_>>());
        if !protos.is_empty() { doc["protos"] = serde_json::json!(protos); }
    }
    doc
}

fn validate_native_save(scene: &Scene) -> Result<(), IgzError> {
    let current = scene_model(scene);
    // These modeled types still have incompatible original schemas. Preserve
    // original records for no-op saves; refuse edits instead of writing a file
    // the Python reader silently misinterprets.
    {
        for key in ["compositions", "terrain"] {
            let value = current.get(key).unwrap();
            let nonempty = !value.is_null() && value.as_array().map(|a| !a.is_empty()).unwrap_or(true);
            if (scene.native_document.is_object() && scene.native_baseline.get(key) != Some(value))
                || (!scene.native_document.is_object() && nonempty) {
                return Err(IgzError::InvalidDocument(format!(
                    "editing {key} in an imported document requires a native schema adapter")));
            }
        }
    }
    Ok(())
}

pub fn save_igz_format1(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    validate_native_save(scene)?;
    let envelope = IgzEnvelope {
        igz_format: 1,
        app_version: "0.3.15".to_string(),
        scene: scene_to_igz_document(scene),
    };
    let json_bytes = serde_json::to_vec_pretty(&envelope)?;
    let mut file = File::create(path)?;
    file.write_all(&json_bytes)?;
    Ok(())
}

pub fn save_igz_format2(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    validate_native_save(scene)?;
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut modified_scene = scene.clone();
    let mut written = std::collections::HashSet::new();
    for (name, bytes) in &scene.embedded_textures {
        zip.start_file(name, options)?;
        zip.write_all(bytes)?;
        written.insert(name.clone());
    }
    for (_mat_name, mat) in &mut modified_scene.materials {
        if let Some(tex_path) = &mat.texture_path {
            let p = Path::new(tex_path);
            if p.exists() {
                if let Ok(data) = std::fs::read(p) {
                    let filename = p.file_name().unwrap_or_default().to_string_lossy();
                    let entry_name = format!("textures/{filename}");
                    if written.insert(entry_name.clone()) {
                        zip.start_file(&entry_name, options)?;
                        zip.write_all(&data)?;
                    }
                    mat.texture_path = Some(entry_name);
                }
            }
        }
    }

    let envelope = IgzEnvelope {
        igz_format: 2,
        app_version: "0.3.15".to_string(),
        scene: scene_to_igz_document(&modified_scene),
    };
    let json_bytes = serde_json::to_vec_pretty(&envelope)?;
    zip.start_file("document.json", options)?;
    zip.write_all(&json_bytes)?;

    zip.finish()?;
    Ok(())
}

pub fn save_igz(scene: &Scene, path: &Path) -> Result<(), IgzError> {
    let has_texture = scene.materials.values().any(|m| m.texture_path.is_some());
    if has_texture || !scene.embedded_textures.is_empty() {
        save_igz_format2(scene, path)
    } else {
        save_igz_format1(scene, path)
    }
}

pub fn load_igz(path: &Path) -> Result<Scene, IgzError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 4];
    let n = file.read(&mut header)?;

    let mut embedded_textures = std::collections::HashMap::new();
    let json_text = if n >= 4 && &header == ZIP_MAGIC {
        let mut archive = zip::ZipArchive::new(File::open(path)?)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();
            if name != "document.json" && !file.is_dir() {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                embedded_textures.insert(name, bytes);
            }
        }
        let mut doc = archive.by_name("document.json")?;
        let mut s = String::new();
        doc.read_to_string(&mut s)?;
        s
    } else {
        let mut content = Vec::new();
        content.extend_from_slice(&header[..n]);
        file.read_to_end(&mut content)?;
        String::from_utf8_lossy(&content).to_string()
    };

    let envelope: IgzEnvelope = serde_json::from_str(&json_text)?;
    let s_val = &envelope.scene;

    // Check if serialized as Rust Scene struct directly
    if s_val.get("mesh").is_some() && s_val.get("selection").is_some() {
        let mut scene: Scene = serde_json::from_value(s_val.clone())?;
        scene.embedded_textures = embedded_textures;
        scene.mesh.rebuild_spatial_index();
        for g in &mut scene.groups {
            g.mesh.rebuild_spatial_index();
        }
        return Ok(scene);
    }

    // Standard IngeTrazo format
    let mut scene = Scene::new();
    scene.embedded_textures = embedded_textures;
    scene.native_document = s_val.clone();
    scene.mesh = json_to_mesh(s_val);

    // Load component prototypes if present
    let mut proto_meshes = Vec::new();
    if let Some(protos_arr) = s_val.get("protos").and_then(|p| p.as_array()) {
        for p_val in protos_arr {
            proto_meshes.push(json_to_mesh(p_val));
        }
    }

    fn read_group(v: &serde_json::Value, protos: &[Mesh]) -> Result<Group, IgzError> {
        let mut g = Group::new(v.get("name").and_then(|v| v.as_str()).unwrap_or("Group"));
        g.mesh = if let Some(idx) = v.get("proto").and_then(|v| v.as_u64()) {
            protos.get(idx as usize).cloned().ok_or_else(||
                IgzError::InvalidDocument(format!("missing component prototype {idx}")))?
        } else { json_to_mesh(v) };
        if let Some(values) = v.get("xform").or_else(|| v.get("transform")).and_then(|v| v.as_array()) {
            if values.len() != 16 { return Err(IgzError::InvalidDocument("transform must have 16 numbers".into())); }
            let mut cols = [0.; 16];
            for (i, val) in values.iter().enumerate() {
                cols[i] = val.as_f64().ok_or_else(|| IgzError::InvalidDocument("non-numeric transform".into()))?;
            }
            g.transform = DMat4::from_cols_array(&cols);
        }
        g.layer = v.get("layer").and_then(|v| v.as_str()).map(str::to_owned);
        g.ifc_tag = v.get("ifc").filter(|v| !v.is_null()).cloned();
        g.hidden = v.get("hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        g.billboard = v.get("billboard").map(|v| v == true || v == "mesh").unwrap_or(false);
        if let Some(children) = v.get("children").and_then(|v| v.as_array()) {
            g.children = children.iter().map(|v| read_group(v, protos)).collect::<Result<_,_>>()?;
        }
        g.mesh.rebuild_spatial_index();
        g.native_document = v.clone();
        g.native_baseline = group_model(&g);
        Ok(g)
    }
    if let Some(groups_arr) = s_val.get("groups").and_then(|g| g.as_array()) {
        scene.groups = groups_arr.iter().map(|v| read_group(v, &proto_meshes)).collect::<Result<_,_>>()?;
    }

    if let Some(layers_arr) = s_val.get("layers").and_then(|l| l.as_array()) {
        scene.layers.clear();
        for l_val in layers_arr {
            let name = l_val.get("name").and_then(|n| n.as_str()).unwrap_or("Layer0").to_string();
            let visible = l_val.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
            let locked = l_val.get("locked").and_then(|l| l.as_bool()).unwrap_or(false);
            scene.layers.push(Layer {
                name,
                visible,
                locked,
                color: l_val.get("color").and_then(|v| serde_json::from_value(v.clone()).ok()),
            });
        }
    }

    if let Some(cam) = s_val.get("camera") {
        if let Some(tgt) = cam.get("target").and_then(|t| t.as_array()) {
            if tgt.len() >= 3 {
                scene.camera.target = DVec3::new(
                    tgt[0].as_f64().unwrap_or(0.0),
                    tgt[1].as_f64().unwrap_or(0.0),
                    tgt[2].as_f64().unwrap_or(0.0),
                );
            }
        }
        if let Some(dist) = cam.get("distance").and_then(|d| d.as_f64()) {
            scene.camera.distance = dist;
        }
        if let Some(yaw) = cam.get("yaw").and_then(|y| y.as_f64()) {
            scene.camera.yaw = yaw;
        }
        if let Some(pitch) = cam.get("pitch").and_then(|p| p.as_f64()) {
            scene.camera.pitch = pitch;
        }
        if let Some(persp) = cam.get("perspective").and_then(|p| p.as_bool()) {
            scene.camera.perspective = persp;
        }
    }

    if let Some(materials_val) = s_val.get("materials") {
        if let Some(materials_obj) = materials_val.as_object() {
            for (name, mat_val) in materials_obj {
                if let Ok(mat) = serde_json::from_value::<Material>(mat_val.clone()) {
                    scene.materials.insert(name.clone(), mat);
                }
            }
        } else if let Some(materials_arr) = materials_val.as_array() {
            for m_item in materials_arr {
                if let Some(name) = m_item.get("name").and_then(|n| n.as_str()) {
                    let mut col = match name.to_lowercase().as_str() {
                        "metal_dark" => [0.22, 0.24, 0.26, 1.0],
                        "beech" | "wood" => [0.78, 0.58, 0.40, 1.0],
                        "concrete_smooth" | "concrete" => [0.72, 0.72, 0.70, 1.0],
                        _ => [0.80, 0.75, 0.70, 1.0],
                    };
                    if let Some(c_arr) = m_item.get("color").and_then(|c| c.as_array()) {
                        if c_arr.len() >= 3 {
                            col[0] = c_arr[0].as_f64().unwrap_or(col[0] as f64) as f32;
                            col[1] = c_arr[1].as_f64().unwrap_or(col[1] as f64) as f32;
                            col[2] = c_arr[2].as_f64().unwrap_or(col[2] as f64) as f32;
                            if c_arr.len() >= 4 {
                                col[3] = c_arr[3].as_f64().unwrap_or(col[3] as f64) as f32;
                            }
                        }
                    }
                    let mut mat = Material::new(name, col);
                    if let Some(op) = m_item.get("opacity").and_then(|o| o.as_f64()) {
                        mat.opacity = op as f32;
                        mat.color[3] = op as f32;
                    }
                    if let Some(tex) = m_item.get("texture") {
                        if let Some(emb) = tex.get("embed").and_then(|e| e.as_str()) {
                            mat.texture_embed = Some(emb.to_string());
                        }
                        if let Some(p) = tex.get("path").and_then(|p| p.as_str()) {
                            mat.texture_path = Some(p.to_string());
                        }
                    }
                    mat.native_document = m_item.clone();
                    mat.native_baseline = material_model(&mat);
                    scene.materials.insert(name.to_string(), mat);
                }
            }
        }
    }

    if let Some(sv_val) = s_val.get("saved_views").and_then(|v| v.as_array()) {
        for sv_item in sv_val {
            if let Ok(mut sv) = serde_json::from_value::<ingetrazo_core::saved_views::SavedView>(sv_item.clone()) {
                sv.fov_deg = sv_item.get("fov").or_else(|| sv_item.get("fov_deg")).and_then(|v| v.as_f64()).unwrap_or(45.);
                sv.perspective = !sv.parallel;
                scene.saved_views.push(sv);
            }
        }
    }

    if let Some(unit_val) = s_val.get("unit") {
        if let Ok(unit) = serde_json::from_value::<LengthUnit>(unit_val.clone()) {
            scene.unit = unit;
        }
    }

    if let Some(dims_val) = s_val.get("dimensions") {
        if let Ok(dims) = serde_json::from_value::<Vec<Dimension>>(dims_val.clone()) {
            scene.dimensions = dims;
        } else if let Some(arr) = dims_val.as_array() {
            for item in arr {
                if let Ok(dim) = serde_json::from_value::<Dimension>(item.clone()) {
                    scene.dimensions.push(dim);
                }
            }
        }
    }

    if let Some(guides_val) = s_val.get("guides") {
        if let Ok(guides) = serde_json::from_value::<Vec<Guide>>(guides_val.clone()) {
            scene.guides = guides;
        }
    }

    if let Some(tl_val) = s_val.get("text_labels") {
        if let Ok(labels) = serde_json::from_value::<Vec<ingetrazo_core::textlabel::TextLabel>>(tl_val.clone()) {
            scene.text_labels = labels;
        } else if let Some(arr) = tl_val.as_array() {
            for item in arr {
                if let Ok(lab) = serde_json::from_value::<ingetrazo_core::textlabel::TextLabel>(item.clone()) {
                    scene.text_labels.push(lab);
                }
            }
        }
    }

    if let Some(cam) = s_val.get("camera") {
        if let Some(fov) = cam.get("fov_deg").and_then(|v| v.as_f64()) { scene.camera.fov_deg = fov; }
    }
    if let Some(view) = s_val.get("section_view") {
        scene.show_section_planes = !view.get("planes_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        scene.show_section_cuts = !view.get("cuts_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
        scene.show_section_fill = !view.get("fill_hidden").and_then(|v| v.as_bool()).unwrap_or(false);
    }
    macro_rules! load_field {
        ($field:ident, $key:literal) => {
            if let Some(v) = s_val.get($key) {
                if let Ok(value) = serde_json::from_value(v.clone()) { scene.$field = value; }
            }
        }
    }
    if let Some(planes) = s_val.get("section_planes").and_then(|v| v.as_array()) {
        for plane in planes {
            if let (Some(point), Some(normal)) = (plane.get("point"),plane.get("normal")) {
                if let (Ok(point),Ok(normal)) = (serde_json::from_value::<DVec3>(point.clone()),serde_json::from_value::<DVec3>(normal.clone())) {
                    let mut p = ingetrazo_core::section::SectionPlane::new(point,normal,
                        plane.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        plane.get("symbol").and_then(|v| v.as_str()).unwrap_or(""));
                    p.active = plane.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                    if let Some(uid) = plane.get("uid").and_then(|v| v.as_u64()) { p.id = uuid::Uuid::from_u128(uid as u128); }
                    scene.section_planes.push(p);
                }
            }
        }
    }
    if let Some(points) = s_val.get("geo_points").and_then(|v| v.as_array()) {
        for raw in points {
            if let Some(position) = raw.get("position") {
                if let Ok(position) = serde_json::from_value(position.clone()) {
                    let mut point = ingetrazo_core::georef::GeoPoint::new(position,
                        raw.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                        raw.get("desc").and_then(|v| v.as_str()).unwrap_or(""));
                    point.native_document = raw.clone();
                    point.native_baseline = point_model(&point);
                    scene.geo_points.push(point);
                }
            }
        }
    }
    if let Some(paths) = s_val.get("geo_paths").and_then(|v| v.as_array()) {
        for raw in paths {
            if let Some(points) = raw.get("points") {
                if let Ok(points) = serde_json::from_value(points.clone()) {
                    let mut path = ingetrazo_core::georef::GeoPath::new(points,
                        raw.get("closed").and_then(|v| v.as_bool()).unwrap_or(false),
                        raw.get("name").and_then(|v| v.as_str()).unwrap_or(""));
                    path.surface = match raw.get("surface").and_then(|v| v.as_str()) {
                        Some("flat") => Some(ingetrazo_core::georef::SurfaceFillMode::Flat),
                        Some("draped") => Some(ingetrazo_core::georef::SurfaceFillMode::Draped),
                        _ => None
                    };
                    path.native_document = raw.clone();
                    path.native_baseline = path_model(&path);
                    scene.geo_paths.push(path);
                }
            }
        }
    }
    load_field!(terrain, "terrain");
    load_field!(sheets, "compositions");
    load_field!(active_sheet_index, "active_sheet_index");
    if let Some(v) = s_val.get("georef").and_then(|v| v.get("datum")) {
        if let (Some(lat),Some(lon)) = (v.get("lat").and_then(|v| v.as_f64()),v.get("lon").and_then(|v| v.as_f64())) {
            scene.geo_datum = Some(ingetrazo_core::georef::SceneDatum::new(lat,lon,
                v.get("alt").and_then(|v| v.as_f64()).unwrap_or(0.),
                v.get("north").and_then(|v| v.as_f64()).unwrap_or(0.)));
        }
    }
    if let Some(u) = s_val.get("units").and_then(|v| v.get("length")).and_then(|v| v.as_str()) {
        scene.unit = match u {
            "cm" => LengthUnit::Centimeters, "mm" => LengthUnit::Millimeters,
            "in" => LengthUnit::Inches, "ft" => LengthUnit::Feet, _ => LengthUnit::Meters
        };
    }
    scene.native_document = s_val.clone();
    scene.native_baseline = scene_model(&scene);
    scene.mesh.rebuild_spatial_index();
    for g in &mut scene.groups {
        g.mesh.rebuild_spatial_index();
    }

    Ok(scene)
}
