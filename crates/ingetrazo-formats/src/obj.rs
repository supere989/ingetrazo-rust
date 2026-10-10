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

    let triangles=crate::exchange::triangles(scene);
    for triangle in &triangles {
        for p in triangle.points {writeln!(writer,"v {:.6} {:.6} {:.6}",p.x,p.y,p.z)?;}
    }
    for triangle in &triangles {
        let [a,b,c]=triangle.points;
        let n=(b-a).cross(c-a).normalize_or_zero();
        writeln!(writer,"vn {:.6} {:.6} {:.6}",n.x,n.y,n.z)?;
    }
    for (i,_) in triangles.iter().enumerate() {
        let v=3*i+1;let n=i+1;
        writeln!(writer,"f {v}//{n} {}//{n} {}//{n}",v+1,v+2)?;
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
