// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! STL (Stereolithography) format export and import (Binary & ASCII).

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use glam::DVec3;

use ingetrazo_core::scene::Scene;

#[derive(thiserror::Error, Debug)]
pub enum StlError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid STL format: {0}")]
    InvalidFormat(String),
}

fn collect_scene_triangles(scene: &Scene) -> Vec<(DVec3, DVec3, DVec3, DVec3)> {
    crate::exchange::triangles(scene).into_iter().map(|t| {
        let [a,b,c]=t.points;
        ((b-a).cross(c-a).normalize_or_zero(),a,b,c)
    }).collect()
}

/// Export scene to binary STL format.
pub fn export_stl_binary(scene: &Scene, writer: &mut impl Write) -> Result<(), StlError> {
    let mut header = [0u8; 80];
    let title = b"IngeTrazo Rust Native Binary STL";
    header[..title.len()].copy_from_slice(title);
    writer.write_all(&header)?;

    let triangles = collect_scene_triangles(scene);
    let count = triangles.len() as u32;
    writer.write_all(&count.to_le_bytes())?;

    for (norm, p0, p1, p2) in triangles {
        let mut buf = [0u8; 50];
        // Normal
        buf[0..4].copy_from_slice(&(norm.x as f32).to_le_bytes());
        buf[4..8].copy_from_slice(&(norm.y as f32).to_le_bytes());
        buf[8..12].copy_from_slice(&(norm.z as f32).to_le_bytes());
        // V0
        buf[12..16].copy_from_slice(&(p0.x as f32).to_le_bytes());
        buf[16..20].copy_from_slice(&(p0.y as f32).to_le_bytes());
        buf[20..24].copy_from_slice(&(p0.z as f32).to_le_bytes());
        // V1
        buf[24..28].copy_from_slice(&(p1.x as f32).to_le_bytes());
        buf[28..32].copy_from_slice(&(p1.y as f32).to_le_bytes());
        buf[32..36].copy_from_slice(&(p1.z as f32).to_le_bytes());
        // V2
        buf[36..40].copy_from_slice(&(p2.x as f32).to_le_bytes());
        buf[40..44].copy_from_slice(&(p2.y as f32).to_le_bytes());
        buf[44..48].copy_from_slice(&(p2.z as f32).to_le_bytes());
        // Attribute byte count = 0
        buf[48..50].copy_from_slice(&0u16.to_le_bytes());

        writer.write_all(&buf)?;
    }

    Ok(())
}

/// Export scene to ASCII STL format.
pub fn export_stl_ascii(scene: &Scene, writer: &mut impl Write) -> Result<(), StlError> {
    writeln!(writer, "solid IngeTrazo")?;

    for (norm, p0, p1, p2) in collect_scene_triangles(scene) {
        writeln!(writer, "  facet normal {:.6} {:.6} {:.6}", norm.x, norm.y, norm.z)?;
        writeln!(writer, "    outer loop")?;
        writeln!(writer, "      vertex {:.6} {:.6} {:.6}", p0.x, p0.y, p0.z)?;
        writeln!(writer, "      vertex {:.6} {:.6} {:.6}", p1.x, p1.y, p1.z)?;
        writeln!(writer, "      vertex {:.6} {:.6} {:.6}", p2.x, p2.y, p2.z)?;
        writeln!(writer, "    endloop")?;
        writeln!(writer, "  endfacet")?;
    }

    writeln!(writer, "endsolid IngeTrazo")?;
    Ok(())
}

/// Import binary STL stream.
pub fn import_stl_binary(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    let mut header = [0u8; 80];
    reader.read_exact(&mut header)?;

    let mut count_buf = [0u8; 4];
    reader.read_exact(&mut count_buf)?;
    let count = u32::from_le_bytes(count_buf) as usize;

    let mut tri_buf = [0u8; 50];
    for _ in 0..count {
        if reader.read_exact(&mut tri_buf).is_err() {
            break;
        }

        let p0 = DVec3::new(
            f32::from_le_bytes(tri_buf[12..16].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[16..20].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[20..24].try_into().unwrap()) as f64,
        );
        let p1 = DVec3::new(
            f32::from_le_bytes(tri_buf[24..28].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[28..32].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[32..36].try_into().unwrap()) as f64,
        );
        let p2 = DVec3::new(
            f32::from_le_bytes(tri_buf[36..40].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[40..44].try_into().unwrap()) as f64,
            f32::from_le_bytes(tri_buf[44..48].try_into().unwrap()) as f64,
        );

        let i0 = scene.mesh.add_vertex(p0);
        let i1 = scene.mesh.add_vertex(p1);
        let i2 = scene.mesh.add_vertex(p2);
        scene.mesh.add_face(vec![i0, i1, i2], vec![]);
    }

    Ok(())
}

/// Import ASCII STL stream.
pub fn import_stl_ascii(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    let buf_reader = BufReader::new(reader);
    let mut current_verts = Vec::new();

    for line_res in buf_reader.lines() {
        let line = line_res?;
        let trimmed = line.trim();
        if trimmed.starts_with("vertex") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                if let (Ok(x), Ok(y), Ok(z)) = (
                    parts[1].parse::<f64>(),
                    parts[2].parse::<f64>(),
                    parts[3].parse::<f64>(),
                ) {
                    current_verts.push(DVec3::new(x, y, z));
                }
            }
        } else if trimmed.starts_with("endfacet") {
            if current_verts.len() == 3 {
                let i0 = scene.mesh.add_vertex(current_verts[0]);
                let i1 = scene.mesh.add_vertex(current_verts[1]);
                let i2 = scene.mesh.add_vertex(current_verts[2]);
                scene.mesh.add_face(vec![i0, i1, i2], vec![]);
            }
            current_verts.clear();
        }
    }

    Ok(())
}

/// Auto-detect ASCII or Binary STL and import into scene.
pub fn import_stl(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;

    if buf.len() >= 5 && buf.starts_with(b"solid") {
        // Try ASCII
        let mut cursor = std::io::Cursor::new(&buf);
        let mut test_scene = Scene::new();
        if import_stl_ascii(&mut cursor, &mut test_scene).is_ok() && !test_scene.mesh.faces.is_empty() {
            scene.mesh = test_scene.mesh;
            return Ok(());
        }
    }

    let mut cursor = std::io::Cursor::new(&buf);
    import_stl_binary(&mut cursor, scene)
}

pub fn save_stl(scene: &Scene, path: &Path) -> Result<(), StlError> {
    let mut file = File::create(path)?;
    export_stl_binary(scene, &mut file)
}

pub fn load_stl(path: &Path) -> Result<Scene, StlError> {
    let mut file = File::open(path)?;
    let mut scene = Scene::new();
    import_stl(&mut file, &mut scene)?;
    Ok(scene)
}
