// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::io::Cursor;
use glam::{DMat4, DVec3};

use ingetrazo_core::camera::OrbitCamera;
use ingetrazo_core::dimensions::Dimension;
use ingetrazo_core::groups::Group;
use ingetrazo_core::guides::Guide;
use ingetrazo_core::layers::Layer;
use ingetrazo_core::materials::Material;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_core::scene::Scene;
use ingetrazo_core::units::LengthUnit;

use ingetrazo_formats::dxf::{export_dxf, import_dxf};
use ingetrazo_formats::gltf::{export_glb, GLTF_MAGIC};
use ingetrazo_formats::igz::{load_igz, save_igz, ZIP_MAGIC};
use ingetrazo_formats::obj::{export_obj, import_obj};
use ingetrazo_formats::stl::{export_stl_ascii, export_stl_binary, import_stl, import_stl_ascii, import_stl_binary};

/// Build a 1x1x1 unit cube scene with 6 quad faces.
fn create_test_cube_scene() -> Scene {
    let mut scene = Scene::new();
    scene.unit = LengthUnit::Meters;

    // 8 cube vertices
    let v000 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
    let v100 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
    let v110 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
    let v010 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
    let v001 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 1.0));
    let v101 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 1.0));
    let v111 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 1.0));
    let v011 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 1.0));

    // 6 quad faces
    scene.mesh.add_face(vec![v000, v010, v110, v100], vec![]); // bottom (-Z)
    scene.mesh.add_face(vec![v001, v101, v111, v011], vec![]); // top (+Z)
    scene.mesh.add_face(vec![v000, v100, v101, v001], vec![]); // front (-Y)
    scene.mesh.add_face(vec![v100, v110, v111, v101], vec![]); // right (+X)
    scene.mesh.add_face(vec![v110, v010, v011, v111], vec![]); // back (+Y)
    scene.mesh.add_face(vec![v010, v000, v001, v011], vec![]); // left (-X)

    // Add layer
    scene.layers.push(Layer::new("Walls"));

    // Add material
    scene.materials.insert(
        "Brick".to_string(),
        Material::new("Brick", [0.8, 0.2, 0.2, 1.0]),
    );

    // Add camera
    let mut cam = OrbitCamera::new();
    cam.target = DVec3::new(0.5, 0.5, 0.5);
    cam.distance = 5.0;
    scene.camera = cam;

    // Add dimension
    scene.dimensions.push(Dimension::new(
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(0.0, -0.5, 0.0),
    ));

    // Add guide
    scene.guides.push(Guide::point(DVec3::new(0.5, 0.5, 1.0)));

    // Add a group with a pyramid
    let mut group_mesh = Mesh::new();
    let b0 = group_mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
    let b1 = group_mesh.add_vertex(DVec3::new(3.0, 0.0, 0.0));
    let b2 = group_mesh.add_vertex(DVec3::new(3.0, 1.0, 0.0));
    let b3 = group_mesh.add_vertex(DVec3::new(2.0, 1.0, 0.0));
    let apex = group_mesh.add_vertex(DVec3::new(2.5, 0.5, 1.0));
    group_mesh.add_face(vec![b0, b3, b2, b1], vec![]);
    group_mesh.add_face(vec![b0, b1, apex], vec![]);
    group_mesh.add_face(vec![b1, b2, apex], vec![]);
    group_mesh.add_face(vec![b2, b3, apex], vec![]);
    group_mesh.add_face(vec![b3, b0, apex], vec![]);

    let mut group = Group::with_mesh("PyramidGroup", group_mesh);
    group.transform = DMat4::from_translation(DVec3::new(1.0, 2.0, 0.0));
    scene.groups.push(group);

    scene.bump_version();
    scene
}

