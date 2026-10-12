// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! IngeTrazo file format support:
//! - IGZ (.igz): Native document format
//! - OBJ (.obj): Wavefront OBJ exchange
//! - STL (.stl): Stereolithography 3D print format

mod exchange;
pub mod skp;
pub use skp::{load_skp,SkpError};
pub mod dae;
mod dae_import;
pub mod dxf;
pub mod sheet_dxf;
pub mod gltf;
pub mod gltf_import;
pub mod ifc;
pub mod igz;
pub mod geoimport;
pub mod webp;
pub mod obj;
pub mod stl;

pub use dae::export_dae;
pub use dae_import::{load_dae,DaeError};
pub use dxf::{export_dxf, import_dxf, DxfError};
pub use gltf_import::load_gltf;
pub use gltf::{export_glb, export_gltf, GltfError, GLTF_MAGIC};
pub use ifc::export_ifc;
pub use igz::{load_igz, save_igz, save_igz_format1, save_igz_format2, scene_to_igz_document, IgzError, ZIP_MAGIC};
pub use obj::{export_obj, import_obj, load_obj, save_obj, ObjError};
pub use stl::{
    export_stl_ascii, export_stl_binary, import_stl, import_stl_ascii, import_stl_binary,
    load_stl, save_stl, StlError,
};


use std::path::Path;
use ingetrazo_core::scene::Scene;

#[derive(thiserror::Error, Debug)]
pub enum FormatError {
    #[error("SKP format error: {0}")]
    Skp(#[from] SkpError),
    #[error("COLLADA format error: {0}")]
    Dae(#[from] DaeError),
    #[error("IGZ format error: {0}")]
    Igz(#[from] IgzError),
    #[error("OBJ format error: {0}")]
    Obj(#[from] ObjError),
    #[error("STL format error: {0}")]
    Stl(#[from] StlError),
    #[error("DXF format error: {0}")]
    Dxf(#[from] DxfError),
    #[error("glTF format error: {0}")]
    Gltf(#[from] GltfError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unsupported format: {0}")]
    Unsupported(String),
}

/// Headless loader automatically determining format by file extension.
pub fn load_file(path: &Path) -> Result<Scene, FormatError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "igz" => Ok(load_igz(path)?),
        "skp" => Ok(load_skp(path)?),
        "dae" => Ok(load_dae(path)?),
        "gltf" | "glb" => Ok(load_gltf(path)?),
        "obj" => Ok(load_obj(path)?),
        "stl" => Ok(load_stl(path)?),
        "dxf" => {
            let mut file = std::fs::File::open(path)?;
            let mut scene = Scene::new();
            import_dxf(&mut file, &mut scene)?;
            Ok(scene)
        }
        other => Err(FormatError::Unsupported(format!("*.{other}"))),
    }
}

/// Headless saver automatically determining format by file extension.
pub fn save_file(scene: &Scene, path: &Path) -> Result<(), FormatError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "igz" => Ok(save_igz(scene, path)?),
        "obj" => Ok(save_obj(scene, path)?),
        "stl" => Ok(save_stl(scene, path)?),
        "dxf" => {
            let mut file = std::fs::File::create(path)?;
            export_dxf(scene, &mut file)?;
            Ok(())
        }
        "gltf" => {
            let mut file = std::fs::File::create(path)?;
            export_gltf(scene, &mut file)?;
            Ok(())
        }
        "glb" => {
            let mut file = std::fs::File::create(path)?;
            export_glb(scene, &mut file)?;
            Ok(())
        }
        "dae" => {
            let mut file = std::fs::File::create(path)?;
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("IngeTrazo Model");
            export_dae(scene, &mut file, name)?;
            Ok(())
        }
        "ifc" => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("IngeTrazo Project");
            ifc::save_ifc(scene, path, name)?;
            Ok(())
        }
        other => Err(FormatError::Unsupported(format!("*.{other}"))),
    }
}

/// Headless converter from input file to output file.
pub fn convert_file(input: &Path, output: &Path) -> Result<(), FormatError> {
    let scene = load_file(input)?;
    save_file(&scene, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    #[test]
    fn test_igz_save_and_load() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_model.igz");

        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        save_file(&scene, &path).expect("Failed to save IGZ");
        let loaded = load_file(&path).expect("Failed to load IGZ");

        assert_eq!(loaded.mesh.vertices.len(), 3);
        assert_eq!(loaded.mesh.faces.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_obj_export_and_import() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_model.obj");

        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(2.0, 3.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        save_file(&scene, &path).expect("Failed to save OBJ");
        let loaded = load_file(&path).expect("Failed to load OBJ");

        assert_eq!(loaded.mesh.vertices.len(), 3);
        assert_eq!(loaded.mesh.faces.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_stl_export_and_import() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_model.stl");

        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        save_file(&scene, &path).expect("Failed to save STL");
        let loaded = load_file(&path).expect("Failed to load STL");

        assert_eq!(loaded.mesh.vertices.len(), 3);
        assert_eq!(loaded.mesh.faces.len(), 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_convert_file() {
        let temp_dir = std::env::temp_dir();
        let in_path = temp_dir.join("convert_src.obj");
        let out_path = temp_dir.join("convert_dest.stl");

        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        save_file(&scene, &in_path).unwrap();
        convert_file(&in_path, &out_path).unwrap();

        let loaded = load_file(&out_path).unwrap();
        assert_eq!(loaded.mesh.faces.len(), 1);

        let _ = std::fs::remove_file(in_path);
        let _ = std::fs::remove_file(out_path);
    }
}

pub mod sheet_pdf;

pub mod sheet_pdf_shapes;

pub mod paper_text;

mod paper_font_subset;
pub mod paper_font;
pub mod paper_font_resolver;

pub mod paper_text_layout;

pub mod paper_text_painter;

pub mod annotation_font;

#[cfg(target_os="macos")]
mod paper_coretext_glyph;

#[cfg(any(target_os="macos",test))]
mod paper_font_sfnt;

pub mod text3d;

pub mod photo_bounds;

pub mod sheet_image;

mod sheet_jpeg;


pub mod photo_profile;

pub mod dem_profile;

pub mod dwg;
