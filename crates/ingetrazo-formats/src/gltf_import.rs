//! glTF reference import in world Z-up metres with self-contained textures.
use crate::gltf::GltfError;
use base64::Engine;
use glam::{DMat3, DMat4, DVec2, DVec3};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use std::{collections::HashSet, path::Path};
fn invalid(message: impl Into<String>) -> GltfError {
    GltfError::Invalid(message.into())
}

pub fn load_gltf(path: &Path) -> Result<Scene, GltfError> {
    load_gltf_policy(path, None)
}
pub fn parse_gltf_import(path: &Path) -> Result<crate::FileImportPlan, GltfError> {
    let source = app_policy::read_source_json(path)?;
    let scene = load_gltf_policy(path, Some(&source))?;
    Ok(crate::FileImportPlan::from_file_scene(scene))
}
fn load_gltf_policy(path: &Path, source: Option<&app_policy::Input>) -> Result<Scene, GltfError> {
    let mut parsed = if let Some(source) = source {
        ::gltf::Gltf::from_slice(&source.bytes)
    } else {
        ::gltf::Gltf::open(path)
    }
    .map_err(|e| invalid(e.to_string()))?;
    let source = source.map(|i| &i.json);
    for extension in parsed.document.extensions_required() {
        if ![
            "KHR_texture_transform",
            "KHR_materials_pbrSpecularGlossiness",
            "KHR_materials_unlit",
        ]
        .contains(&extension)
        {
            return Err(invalid(format!("Required glTF extension {extension} is unsupported; use an uncompressed static export")));
        }
    }
    let buffers = ::gltf::import_buffers(&parsed.document, path.parent(), parsed.blob.take())
        .map_err(|e| invalid(e.to_string()))?;
    // Check source ranges before the library's iterator slices them.
    for view in parsed.document.views() {
        let data = buffers
            .get(view.buffer().index())
            .ok_or_else(|| invalid("Missing glTF buffer"))?;
        if view
            .offset()
            .checked_add(view.length())
            .map_or(true, |end| end > data.0.len())
        {
            return Err(invalid("glTF buffer view exceeds available bytes"));
        }
    }
    for accessor in parsed.document.accessors() {
        if let Some(view) = accessor.view() {
            let stride = view.stride().unwrap_or(accessor.size());
            let bytes = accessor
                .count()
                .saturating_sub(1)
                .checked_mul(stride)
                .and_then(|n| {
                    n.checked_add(if accessor.count() == 0 {
                        0
                    } else {
                        accessor.size()
                    })
                })
                .and_then(|n| n.checked_add(accessor.offset()));
            if bytes.map_or(true, |n| n > view.length()) {
                return Err(invalid("glTF accessor exceeds its buffer view"));
            }
        }
        if accessor.sparse().is_some() {
            return Err(invalid("Sparse glTF accessors are unsupported"));
        }
    }
    let mut scene = Scene::new();
    let mut images = Vec::new();
    for image in parsed.document.images() {
        let (bytes, extension) = match image.source() {
            ::gltf::image::Source::View { view, mime_type } => {
                let data = &buffers[view.buffer().index()].0;
                (
                    data[view.offset()..view.offset() + view.length()].to_vec(),
                    if mime_type == "image/jpeg" {
                        "jpg"
                    } else {
                        "png"
                    }
                    .to_owned(),
                )
            }
            ::gltf::image::Source::Uri { uri, mime_type } => {
                if uri.starts_with("data:") {
                    let (header, payload) = uri
                        .split_once(',')
                        .ok_or_else(|| invalid("Invalid image data URI"))?;
                    if !header.ends_with(";base64") {
                        return Err(invalid("Only base64 image data URIs are supported"));
                    }
                    (
                        base64::engine::general_purpose::STANDARD
                            .decode(payload)
                            .map_err(|e| invalid(e.to_string()))?,
                        if header.contains("image/jpeg") {
                            "jpg"
                        } else {
                            "png"
                        }
                        .into(),
                    )
                } else {
                    if uri.contains("://") {
                        return Err(invalid(
                            "Remote texture URI is unsupported; package textures locally",
                        ));
                    }
                    let source = path.parent().unwrap_or_else(|| Path::new(".")).join(uri);
                    let extension = mime_type
                        .map(|m| if m == "image/jpeg" { "jpg" } else { "png" })
                        .or_else(|| source.extension().and_then(|e| e.to_str()))
                        .unwrap_or("png")
                        .to_owned();
                    (std::fs::read(source)?, extension)
                }
            }
        };
        let key = format!("textures/gltf-{}.{}", uuid::Uuid::new_v4(), extension);
        scene.embedded_textures.insert(key.clone(), bytes);
        images.push(key);
    }
    let mut mesh = Mesh::new();
    let mut path_nodes = HashSet::new();
    let mut source_faces = std::collections::HashMap::new();
    let roots = parsed
        .document
        .scenes()
        .flat_map(|s| s.nodes())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        for node in parsed.document.nodes() {
            walk(
                node,
                DMat4::IDENTITY,
                &buffers,
                &images,
                &mut mesh,
                &mut path_nodes,
                0,
                source,
                &mut source_faces,
            )?;
        }
    } else {
        for node in roots {
            walk(
                node,
                DMat4::IDENTITY,
                &buffers,
                &images,
                &mut mesh,
                &mut path_nodes,
                0,
                source,
                &mut source_faces,
            )?;
        }
    }
    if source.is_none() {
        let bounds = mesh.vertices.iter().fold(
            (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(lo, hi), v| (lo.min(v.position), hi.max(v.position)),
        );
        let dimension = (bounds.1 - bounds.0).max_element();
        let scale = [1., 0.0254, 0.01, 0.001]
            .into_iter()
            .find(|s| {
                let d = dimension * s;
                d >= 0.2 && d <= 40.
            })
            .unwrap_or(1.);
        if scale != 1. {
            for vertex in &mut mesh.vertices {
                vertex.position *= scale;
            }
            for face in &mut mesh.faces {
                if let Some(values) = face
                    .attrs
                    .get_mut("texture")
                    .and_then(|v| v.get_mut("uvw"))
                    .and_then(|v| v.as_array_mut())
                {
                    for index in [0, 1, 2, 4, 5, 6] {
                        values[index] = serde_json::json!(values[index].as_f64().unwrap() / scale);
                    }
                }
            }
            mesh.rebuild_spatial_index();
        }
    }
    if let Some(source) = source {
        app_policy::finish(&mut mesh, &source_faces, source, &scene.embedded_textures)?;
    } else {
        for edge in &mut mesh.edges {
            if edge.faces.len() > 1 {
                edge.soft = true;
            }
        }
    }
    let title = parsed
        .document
        .as_json()
        .asset
        .extras
        .as_ref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.get()).ok())
        .and_then(|value| {
            value
                .get("title")
                .and_then(|t| t.as_str())
                .map(str::to_owned)
        });
    let title = if source.is_some() {
        title.filter(|s| !s.is_empty())
    } else {
        title
    };
    let name = title
        .as_deref()
        .or_else(|| path.file_stem().and_then(|s| s.to_str()))
        .unwrap_or("Imported glTF")
        .chars()
        .take(60)
        .collect::<String>();
    scene.groups.push(Group::with_mesh(name, mesh));
    scene.bump_version();
    Ok(scene)
}