#[test]
fn test_igz_format1_roundtrip() {
    let original = create_test_cube_scene();
    let dir = std::env::temp_dir();
    let test_path = dir.join("test_roundtrip_format1.igz");

    // Save as IGZ (Format 1 - untextured)
    save_igz(&original, &test_path).expect("Failed to save IGZ Format 1");

    // Verify it is plain text JSON (not ZIP)
    let bytes = std::fs::read(&test_path).expect("Failed to read saved file");
    assert!(!bytes.starts_with(ZIP_MAGIC), "Expected Format 1 plain JSON, found ZIP");

    // Load back
    let loaded = load_igz(&test_path).expect("Failed to load IGZ");

    // Assert mesh equality
    assert_eq!(loaded.mesh.vertices.len(), original.mesh.vertices.len());
    assert_eq!(loaded.mesh.faces.len(), original.mesh.faces.len());
    assert_eq!(loaded.mesh.edges.len(), original.mesh.edges.len());

    // Assert cube volume
    let orig_vol = original.mesh.volume().expect("Original cube must have volume");
    let load_vol = loaded.mesh.volume().expect("Loaded cube must have volume");
    assert!((orig_vol - load_vol).abs() < 1e-4);

    // Assert metadata
    assert_eq!(loaded.unit, original.unit);
    assert_eq!(loaded.layers.len(), original.layers.len());
    assert!(loaded.materials.contains_key("Brick"));
    assert_eq!(loaded.dimensions.len(), 1);
    assert_eq!(loaded.guides.len(), 1);
    assert_eq!(loaded.groups.len(), 1);
    assert_eq!(loaded.groups[0].mesh.faces.len(), original.groups[0].mesh.faces.len());

    let _ = std::fs::remove_file(test_path);
}

#[test]
fn test_igz_format2_zip_roundtrip() {
    let mut original = create_test_cube_scene();
    let dir = std::env::temp_dir();

    // Create a dummy texture file
    let tex_path = dir.join("brick_texture.png");
    std::fs::write(&tex_path, b"FAKE_PNG_IMAGE_DATA_12345").expect("Failed to write dummy texture");

    // Attach texture to material
    let mut mat = Material::with_texture("TexturedBrick", tex_path.to_str().unwrap());
    mat.texture_path = Some(tex_path.to_str().unwrap().to_string());
    original.materials.insert("TexturedBrick".to_string(), mat);

    let test_path = dir.join("test_roundtrip_format2.igz");

    // Save as IGZ (Format 2 - textured, ZIP container)
    save_igz(&original, &test_path).expect("Failed to save IGZ Format 2");

    // Verify file starts with PK ZIP magic
    let bytes = std::fs::read(&test_path).expect("Failed to read saved file");
    assert!(bytes.starts_with(ZIP_MAGIC), "Expected Format 2 ZIP container magic");

    // Load back
    let loaded = load_igz(&test_path).expect("Failed to load IGZ Format 2");

    // Verify geometry and volume
    assert_eq!(loaded.mesh.vertices.len(), original.mesh.vertices.len());
    assert_eq!(loaded.mesh.faces.len(), original.mesh.faces.len());
    let orig_vol = original.mesh.volume().unwrap();
    let load_vol = loaded.mesh.volume().unwrap();
    assert!((orig_vol - load_vol).abs() < 1e-4);

    // Verify materials
    assert!(loaded.materials.contains_key("TexturedBrick"));

    // Cleanup
    let _ = std::fs::remove_file(tex_path);
    let _ = std::fs::remove_file(test_path);
}

