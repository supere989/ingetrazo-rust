// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! glTF / GLB binary export.

use std::io::Write;
use ingetrazo_core::scene::Scene;

pub const GLTF_MAGIC: u32 = 0x46546C67; // "glTF"

#[derive(thiserror::Error, Debug)]
pub enum GltfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

fn build_document(scene: &Scene) -> (serde_json::Value, Vec<u8>) {
    // Collect triangles: vertex positions as f32
    let mut positions: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for f_id in 0..scene.mesh.faces.len() {
        for (p0, p1, p2) in scene.mesh.triangulate_face(f_id) {
            let base = (positions.len() / 3) as u32;
            positions.extend_from_slice(&[p0.x as f32, p0.y as f32, p0.z as f32]);
            positions.extend_from_slice(&[p1.x as f32, p1.y as f32, p1.z as f32]);
            positions.extend_from_slice(&[p2.x as f32, p2.y as f32, p2.z as f32]);
            indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
    }

    // Bake every nested local transform into the exported world geometry.
    fn collect_group(group: &ingetrazo_core::groups::Group, parent: glam::DMat4,
                     positions: &mut Vec<f32>, indices: &mut Vec<u32>) {
        if group.hidden { return; }
        let world = parent * group.transform;
        let reflected = world.determinant() < 0.0;
        for face in 0..group.mesh.faces.len() {
            for (a, b, c) in group.mesh.triangulate_face(face) {
                let points = if reflected { [a, c, b] } else { [a, b, c] };
                let base = (positions.len() / 3) as u32;
                for point in points {
                    let p = world.transform_point3(point);
                    positions.extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32]);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
        for child in &group.children { collect_group(child, world, positions, indices); }
    }
    for group in &scene.groups {
        collect_group(group, glam::DMat4::IDENTITY, &mut positions, &mut indices);
    }

    if positions.is_empty() {
        return (serde_json::json!({"asset": {"version": "2.0", "generator": "IngeTrazo"}, "scene": 0, "scenes": [{"nodes": []}]}), Vec::new());
    }

    // Binary buffer: positions (f32) + indices (u32)
    let mut bin_data = Vec::new();
    let pos_offset = 0;
    let pos_len = positions.len() * 4;
    for &val in &positions {
        bin_data.extend_from_slice(&val.to_le_bytes());
    }

    // Pad to 4-byte boundary
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }
    let idx_offset = bin_data.len();
    let idx_len = indices.len() * 4;
    for &val in &indices {
        bin_data.extend_from_slice(&val.to_le_bytes());
    }
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let min_max = if !positions.is_empty() {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for chunk in positions.chunks_exact(3) {
            for i in 0..3 {
                min[i] = min[i].min(chunk[i]);
                max[i] = max[i].max(chunk[i]);
            }
        }
        (min, max)
    } else {
        ([0.0; 3], [0.0; 3])
    };

    let gltf_json = serde_json::json!({
        "asset": { "version": "2.0", "generator": "IngeTrazo" },
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{
            "primitives": [{
                "attributes": { "POSITION": 0 },
                "indices": 1,
            }]
        }],
        "buffers": [{ "byteLength": bin_data.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 },
        ],
        "accessors": [
            {
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5126, // FLOAT
                "count": positions.len() / 3,
                "type": "VEC3",
                "max": min_max.1,
                "min": min_max.0,
            },
            {
                "bufferView": 1,
                "byteOffset": 0,
                "componentType": 5125, // UNSIGNED_INT
                "count": indices.len(),
                "type": "SCALAR",
            }
        ]
    });

    (gltf_json, bin_data)
}

/// Export textual glTF with an embedded buffer, so the document is portable.
pub fn export_gltf(scene: &Scene, writer: &mut impl Write) -> Result<(), GltfError> {
    let (mut document, buffer) = build_document(scene);
    if buffer.is_empty() {
        serde_json::to_writer_pretty(writer, &document)?;
        return Ok(());
    }
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(buffer.len().div_ceil(3) * 4);
    for bytes in buffer.chunks(3) {
        let value = ((bytes[0] as u32) << 16)
            | ((bytes.get(1).copied().unwrap_or(0) as u32) << 8)
            | bytes.get(2).copied().unwrap_or(0) as u32;
        encoded.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        encoded.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        encoded.push(if bytes.len() > 1 { ALPHABET[((value >> 6) & 63) as usize] as char } else { '=' });
        encoded.push(if bytes.len() > 2 { ALPHABET[(value & 63) as usize] as char } else { '=' });
    }
    document["buffers"][0]["uri"] = serde_json::json!(format!("data:application/octet-stream;base64,{encoded}"));
    serde_json::to_writer_pretty(writer, &document)?;
    Ok(())
}

