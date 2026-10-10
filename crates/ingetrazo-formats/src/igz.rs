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

pub fn scene_to_igz_document(scene: &Scene) -> serde_json::Value {
    let loose = mesh_to_json(&scene.mesh);
    let mut groups_json = Vec::new();
    for g in &scene.groups {
        let mut g_obj = mesh_to_json(&g.mesh);
        g_obj["name"] = serde_json::Value::String(g.name.clone());
        let m = g.transform.to_cols_array();
        g_obj["transform"] = serde_json::json!(m);
        groups_json.push(g_obj);
    }

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
    doc["materials"] = serde_json::to_value(&scene.materials).unwrap_or(serde_json::json!({}));
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
    doc["unit"] = serde_json::to_value(&scene.unit).unwrap_or(serde_json::json!("Meters"));

    doc
}

pub fn save_igz_format1(scene: &Scene, path: &Path) -> Result<(), IgzError> {
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
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut modified_scene = scene.clone();
    for (_mat_name, mat) in &mut modified_scene.materials {
        if let Some(tex_path) = &mat.texture_path {
            let p = Path::new(tex_path);
            if p.exists() {
                if let Ok(data) = std::fs::read(p) {
                    let filename = p.file_name().unwrap_or_default().to_string_lossy();
                    let entry_name = format!("textures/{filename}");
                    zip.start_file(&entry_name, options)?;
                    zip.write_all(&data)?;
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
    if has_texture {
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
            if name.starts_with("textures/") {
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
    scene.mesh = json_to_mesh(s_val);

    // Load component prototypes if present
    let mut proto_meshes = Vec::new();
    if let Some(protos_arr) = s_val.get("protos").and_then(|p| p.as_array()) {
        for p_val in protos_arr {
            proto_meshes.push(json_to_mesh(p_val));
        }
    }

    if let Some(groups_arr) = s_val.get("groups").and_then(|g| g.as_array()) {
        for g_val in groups_arr {
            let name = g_val.get("name").and_then(|n| n.as_str()).unwrap_or("Group").to_string();
            let mut g = Group::new(name);
            if let Some(proto_idx) = g_val.get("proto").and_then(|p| p.as_u64()) {
                if let Some(p_mesh) = proto_meshes.get(proto_idx as usize) {
                    g.mesh = p_mesh.clone();
                } else {
                    g.mesh = json_to_mesh(g_val);
                }
            } else {
                g.mesh = json_to_mesh(g_val);
            }

            if g_val.get("billboard").and_then(|b| b.as_bool()).unwrap_or(false) {
                g.billboard = true;
            }
            if let Some(faces_arr) = g_val.get("faces").and_then(|f| f.as_array()) {
                for f in faces_arr {
                    if let Some(tex) = f.get("texture") {
                        if let Some(emb) = tex.get("embed").and_then(|e| e.as_str()) {
                            g.billboard_texture = Some(emb.to_string());
                        }
                    }
                }
            }

            let t_val = g_val.get("transform").or_else(|| g_val.get("xform"));
            if let Some(t_arr) = t_val.and_then(|t| t.as_array()) {
                if t_arr.len() == 16 {
                    let mut cols = [0.0; 16];
                    for (i, val) in t_arr.iter().enumerate() {
                        cols[i] = val.as_f64().unwrap_or(0.0);
                    }
                    g.transform = DMat4::from_cols_array(&cols);
                }
            }
            scene.groups.push(g);
        }
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
                color: None,
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
                    scene.materials.insert(name.to_string(), mat);
                }
            }
        }
    }

    if let Some(sv_val) = s_val.get("saved_views").and_then(|v| v.as_array()) {
        for sv_item in sv_val {
            if let Ok(sv) = serde_json::from_value::<ingetrazo_core::saved_views::SavedView>(sv_item.clone()) {
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

    scene.mesh.rebuild_spatial_index();
    for g in &mut scene.groups {
        g.mesh.rebuild_spatial_index();
    }

    Ok(scene)
}