#[test]
fn test_obj_export_and_import_roundtrip() {
    let original = create_test_cube_scene();

    // Export to OBJ in-memory buffer
    let mut buffer = Vec::new();
    export_obj(&original, &mut buffer).expect("Failed to export OBJ");

    let obj_str = String::from_utf8(buffer.clone()).expect("Invalid UTF-8 in OBJ export");

    // Check OBJ content: vertices, normals, and polygon faces
    assert!(obj_str.contains("v 0.000000 0.000000 0.000000"));
    assert!(obj_str.contains("vn "));
    assert!(obj_str.contains("f "));

    // Re-import into new Scene
    let mut new_scene = Scene::new();
    let mut cursor = Cursor::new(buffer);
    import_obj(&mut cursor, &mut new_scene).expect("Failed to import OBJ");

    // In OBJ export, original scene had 6 faces on cube + 5 faces on group pyramid = 11 faces
    assert!(new_scene.mesh.faces.len() >= 6);

    // Match the original writer's triangulated interchange while retaining
    // actual closed geometry, including the transformed pyramid.
    assert_eq!(new_scene.mesh.faces.len(),18);
    assert!(new_scene.mesh.faces.iter().all(|f|f.vertices.len()==3));
    assert!((new_scene.mesh.volume().unwrap()-4.0/3.0).abs()<1e-6);

    // Verify cube volume
    let cube_vol = original.mesh.volume().unwrap();
    assert!((cube_vol - 1.0).abs() < 1e-4);
}

#[test]
fn test_stl_binary_roundtrip() {
    let original = create_test_cube_scene();

    // Export only the loose cube mesh to binary STL
    let mut cube_scene = Scene::new();
    cube_scene.mesh = original.mesh.clone();

    let mut buffer = Vec::new();
    export_stl_binary(&cube_scene, &mut buffer).expect("Failed to export binary STL");

    // 80 header + 4 count + 12 triangles * 50 = 684 bytes
    assert_eq!(buffer.len(), 80 + 4 + 12 * 50);

    // Import into new Scene
    let mut new_scene = Scene::new();
    let mut cursor = Cursor::new(buffer);
    import_stl_binary(&mut cursor, &mut new_scene).expect("Failed to import binary STL");

    assert_eq!(new_scene.mesh.faces.len(), 12, "A triangulated cube has 12 triangle facets");
    assert!(new_scene.mesh.is_manifold_solid(), "Cube should be watertight manifold solid");
    let vol = new_scene.mesh.volume().expect("Imported cube should have volume");
    assert!((vol - 1.0).abs() < 1e-4);
}

#[test]
fn test_stl_ascii_roundtrip() {
    let original = create_test_cube_scene();
    let mut cube_scene = Scene::new();
    cube_scene.mesh = original.mesh.clone();

    let mut buffer = Vec::new();
    export_stl_ascii(&cube_scene, &mut buffer).expect("Failed to export ASCII STL");

    let ascii_str = String::from_utf8(buffer.clone()).expect("Invalid UTF-8 in ASCII STL");
    assert!(ascii_str.starts_with("solid IngeTrazo"));
    assert!(ascii_str.contains("facet normal"));
    assert!(ascii_str.contains("outer loop"));
    assert!(ascii_str.contains("vertex"));
    assert!(ascii_str.ends_with("endsolid IngeTrazo\n"));

    let mut new_scene = Scene::new();
    let mut cursor = Cursor::new(buffer);
    import_stl_ascii(&mut cursor, &mut new_scene).expect("Failed to import ASCII STL");

    assert_eq!(new_scene.mesh.faces.len(), 12);
    assert!(new_scene.mesh.is_manifold_solid());
    let vol = new_scene.mesh.volume().expect("Imported cube should have volume");
    assert!((vol - 1.0).abs() < 1e-4);
}

#[test]
fn test_stl_auto_detect() {
    let original = create_test_cube_scene();
    let mut cube_scene = Scene::new();
    cube_scene.mesh = original.mesh.clone();

    // 1. Test binary auto-detect
    let mut bin_buf = Vec::new();
    export_stl_binary(&cube_scene, &mut bin_buf).unwrap();
    let mut bin_scene = Scene::new();
    let mut cursor = Cursor::new(bin_buf);
    import_stl(&mut cursor, &mut bin_scene).expect("Auto-detect failed on binary STL");
    assert_eq!(bin_scene.mesh.faces.len(), 12);

    // 2. Test ASCII auto-detect
    let mut asc_buf = Vec::new();
    export_stl_ascii(&cube_scene, &mut asc_buf).unwrap();
    let mut asc_scene = Scene::new();
    let mut cursor = Cursor::new(asc_buf);
    import_stl(&mut cursor, &mut asc_scene).expect("Auto-detect failed on ASCII STL");
    assert_eq!(asc_scene.mesh.faces.len(), 12);
}

