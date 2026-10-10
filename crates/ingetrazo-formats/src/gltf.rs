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

pub fn export_glb(scene: &Scene, writer: &mut impl Write) -> Result<(), GltfError> {
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

    // Include groups
    for g in &scene.groups {
        if g.hidden {
            continue;
        }
        for f_id in 0..g.mesh.faces.len() {
            for (p0, p1, p2) in g.mesh.triangulate_face(f_id) {
                let tp0 = g.transform.transform_point3(p0);
                let tp1 = g.transform.transform_point3(p1);
                let tp2 = g.transform.transform_point3(p2);
                let base = (positions.len() / 3) as u32;
                positions.extend_from_slice(&[tp0.x as f32, tp0.y as f32, tp0.z as f32]);
                positions.extend_from_slice(&[tp1.x as f32, tp1.y as f32, tp1.z as f32]);
                positions.extend_from_slice(&[tp2.x as f32, tp2.y as f32, tp2.z as f32]);
                indices.extend_from_slice(&[base, base + 1, base + 2]);
            }
        }
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