fn walk(
    node: ::gltf::Node<'_>,
    parent: DMat4,
    buffers: &[::gltf::buffer::Data],
    images: &[String],
    mesh: &mut Mesh,
    stack: &mut HashSet<usize>,
    depth: usize,
    source: Option<&serde_json::Value>,
    source_faces: &mut std::collections::HashMap<usize, app_policy::SourceFace>,
) -> Result<(), GltfError> {
    if depth > 64 || !stack.insert(node.index()) {
        return Err(invalid("Recursive or excessively nested glTF nodes"));
    }
    let matrix = node
        .transform()
        .matrix()
        .map(|column| column.map(f64::from));
    let world = parent
        * if let Some(source) = source {
            app_policy::node_matrix(source, node.index())?
        } else {
            DMat4::from_cols_array_2d(&matrix)
        };
    if !world.is_finite() {
        return Err(invalid("Nonfinite glTF node transform"));
    }
    if let Some(node_mesh) = node.mesh() {
        for primitive in node_mesh.primitives() {
            if primitive.mode() != ::gltf::mesh::Mode::Triangles {
                return Err(invalid(
                    "Only glTF triangle primitives are currently supported",
                ));
            }
            let reader = primitive
                .reader(|buffer| buffers.get(buffer.index()).map(|data| data.0.as_slice()));
            let positions = reader
                .read_positions()
                .ok_or_else(|| invalid("glTF primitive has no readable positions"))?
                .map(|p| {
                    let p =
                        world.transform_point3(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
                    DVec3::new(p.x, -p.z, p.y)
                })
                .collect::<Vec<_>>();
            if positions.iter().any(|p| !p.is_finite()) {
                return Err(invalid("Nonfinite glTF vertex"));
            }
            let indices = reader
                .read_indices()
                .map(|i| i.into_u32().collect::<Vec<_>>())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            if indices.len() % 3 != 0 {
                return Err(invalid("Incomplete glTF triangle index array"));
            }
            let material = primitive.material();
            let pbr = material.pbr_metallic_roughness();
            let legacy = material.pbr_specular_glossiness();
            let color = legacy
                .as_ref()
                .map_or_else(|| pbr.base_color_factor(), |m| m.diffuse_factor());
            let texture = legacy
                .as_ref()
                .and_then(|m| m.diffuse_texture())
                .or_else(|| pbr.base_color_texture());
            let uv = texture
                .as_ref()
                .and_then(|info| {
                    reader.read_tex_coords(
                        info.texture_transform()
                            .and_then(|t| t.tex_coord())
                            .unwrap_or(info.tex_coord()),
                    )
                })
                .map(|data| {
                    if source.is_some() {
                        match data {
                            ::gltf::mesh::util::ReadTexCoords::U8(values) => values
                                .map(|p| DVec2::new(p[0] as f64 / 255., p[1] as f64 / 255.))
                                .collect(),
                            ::gltf::mesh::util::ReadTexCoords::U16(values) => values
                                .map(|p| DVec2::new(p[0] as f64 / 65535., p[1] as f64 / 65535.))
                                .collect(),
                            ::gltf::mesh::util::ReadTexCoords::F32(values) => values
                                .map(|p| DVec2::new(p[0] as f64, p[1] as f64))
                                .collect(),
                        }
                    } else {
                        data.into_f32()
                            .map(|p| DVec2::new(p[0] as f64, p[1] as f64))
                            .collect::<Vec<_>>()
                    }
                });
            if uv.as_ref().map_or(false, |values| {
                values.len() != positions.len() || values.iter().any(|v| !v.is_finite())
            }) {
                return Err(invalid("Invalid glTF texture coordinate array"));
            }
            for triangle in indices.chunks_exact(3) {
                let mut triangle = [
                    triangle[0] as usize,
                    triangle[1] as usize,
                    triangle[2] as usize,
                ];
                if triangle.iter().any(|i| *i >= positions.len()) {
                    return Err(invalid("glTF index outside position array"));
                }
                if source.is_none() && world.determinant() < 0. {
                    triangle.swap(1, 2);
                }
                let points = triangle.map(|i| positions[i]);
                let ids = points.map(|p| {
                    if source.is_some() {
                        let id = mesh.vertices.len();
                        mesh.vertices.push(ingetrazo_core::mesh::MeshVertex::new(p));
                        id
                    } else {
                        mesh.add_vertex(p)
                    }
                });
                let Some(face) = mesh.add_face(ids.to_vec(), vec![]) else {
                    return Err(invalid("Degenerate glTF triangle"));
                };
                if source.is_some() {
                    source_faces.insert(
                        face,
                        app_policy::SourceFace {
                            material: material.index(),
                            uv: None,
                        },
                    );
                }
                let attrs = &mut mesh.faces[face].attrs;
                attrs.insert(
                    "color".into(),
                    serde_json::json!([color[0], color[1], color[2]]),
                );
                if material.alpha_mode() == ::gltf::material::AlphaMode::Blend {
                    attrs.insert("opacity".into(), serde_json::json!(color[3]));
                }
                attrs.insert(
                    "gltf_roughness".into(),
                    serde_json::json!(pbr.roughness_factor()),
                );
                if let (Some(info), Some(uv)) = (texture.as_ref(), uv.as_ref()) {
                    let mut uv = triangle.map(|i| uv[i]);
                    if let Some(source) = source {
                        app_policy::transform_uv(source, material.index(), &mut uv)?;
                    } else if let Some(transform) = info.texture_transform() {
                        let offset = DVec2::from_array(transform.offset().map(f64::from));
                        let scale = DVec2::from_array(transform.scale().map(f64::from));
                        let rotation = transform.rotation() as f64;
                        for value in &mut uv {
                            let p = *value * scale;
                            *value = DVec2::new(
                                p.x * rotation.cos() - p.y * rotation.sin(),
                                p.x * rotation.sin() + p.y * rotation.cos(),
                            ) + offset;
                        }
                    }
                    if source.is_some() {
                        source_faces.get_mut(&face).unwrap().uv = Some(uv);
                    }
                    let e1 = points[1] - points[0];
                    let e2 = points[2] - points[0];
                    let normal = e1.cross(e2);
                    let matrix = DMat3::from_cols(e1, e2, normal).transpose();
                    if matrix.determinant().abs() < 1e-18 {
                        return Err(invalid("Degenerate glTF UV plane"));
                    }
                    let gu =
                        matrix.inverse() * DVec3::new(uv[1].x - uv[0].x, uv[2].x - uv[0].x, 0.);
                    let gv =
                        matrix.inverse() * DVec3::new(uv[1].y - uv[0].y, uv[2].y - uv[0].y, 0.);
                    let image = images
                        .get(info.texture().source().index())
                        .ok_or_else(|| invalid("Missing glTF texture image"))?;
                    attrs.insert("texture".into(),serde_json::json!({"embed":image,"uvw":[gu.x,gu.y,gu.z,uv[0].x-gu.dot(points[0]),gv.x,gv.y,gv.z,uv[0].y-gv.dot(points[0])]}));
                }
            }
        }
    }
    for child in node.children() {
        walk(
            child,
            world,
            buffers,
            images,
            mesh,
            stack,
            depth + 1,
            source,
            source_faces,
        )?;
    }
    stack.remove(&node.index());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_buffer_nested_transform_material_texture_uv_and_title() {
        let directory = std::env::temp_dir().join(format!("gltf-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let mut buffer = Vec::new();
        for value in [0_f32, 0., 0., 1., 0., 0., 0., 0., 1.] {
            buffer.extend(value.to_le_bytes());
        }
        for value in [0_f32, 0., 1., 0., 0., 1.] {
            buffer.extend(value.to_le_bytes());
        }
        std::fs::write(directory.join("mesh.bin"), &buffer).unwrap();
        let image=base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aSAAAAABJRU5ErkJggg==").unwrap();
        std::fs::write(directory.join("pixel.png"), &image).unwrap();
        let value = serde_json::json!({"asset":{"version":"2.0","extras":{"title":"Reference title"}},"buffers":[{"uri":"mesh.bin","byteLength":60}],"bufferViews":[{"buffer":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":24}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,0,1]},{"bufferView":1,"componentType":5126,"count":3,"type":"VEC2"}],"images":[{"uri":"pixel.png"}],"textures":[{"source":0}],"materials":[{"alphaMode":"BLEND","pbrMetallicRoughness":{"baseColorFactor":[0.2,0.3,0.4,0.5],"baseColorTexture":{"index":0}}}],"meshes":[{"primitives":[{"attributes":{"POSITION":0,"TEXCOORD_0":1},"material":0}]}],"nodes":[{"translation":[2,3,4],"children":[1]},{"mesh":0,"scale":[-2,3,1]}],"scenes":[{"nodes":[0]}],"scene":0});
        let path = directory.join("reference.gltf");
        std::fs::write(&path, value.to_string()).unwrap();
        let scene = load_gltf(&path).unwrap();
        assert_eq!(scene.groups[0].name, "Reference title");
        let mesh = &scene.groups[0].mesh;
        assert_eq!(mesh.faces.len(), 1);
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.position == DVec3::new(0., -4., 3.)));
        assert!(mesh
            .vertices
            .iter()
            .any(|v| v.position == DVec3::new(2., -5., 3.)));
        let face = &mesh.faces[0];
        assert!((face.attrs["opacity"].as_f64().unwrap() - 0.5).abs() < 1e-8);
        let texture = &face.attrs["texture"];
        assert_eq!(
            scene.embedded_textures[texture["embed"].as_str().unwrap()],
            image
        );
        let uv = texture["uvw"].as_array().unwrap();
        let n = |i: usize| uv[i].as_f64().unwrap();
        let p = DVec3::new(0., -4., 3.);
        assert!((DVec3::new(n(0), n(1), n(2)).dot(p) + n(3) - 1.).abs() < 1e-8);
        assert!((DVec3::new(n(4), n(5), n(6)).dot(p) + n(7)).abs() < 1e-8);
        std::fs::write(directory.join("mesh.bin"), [0u8; 8]).unwrap();
        assert!(load_gltf(&path).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[path = "gltf_app_import.rs"]
mod app_policy;