#[test]
fn test_glb_export() {
    let original = create_test_cube_scene();
    let mut buffer = Vec::new();

    export_glb(&original, &mut buffer).expect("Failed to export GLB");

    // Verify GLB magic header
    assert!(buffer.len() >= 12);
    let magic = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
    let version = u32::from_le_bytes(buffer[4..8].try_into().unwrap());
    let length = u32::from_le_bytes(buffer[8..12].try_into().unwrap());

    assert_eq!(magic, GLTF_MAGIC);
    assert_eq!(version, 2);
    assert_eq!(length as usize, buffer.len());

    // Chunk 0: JSON
    let json_len = u32::from_le_bytes(buffer[12..16].try_into().unwrap()) as usize;
    let json_type = u32::from_le_bytes(buffer[16..20].try_into().unwrap());
    assert_eq!(json_type, 0x4E4F534A); // "JSON"

    let json_str = std::str::from_utf8(&buffer[20..20 + json_len]).expect("Invalid UTF-8 in glTF JSON");
    let parsed: serde_json::Value = serde_json::from_str(json_str).expect("Invalid glTF JSON payload");

    assert_eq!(parsed["asset"]["version"], "2.0");
    assert!(parsed["meshes"].is_array());
    assert!(parsed["accessors"].is_array());
    assert!(parsed["bufferViews"].is_array());
}

#[test]
fn test_dxf_export_and_import() {
    let original = create_test_cube_scene();
    let mut buffer = Vec::new();

    export_dxf(&original, &mut buffer).expect("Failed to export DXF");
    let dxf_str = String::from_utf8(buffer.clone()).expect("Invalid UTF-8 in DXF");

    assert!(dxf_str.contains("SECTION\n2\nHEADER"));
    assert!(dxf_str.contains("SECTION\n2\nTABLES"));
    assert!(dxf_str.contains("SECTION\n2\nENTITIES"));
    assert!(dxf_str.contains("3DFACE"));
    assert!(dxf_str.contains("EOF"));

    let mut new_scene = Scene::new();
    let mut cursor = Cursor::new(buffer);
    import_dxf(&mut cursor, &mut new_scene).expect("Failed to import DXF");

    assert!(new_scene.mesh.faces.len() >= 6, "DXF import should reconstruct faces");
}

#[test]
fn test_load_example_banca_pergola() {
    use std::path::Path;
    let path = Path::new("../../ingetrazo/examples/banca-pergola-yanque.igz");
    if !path.exists() {
        return;
    }
    let scene = load_igz(path).expect("Failed to load example banca pergola");
    println!("Loaded scene unit: {:?}", scene.unit);
    println!("Loaded materials: {:?}", scene.materials.keys().collect::<Vec<_>>());
    for (k, m) in &scene.materials {
        println!("  mat {}: color={:?}, tex={:?}", k, m.color, m.texture_path);
    }
    println!("Loaded embedded textures: {:?}", scene.embedded_textures.keys().collect::<Vec<_>>());
    println!("Loaded groups: {}", scene.groups.len());
    for (i, g) in scene.groups.iter().enumerate() {
        println!("  group {}: name={}, billboard={}, faces={}, edges={}", i, g.name, g.billboard, g.mesh.faces.len(), g.mesh.edges.len());
        for (f_idx, f) in g.mesh.faces.iter().take(3).enumerate() {
            println!("    face {}: attrs={:?}", f_idx, f.attrs);
        }
    }
}