pub fn export_glb(scene: &Scene, writer: &mut impl Write) -> Result<(), GltfError> {
    let (gltf_json, bin_data) = build_document(scene);
    let mut json_bytes = serde_json::to_vec(&gltf_json)?;
    // Pad JSON chunk to 4 bytes with spaces (0x20)
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin_data.len();

    // 1. Header (12 bytes)
    writer.write_all(&GLTF_MAGIC.to_le_bytes())?;
    writer.write_all(&2u32.to_le_bytes())?; // version 2
    writer.write_all(&(total_len as u32).to_le_bytes())?;

    // 2. Chunk 0: JSON (8 bytes header + payload)
    writer.write_all(&(json_bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&0x4E4F534Au32.to_le_bytes())?; // "JSON"
    writer.write_all(&json_bytes)?;

    // 3. Chunk 1: BIN (8 bytes header + payload)
    writer.write_all(&(bin_data.len() as u32).to_le_bytes())?;
    writer.write_all(&0x004E4942u32.to_le_bytes())?; // "BIN\0"
    writer.write_all(&bin_data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DMat4, DVec3};
    use ingetrazo_core::groups::Group;

    fn nested_scene() -> Scene {
        let mut scene = Scene::new();
        let mut parent = Group::new("parent");
        parent.transform = DMat4::from_translation(DVec3::new(10.0, 0.0, 0.0));
        let mut child = Group::new("child");
        child.transform = DMat4::from_translation(DVec3::new(0.0, 5.0, 0.0));
        let a = child.mesh.add_vertex(DVec3::ZERO);
        let b = child.mesh.add_vertex(DVec3::X);
        let c = child.mesh.add_vertex(DVec3::Y);
        child.mesh.add_face(vec![a, b, c], vec![]);
        parent.children.push(child);
        scene.groups.push(parent);
        scene
    }

    #[test]
    fn nested_world_geometry_is_exported() {
        let (doc, bytes) = build_document(&nested_scene());
        assert_eq!(doc["accessors"][0]["count"], 3);
        assert_eq!(doc["accessors"][0]["min"], serde_json::json!([10.0,5.0,0.0]));
        assert_eq!(doc["accessors"][0]["max"], serde_json::json!([11.0,6.0,0.0]));
        assert_eq!(bytes.len(), 48);
    }

    #[test]
    fn textual_and_binary_containers_are_distinct() {
        let scene = nested_scene();
        let mut text = Vec::new();
        export_gltf(&scene, &mut text).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&text).unwrap();
        assert!(doc["buffers"][0]["uri"].as_str().unwrap().starts_with("data:application/octet-stream;base64,"));
        let mut binary = Vec::new();
        export_glb(&scene, &mut binary).unwrap();
        assert_eq!(&binary[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(binary[8..12].try_into().unwrap()) as usize, binary.len());
    }

    #[test]
    fn hidden_parent_suppresses_descendants_and_empty_has_no_accessors() {
        let mut scene = nested_scene();
        scene.groups[0].hidden = true;
        let mut text = Vec::new();
        export_gltf(&scene, &mut text).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&text).unwrap();
        assert!(doc.get("accessors").is_none());
        assert!(doc.get("buffers").is_none());
    }

    #[test]
    fn reflected_instance_retains_outward_winding() {
        let mut scene = nested_scene();
        scene.groups[0].transform = DMat4::from_scale(DVec3::new(-1.0,1.0,1.0));
        let (_, bytes) = build_document(&scene);
        let floats: Vec<f32> = bytes[..36].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        let p: Vec<DVec3> = floats.chunks_exact(3).map(|v| DVec3::new(v[0] as f64,v[1] as f64,v[2] as f64)).collect();
        assert!((p[1]-p[0]).cross(p[2]-p[0]).z > 0.0);
    }
}
