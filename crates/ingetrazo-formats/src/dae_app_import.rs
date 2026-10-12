//! Explicit original App policy; general load_dae keeps its retained legacy behavior.
use super::*;
use image::ImageDecoder;
use ingetrazo_core::{groups::Group, mesh::Mesh};
use std::{io::Read, path::Path};
fn normal(mesh: &Mesh, face: &ingetrazo_core::mesh::MeshFace) -> glam::Vec3 {
    let mut n = glam::Vec3::ZERO;
    for i in 0..face.vertices.len() {
        let a = mesh.vertices[face.vertices[i]].position;
        let b = mesh.vertices[face.vertices[(i + 1) % face.vertices.len()]].position;
        n += glam::Vec3::new(
            ((a.y - b.y) * (a.z + b.z)) as f32,
            ((a.z - b.z) * (a.x + b.x)) as f32,
            ((a.x - b.x) * (a.y + b.y)) as f32,
        );
    }
    n
}
fn image_cutout(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > 16 * 1024 * 1024 {
        return false;
    }
    let Ok(format) = image::guess_format(bytes) else {
        return false;
    };
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let Ok(decoder) = reader.into_decoder() else {
        return false;
    };
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || (width as u64)
            .checked_mul(height as u64)
            .and_then(|n| n.checked_mul(16))
            .is_none_or(|n| n > 64 * 1024 * 1024)
    {
        return false;
    }
    let Ok(image) = image::DynamicImage::from_decoder(decoder) else {
        return false;
    };
    let rgba = image.into_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return false;
    }
    // Source Qt FastTransformation to32x32, IgnoreAspectRatio.
    for y in 0..32 {
        for x in 0..32 {
            let sx = (((2 * x + 1) as u64 * w as u64) / 64).min(w as u64 - 1) as u32;
            let sy = (((2 * y + 1) as u64 * h as u64) / 64).min(h as u64 - 1) as u32;
            if rgba.get_pixel(sx, sy)[3] < 32 {
                return true;
            }
        }
    }
    false
}
fn corner_key(value: f64) -> String {
    let key = format!("{value:.4}");
    if key == "-0.0000" {
        "0.0000".into()
    } else {
        key
    }
}
pub(super) fn faceme(
    group: &Group,
    assets: &HashMap<String, Vec<u8>>,
    alpha: &mut HashMap<String, bool>,
) -> bool {
    if !(1..=400).contains(&group.mesh.faces.len()) {
        return false;
    }
    let (mut path, mut reference) = (None, None);
    let mut corners = std::collections::HashSet::new();
    for face in &group.mesh.faces {
        let Some(image) = face
            .attrs
            .get("texture")
            .and_then(|v| v.get("embed"))
            .and_then(|v| v.as_str())
        else {
            return false;
        };
        if path.is_some_and(|p| p != image) {
            return false;
        }
        path = Some(image);
        let n = normal(&group.mesh, face);
        let length = n.x.hypot(n.y).hypot(n.z);
        if length < 1e-12 {
            continue;
        }
        let n = n / length;
        if n.z.abs() > 0.08 {
            return false;
        }
        if reference.is_some_and(|r: glam::Vec3| r.dot(n).abs() < 0.99) {
            return false;
        }
        reference = Some(n);
        for &v in &face.vertices {
            let p = group.mesh.vertices[v].position;
            corners.insert([corner_key(p.x), corner_key(p.y), corner_key(p.z)]);
        }
    }
    if reference.is_none() {
        return false;
    }
    let image = path.unwrap();
    corners.len() > 4
        || *alpha
            .entry(image.into())
            .or_insert_with(|| assets.get(image).is_some_and(|bytes| image_cutout(bytes)))
}
pub(super) fn combine(groups: &[&Group]) -> Result<Mesh> {
    let mut mesh = Mesh::new();
    for group in groups {
        append(&mut mesh, group)?;
    }
    Ok(mesh)
}
fn append(mesh: &mut Mesh, group: &Group) -> Result<()> {
    for face in &group.mesh.faces {
        let rings = std::iter::once(&face.vertices)
            .chain(face.holes.iter())
            .map(|r| {
                r.iter()
                    .map(|&v| mesh.add_vertex(group.mesh.vertices[v].position))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let i = mesh
            .add_face(rings[0].clone(), rings[1..].to_vec())
            .ok_or_else(|| invalid("invalid prepared COLLADA face"))?;
        mesh.faces[i].attrs = face.attrs.clone();
        mesh.faces[i].layer = face.layer.clone();
    }
    Ok(())
}
pub fn parse_dae_import(path: &Path) -> Result<crate::FileImportPlan> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(invalid("COLLADA encoded input exceeds native admission"));
    }
    let xml = std::str::from_utf8(&bytes).map_err(|_| invalid("COLLADA is not UTF-8"))?;
    let mut scene =
        scene_from_xml_policy(xml, Some(path.parent().unwrap_or(Path::new("."))), true)?;
    let reference = authored_count(xml)? > 400;
    let mut sprites = Vec::new();
    let mut static_groups = Vec::new();
    for mut group in std::mem::take(&mut scene.groups) {
        if group.billboard {
            group.mesh = ingetrazo_core::simplify::simplify_mesh(&group.mesh, 0., false)
                .map_err(invalid)?
                .0;
            crate::obj::obj_source_soften(&mut group.mesh);
            group.billboard = true;
            crate::obj::source_group_kind(&mut group, false);
            group.native_document["billboard"] = serde_json::json!("mesh");
            group.native_baseline["billboard"] = serde_json::json!(true);
            sprites.push(group);
        } else {
            static_groups.push(group);
        }
    }
    scene.groups = static_groups;
    if reference && !scene.groups.is_empty() {
        scene = finish_dae_scene_policy(path, xml, scene, true)?;
    } else {
        for group in std::mem::take(&mut scene.groups) {
            append(&mut scene.mesh, &group)?;
        }
    }
    for group in &mut scene.groups {
        if group.native_document.is_null() {
            let component = group.component_definition.is_some();
            crate::obj::source_group_kind(group, component);
        }
    }
    scene.groups.extend(sprites);
    Ok(crate::FileImportPlan::from_file_scene(scene))
}

