// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! STL (Stereolithography) format export and import (Binary & ASCII).

use glam::DVec3;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use ingetrazo_core::scene::Scene;

#[derive(thiserror::Error, Debug)]
pub enum StlError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid STL format: {0}")]
    InvalidFormat(String),
}

fn collect_scene_triangles(scene: &Scene) -> Vec<(DVec3, DVec3, DVec3, DVec3)> {
    crate::exchange::triangles(scene)
        .into_iter()
        .map(|t| {
            let [a, b, c] = t.points;
            ((b - a).cross(c - a).normalize_or_zero(), a, b, c)
        })
        .collect()
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
        writeln!(
            writer,
            "  facet normal {:.6} {:.6} {:.6}",
            norm.x, norm.y, norm.z
        )?;
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

/// Finite native admission; the source has no corresponding memory cap.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TRIANGLES: usize = 1_000_000;
pub const EDITABLE_FACE_LIMIT: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Simplification {
    None,
    Principal,
    All,
}
impl Simplification {
    pub fn parse(value: &str) -> Result<Self, StlError> {
        match value {
            "none" => Ok(Self::None),
            "principal" => Ok(Self::Principal),
            "all" => Ok(Self::All),
            _ => Err(invalid("unknown simplification mode")),
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Principal => "principal",
            Self::All => "all",
        }
    }
}
pub fn unit_factor(unit: &str) -> Option<f64> {
    match unit {
        "m" => Some(1.),
        "cm" => Some(0.01),
        "mm" => Some(0.001),
        "in" => Some(0.0254),
        "ft" => Some(0.3048),
        _ => None,
    }
}
fn invalid(message: impl Into<String>) -> StlError {
    StlError::InvalidFormat(message.into())
}

/// Prepared geometry owns no document or history. Publication is a separate UI operation.
pub struct StlPlan {
    pub mesh: ingetrazo_core::mesh::Mesh,
    pub read_triangles: usize,
}
impl StlPlan {
    pub fn grouped(&self) -> bool {
        self.mesh.faces.len() > EDITABLE_FACE_LIMIT
    }
    pub fn emit(self, scene: &mut Scene, name: &str) -> Result<(), StlError> {
        if self.grouped() {
            scene
                .groups
                .try_reserve(1)
                .map_err(|_| invalid("group allocation refused"))?;
            scene
                .groups
                .push(ingetrazo_core::groups::Group::with_mesh(name, self.mesh));
        } else {
            for face in &self.mesh.faces {
                let rings = std::iter::once(&face.vertices)
                    .chain(face.holes.iter())
                    .map(|ring| {
                        ring.iter()
                            .map(|&v| scene.mesh.add_vertex(self.mesh.vertices[v].position))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                scene
                    .mesh
                    .add_face(rings[0].clone(), rings[1..].to_vec())
                    .ok_or_else(|| invalid("invalid prepared face"))?;
            }
        }
        Ok(())
    }
}
fn read_bounded(reader: &mut impl Read) -> Result<Vec<u8>, StlError> {
    let mut bytes = Vec::new();
    reader.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("encoded STL exceeds native admission"));
    }
    Ok(bytes)
}
fn scaled(values: [f64; 3], scale: f64) -> Result<DVec3, StlError> {
    let values = values.map(|v| v * scale);
    if values
        .iter()
        .any(|v| !v.is_finite() || v.abs() > f32::MAX as f64)
    {
        return Err(invalid("invalid scaled coordinate"));
    }
    Ok(DVec3::from_array(values))
}
fn triangle(mesh: &mut ingetrazo_core::mesh::Mesh, points: [DVec3; 3]) -> Result<(), StlError> {
    let a = points[1] - points[0];
    let b = points[2] - points[0];
    let c = a.cross(b);
    if c.x.hypot(c.y).hypot(c.z) <= 1e-12 {
        return Ok(());
    }
    if mesh.faces.len() >= MAX_TRIANGLES {
        return Err(invalid("triangle inventory exceeds native admission"));
    }
    // Mesh.add_faces_bulk in the original materializes QVector3D after the
    // double-precision scale/degeneracy test.
    let indices = points.map(|p| mesh.add_vertex(p.as_vec3().as_dvec3()));
    mesh.add_face(indices.to_vec(), vec![])
        .ok_or_else(|| invalid("collapsed materialized triangle"))?;
    Ok(())
}
fn parse_binary(bytes: &[u8], scale: f64) -> Result<(ingetrazo_core::mesh::Mesh, usize), StlError> {
    if bytes.len() < 84 {
        return Err(invalid("truncated binary header"));
    }
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    if count > MAX_TRIANGLES {
        return Err(invalid("triangle inventory exceeds native admission"));
    }
    let expected = 84usize
        .checked_add(
            count
                .checked_mul(50)
                .ok_or_else(|| invalid("triangle size overflow"))?,
        )
        .ok_or_else(|| invalid("triangle size overflow"))?;
    if bytes.len() < expected {
        return Err(invalid("truncated binary triangle"));
    }
    let mut mesh = ingetrazo_core::mesh::Mesh::new();
    for record in bytes[84..expected].chunks_exact(50) {
        let mut points = [DVec3::ZERO; 3];
        for (i, p) in points.iter_mut().enumerate() {
            let offset = 12 + i * 12;
            *p = scaled(
                std::array::from_fn(|j| {
                    f32::from_le_bytes(
                        record[offset + j * 4..offset + j * 4 + 4]
                            .try_into()
                            .unwrap(),
                    ) as f64
                }),
                scale,
            )?;
        }
        triangle(&mut mesh, points)?;
    }
    Ok((mesh, count))
}
fn parse_ascii(bytes: &[u8], scale: f64) -> Result<(ingetrazo_core::mesh::Mesh, usize), StlError> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut state = 0;
    let mut points = [DVec3::ZERO; 3];
    let mut vertices = 0;
    let mut count = 0;
    let mut mesh = ingetrazo_core::mesh::Mesh::new();
    for line in text.split(['\r', '\n']) {
        if line.len() > 1024 * 1024 {
            return Err(invalid("ASCII line exceeds native admission"));
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() || parts[0].starts_with('#') {
            continue;
        }
        let key = parts[0].to_ascii_lowercase();
        match state {
            0 => {
                if key == "facet" {
                    if parts.len() != 5 || !parts[1].eq_ignore_ascii_case("normal") {
                        return Err(invalid("invalid facet normal"));
                    }
                    for s in &parts[2..] {
                        if !s
                            .parse::<f64>()
                            .map_err(|_| invalid("invalid normal"))?
                            .is_finite()
                        {
                            return Err(invalid("nonfinite normal"));
                        }
                    }
                    state = 1;
                    vertices = 0;
                }
            }
            1 => {
                if parts.len() != 2 || key != "outer" || !parts[1].eq_ignore_ascii_case("loop") {
                    return Err(invalid("expected outer loop"));
                }
                state = 2;
            }
            2 => {
                if key != "vertex" || parts.len() != 4 {
                    return Err(invalid("expected vertex"));
                }
                let mut values = [0.; 3];
                for i in 0..3 {
                    values[i] = parts[i + 1]
                        .parse()
                        .map_err(|_| invalid("invalid vertex"))?;
                }
                points[vertices] = scaled(values, scale)?;
                vertices += 1;
                if vertices == 3 {
                    state = 3;
                }
            }
            3 => {
                if key != "endloop" || parts.len() != 1 {
                    return Err(invalid("expected endloop"));
                }
                state = 4;
            }
            4 => {
                if key != "endfacet" || parts.len() != 1 {
                    return Err(invalid("expected endfacet"));
                }
                count += 1;
                if count > MAX_TRIANGLES {
                    return Err(invalid("triangle inventory exceeds native admission"));
                }
                triangle(&mut mesh, points)?;
                state = 0;
            }
            _ => unreachable!(),
        }
    }
    if state != 0 {
        return Err(invalid("incomplete facet"));
    }
    Ok((mesh, count))
}
fn finish_mesh(
    mut mesh: ingetrazo_core::mesh::Mesh,
    read_triangles: usize,
    mode: Simplification,
) -> Result<StlPlan, StlError> {
    if mesh.faces.is_empty() {
        return Err(invalid("no usable triangles"));
    }
    if mode != Simplification::None {
        mesh =
            ingetrazo_core::simplify::simplify_mesh(&mesh, 0., mode == Simplification::Principal)
                .map_err(invalid)?
                .0;
    }
    if mesh.faces.len() <= EDITABLE_FACE_LIMIT {
        if mode == Simplification::None {
            // Reuse the native coplanar cleanup and station stitch used by
            // editable exchange imports, scoped to this private import mesh.
            mesh = ingetrazo_core::planar::stitch_boundaries(&mesh).map_err(invalid)?;
            mesh = ingetrazo_core::simplify::simplify_mesh(&mesh, 0., false)
                .map_err(invalid)?
                .0;
            mesh = ingetrazo_core::planar::stitch_boundaries(&mesh).map_err(invalid)?;
        }
        mesh = ingetrazo_core::heal::orient_faces(&mesh).map_err(invalid)?;
    }
    Ok(StlPlan {
        mesh,
        read_triangles,
    })
}
pub fn parse_stl(
    reader: &mut impl Read,
    scale: f64,
    mode: Simplification,
) -> Result<StlPlan, StlError> {
    if !scale.is_finite() || scale <= 0. {
        return Err(invalid("scale must be finite and positive"));
    }
    let bytes = read_bounded(reader)?;
    let binary = if bytes.len() >= 84 {
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as u64;
        let expected = 84 + count * 50;
        bytes.len() as u64 >= expected
            && (bytes.len() as u64 == expected || !bytes[..5].eq_ignore_ascii_case(b"solid"))
    } else {
        false
    };
    let (mesh, count) = if binary {
        parse_binary(&bytes, scale)?
    } else {
        parse_ascii(&bytes, scale)?
    };
    drop(bytes);
    finish_mesh(mesh, count, mode)
}
pub fn parse_stl_path(path: &Path, scale: f64, mode: Simplification) -> Result<StlPlan, StlError> {
    parse_stl(&mut File::open(path)?, scale, mode)
}
pub fn import_stl_binary(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    let bytes = read_bounded(reader)?;
    let (mesh, count) = parse_binary(&bytes, 1.)?;
    finish_mesh(mesh, count, Simplification::Principal)?.emit(scene, "STL")
}
pub fn import_stl_ascii(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    let bytes = read_bounded(reader)?;
    let (mesh, count) = parse_ascii(&bytes, 1.)?;
    finish_mesh(mesh, count, Simplification::Principal)?.emit(scene, "STL")
}
pub fn import_stl(reader: &mut impl Read, scene: &mut Scene) -> Result<(), StlError> {
    parse_stl(reader, 1., Simplification::Principal)?.emit(scene, "STL")
}

pub fn save_stl(scene: &Scene, path: &Path) -> Result<(), StlError> {
    let mut file = File::create(path)?;
    export_stl_binary(scene, &mut file)
}

pub fn load_stl(path: &Path) -> Result<Scene, StlError> {
    let mut file = File::open(path)?;
    let mut scene = Scene::new();
    parse_stl(&mut file, 1., Simplification::Principal)?.emit(
        &mut scene,
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("STL"),
    )?;
    Ok(scene)
}

#[cfg(test)]
mod import_tests {
    use super::*;
    fn binary(solid: bool, tail: bool) -> Vec<u8> {
        let mut out = vec![b' '; 84];
        if solid {
            out[..5].copy_from_slice(b"solid");
        }
        out[80..84].copy_from_slice(&1u32.to_le_bytes());
        let mut triangle = [0u8; 50];
        triangle[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        triangle[24..28].copy_from_slice(&1000f32.to_le_bytes());
        triangle[40..44].copy_from_slice(&1000f32.to_le_bytes());
        out.extend(triangle);
        if tail {
            out.extend(b"metadata");
        }
        out
    }
    #[test]
    fn stl_binary_detection_ignores_normals_and_keeps_trailing_metadata() {
        for (solid, tail) in [(true, false), (false, true)] {
            let plan = parse_stl(
                &mut &binary(solid, tail)[..],
                0.001,
                Simplification::Principal,
            )
            .unwrap();
            assert_eq!(plan.mesh.faces.len(), 1);
            assert!(plan.mesh.vertices.iter().all(|v| v.position.is_finite()));
            assert_eq!(plan.read_triangles, 1);
        }
    }
    #[test]
    fn stl_malformed_import_preserves_existing_mesh() {
        let mut scene = Scene::new();
        scene.mesh.add_vertex(DVec3::X);
        let before = scene.clone();
        let bytes = b"facet normal 0 0 1\nouter loop\nvertex 0 0 0\n";
        assert!(import_stl(&mut &bytes[..], &mut scene).is_err());
        assert_eq!(scene.mesh.vertices.len(), before.mesh.vertices.len());
        assert_eq!(scene.mesh.vertices[0].position, DVec3::X);
        assert!(scene.groups.is_empty());
    }
    #[test]
    fn stl_ascii_rejects_nonfinite_vertex_but_accepts_case_and_bom() {
        let good="\u{feff}SOLID t\r\nFACET normal 0 0 1\r\nOUTER LOOP\r\nVERTEX 0 0 0\r\nvertex 1 0 0\r\nvertex 0 1 0\r\nENDLOOP\r\nENDFACET\r\nENDSOLID t";
        assert_eq!(
            parse_stl(&mut good.as_bytes(), 1., Simplification::Principal)
                .unwrap()
                .mesh
                .faces
                .len(),
            1
        );
        let bad = good.replace("vertex 1 0 0", "vertex NaN 0 0");
        assert!(parse_stl(&mut bad.as_bytes(), 1., Simplification::Principal).is_err());
    }
}
