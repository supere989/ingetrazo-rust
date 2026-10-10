// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Wavefront OBJ format export and import.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use glam::DVec3;

use ingetrazo_core::scene::Scene;

#[derive(thiserror::Error, Debug)]
pub enum ObjError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error at line {0}: {1}")]
    Parse(usize, String),
}

/// Export scene (loose mesh + groups) into an OBJ writer.
pub fn export_obj(scene: &Scene, writer: &mut impl Write) -> Result<(), ObjError> {
    writeln!(writer, "# IngeTrazo OBJ Export")?;

    let mut all_verts: Vec<DVec3> = Vec::new();
    let mut all_normals: Vec<DVec3> = Vec::new();
    let mut all_faces: Vec<(Vec<usize>, usize)> = Vec::new(); // (vert_indices, normal_idx)

    // 1. Loose mesh
    let base_idx = all_verts.len();
    for v in &scene.mesh.vertices {
        all_verts.push(v.position);
    }
    for (f_id, f) in scene.mesh.faces.iter().enumerate() {
        let norm = scene.mesh.face_normal(f_id);
        all_normals.push(norm);
        let n_idx = all_normals.len();
        let loop_idx: Vec<usize> = f.vertices.iter().map(|&v| base_idx + v + 1).collect();
        all_faces.push((loop_idx, n_idx));
    }

    // 2. Groups
    for group in &scene.groups {
        let base_g = all_verts.len();
        for v in &group.mesh.vertices {
            let world_p = group.transform.transform_point3(v.position);
            all_verts.push(world_p);
        }
        for (f_id, f) in group.mesh.faces.iter().enumerate() {
            let local_norm = group.mesh.face_normal(f_id);
            let world_norm = group.transform.transform_vector3(local_norm).normalize_or_zero();
            all_normals.push(world_norm);
            let n_idx = all_normals.len();
            let loop_idx: Vec<usize> = f.vertices.iter().map(|&v| base_g + v + 1).collect();
            all_faces.push((loop_idx, n_idx));
        }
    }



    // Write vertex positions
    for v in &all_verts {
        writeln!(writer, "v {:.6} {:.6} {:.6}", v.x, v.y, v.z)?;
    }

    // Write normals
    for n in &all_normals {
        writeln!(writer, "vn {:.6} {:.6} {:.6}", n.x, n.y, n.z)?;
    }

    // Write polygon faces
    for (v_indices, n_idx) in all_faces {
        write!(writer, "f")?;
        for v_idx in v_indices {
            write!(writer, " {}//{}", v_idx, n_idx)?;
        }
        writeln!(writer)?;
    }

    Ok(())
}

/// Import an OBJ stream into a scene's mesh.
pub fn import_obj(reader: &mut impl Read, scene: &mut Scene) -> Result<(), ObjError> {
    let buf_reader = BufReader::new(reader);
    let mut file_verts: Vec<DVec3> = Vec::new();

    for (line_num, line_res) in buf_reader.lines().enumerate() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => continue,
        };

        match cmd {
            "v" => {
                let x: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex X".to_string())
                })?;
                let y: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex Y".to_string())
                })?;
                let z: f64 = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    ObjError::Parse(line_num + 1, "Invalid vertex Z".to_string())
                })?;
                file_verts.push(DVec3::new(x, y, z));
            }
            "f" => {
                let mut face_pts = Vec::new();
                for part in parts {
                    let v_idx_str = part.split('/').next().unwrap_or("");
                    if let Ok(idx) = v_idx_str.parse::<isize>() {
                        let actual_idx = if idx > 0 {
                            (idx - 1) as usize
                        } else if idx < 0 {
                            (file_verts.len() as isize + idx) as usize
                        } else {
                            continue;
                        };
                        if actual_idx < file_verts.len() {
                            face_pts.push(file_verts[actual_idx]);
                        }
                    }
                }

                if face_pts.len() >= 3 {
                    let mut loop_verts = Vec::new();
                    for pt in face_pts {
                        let v = scene.mesh.add_vertex(pt);
                        loop_verts.push(v);
                    }
                    scene.mesh.add_face(loop_verts, vec![]);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn save_obj(scene: &Scene, path: &Path) -> Result<(), ObjError> {
    let mut file = File::create(path)?;
    export_obj(scene, &mut file)
}

pub fn load_obj(path: &Path) -> Result<Scene, ObjError> {
    let mut file = File::open(path)?;
    let mut scene = Scene::new();
    import_obj(&mut file, &mut scene)?;
    Ok(scene)
}