pub(super) fn authored_count(xml: &str) -> Result<usize> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| invalid(e.to_string()))?;
    let ids: HashMap<_, _> = doc
        .descendants()
        .filter_map(|n| n.attribute("id").map(|id| (id, n)))
        .collect();
    fn primitives(mesh: Node<'_, '_>) -> usize {
        mesh.children()
            .filter(|n| n.is_element())
            .map(|n| match n.tag_name().name() {
                "triangles" | "polylist" => n
                    .attribute("count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                "polygons" => n
                    .children()
                    .filter(|p| p.is_element() && p.tag_name().name() == "p")
                    .count(),
                _ => 0,
            })
            .sum()
    }
    fn count<'a, 'i>(
        node: Node<'a, 'i>,
        ids: &HashMap<&'i str, Node<'a, 'i>>,
        stack: &mut Vec<roxmltree::NodeId>,
    ) -> Result<usize> {
        if stack.len() > 32 || stack.contains(&node.id()) {
            return Err(invalid("Cyclic authored COLLADA hierarchy"));
        }
        stack.push(node.id());
        let mut sum = 0usize;
        for n in node.children().filter(|n| n.is_element()) {
            let add = match n.tag_name().name() {
                "instance_geometry" => n
                    .attribute("url")
                    .and_then(|u| ids.get(u.trim_start_matches('#')))
                    .and_then(|g| child(*g, "mesh"))
                    .map(primitives)
                    .unwrap_or(0),
                "node" => count(n, ids, stack)?,
                "instance_node" => {
                    if let Some(t) = n
                        .attribute("url")
                        .and_then(|u| ids.get(u.trim_start_matches('#')))
                    {
                        count(*t, ids, stack)?
                    } else {
                        0
                    }
                }
                _ => 0,
            };
            sum = sum
                .checked_add(add)
                .ok_or_else(|| invalid("COLLADA count overflow"))?;
        }
        stack.pop();
        Ok(sum)
    }
    let roots: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "visual_scene")
        .flat_map(|n| {
            n.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "node")
        })
        .collect();
    if roots.is_empty() {
        return Ok(doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "geometry")
            .filter_map(|n| child(n, "mesh"))
            .map(primitives)
            .sum());
    }
    roots.into_iter().try_fold(0usize, |sum, n| {
        sum.checked_add(count(n, &ids, &mut vec![])?)
            .ok_or_else(|| invalid("COLLADA count overflow"))
    })
}
