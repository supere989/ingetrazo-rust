// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! glTF / GLB binary export.

use ingetrazo_core::scene::Scene;
use std::io::Write;

pub const GLTF_MAGIC: u32 = 0x46546C67; // "glTF"

#[derive(thiserror::Error, Debug)]
pub enum GltfError {
    #[error("Invalid glTF: {0}")]
    Invalid(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

mod figures;
mod rich;
fn build_document(scene: &Scene) -> Result<(serde_json::Value, Vec<u8>), GltfError> {
    rich::build(scene, None)
}

/// Export textual glTF with an embedded buffer, so the document is portable.
pub fn export_gltf(scene: &Scene, writer: &mut impl Write) -> Result<(), GltfError> {
    let (mut document, buffer) = build_document(scene)?;
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
        encoded.push(if bytes.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        encoded.push(if bytes.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    document["buffers"][0]["uri"] =
        serde_json::json!(format!("data:application/octet-stream;base64,{encoded}"));
    serde_json::to_writer_pretty(writer, &document)?;
    Ok(())
}

pub fn export_glb(scene: &Scene, writer: &mut impl Write) -> Result<(), GltfError> {
    write_glb(build_document(scene)?, writer)
}

/// Render export: camera-facing figures travel as independent nodes carrying
/// the source worker's Z-up feet/yaw extras. Default export remains unchanged.
pub fn export_glb_with_camera(
    scene: &Scene,
    camera: &ingetrazo_core::camera::OrbitCamera,
    writer: &mut impl Write,
) -> Result<(), GltfError> {
    let view = figures::View::from_camera(camera)?;
    write_glb(rich::build(scene, Some(&view))?, writer)
}
fn write_glb(
    (gltf_json, bin_data): (serde_json::Value, Vec<u8>),
    writer: &mut impl Write,
) -> Result<(), GltfError> {
    let mut json_bytes = serde_json::to_vec(&gltf_json)?;
    // Pad JSON chunk to 4 bytes with spaces (0x20)
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }

    let total_len = 12usize
        .checked_add(8 + json_bytes.len())
        .and_then(|n| {
            n.checked_add(if bin_data.is_empty() {
                0
            } else {
                8 + bin_data.len()
            })
        })
        .filter(|n| *n <= u32::MAX as usize)
        .ok_or_else(|| GltfError::Invalid("GLB exceeds 32-bit container extent".into()))?;

    // 1. Header (12 bytes)
    writer.write_all(&GLTF_MAGIC.to_le_bytes())?;
    writer.write_all(&2u32.to_le_bytes())?; // version 2
    writer.write_all(&(total_len as u32).to_le_bytes())?;

    // 2. Chunk 0: JSON (8 bytes header + payload)
    writer.write_all(&(json_bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&0x4E4F534Au32.to_le_bytes())?; // "JSON"
    writer.write_all(&json_bytes)?;

    if bin_data.is_empty() {
        return Ok(());
    }
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
        let (doc, bytes) = build_document(&nested_scene()).unwrap();
        assert_eq!(doc["accessors"][0]["count"], 3);
        assert_eq!(
            doc["accessors"][0]["min"],
            serde_json::json!([10.0, 0.0, -6.0])
        );
        assert_eq!(
            doc["accessors"][0]["max"],
            serde_json::json!([11.0, 0.0, -5.0])
        );
        assert_eq!(bytes.len(), 84);
        assert_eq!(doc["meshes"][0]["primitives"][0]["attributes"]["NORMAL"], 1);
    }

    #[test]
    fn textual_and_binary_containers_are_distinct() {
        let scene = nested_scene();
        let mut text = Vec::new();
        export_gltf(&scene, &mut text).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&text).unwrap();
        assert!(doc["buffers"][0]["uri"]
            .as_str()
            .unwrap()
            .starts_with("data:application/octet-stream;base64,"));
        let mut binary = Vec::new();
        export_glb(&scene, &mut binary).unwrap();
        assert_eq!(&binary[..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(binary[8..12].try_into().unwrap()) as usize,
            binary.len()
        );
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
    fn reflected_export_follows_source_transformed_ring_winding() {
        let mut scene = nested_scene();
        scene.groups[0].transform = DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0));
        let (_, bytes) = build_document(&scene).unwrap();
        let floats: Vec<f32> = bytes[..36]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let p: Vec<DVec3> = floats
            .chunks_exact(3)
            .map(|v| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64))
            .collect();
        assert!((p[1] - p[0]).cross(p[2] - p[0]).y < 0.0);
    }
    #[test]
    fn invalid_geometry_refuses_before_writer_publication() {
        let mut scene = nested_scene();
        scene.groups[0].children[0].mesh.vertices[0].position.x = f64::NAN;
        let mut output = b"previous output".to_vec();
        assert!(export_glb(&scene, &mut output).is_err());
        assert_eq!(output, b"previous output");
    }
}
