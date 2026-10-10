//! Checked COLLADA geometry decoding; scene/material assembly is added separately.
use glam::DVec3;
use roxmltree::Node;
use std::collections::HashMap;
#[derive(thiserror::Error, Debug)]
pub enum DaeError {
    #[error("COLLADA I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid COLLADA: {0}")]
    Invalid(String),
}
type Result<T> = std::result::Result<T, DaeError>;
fn invalid(message: impl Into<String>) -> DaeError {
    DaeError::Invalid(message.into())
}
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}
fn numbers(node: Node<'_, '_>) -> Result<Vec<f64>> {
    node.text()
        .unwrap_or("")
        .split_whitespace()
        .map(|word| {
            word.parse::<f64>()
                .map_err(|_| invalid("invalid number"))
                .and_then(|v| {
                    if v.is_finite() {
                        Ok(v)
                    } else {
                        Err(invalid("nonfinite number"))
                    }
                })
        })
        .collect()
}
fn integer(node: Node<'_, '_>, name: &str, default: usize) -> Result<usize> {
    node.attribute(name).map_or(Ok(default), |s| {
        s.parse().map_err(|_| invalid(format!("invalid {name}")))
    })
}
fn indices(node: Node<'_, '_>) -> Result<Vec<usize>> {
    node.text()
        .unwrap_or("")
        .split_whitespace()
        .map(|s| s.parse().map_err(|_| invalid("invalid or negative index")))
        .collect()
}
pub(crate) fn geometry(mesh: Node<'_, '_>) -> Result<Vec<Vec<DVec3>>> {
    let sources: HashMap<_, _> = mesh
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "source")
        .filter_map(|n| n.attribute("id").map(|id| (id, n)))
        .collect();
    let vertices = child(mesh, "vertices").ok_or_else(|| invalid("missing vertices"))?;
    let input = vertices
        .children()
        .find(|n| n.attribute("semantic") == Some("POSITION"))
        .ok_or_else(|| invalid("missing POSITION"))?;
    let source = input
        .attribute("source")
        .and_then(|url| sources.get(url.trim_start_matches('#')))
        .ok_or_else(|| invalid("missing position source"))?;
    let array = child(*source, "float_array").ok_or_else(|| invalid("missing float_array"))?;
    let values = numbers(array)?;
    if integer(array, "count", values.len())? != values.len() {
        return Err(invalid("float_array count mismatch"));
    }
    let accessor = child(*source, "technique_common").and_then(|n| child(n, "accessor"));
    let stride = accessor.map_or(Ok(3), |n| integer(n, "stride", 3))?;
    let offset = accessor.map_or(Ok(0), |n| integer(n, "offset", 0))?;
    if stride < 3 {
        return Err(invalid("position stride below3"));
    }
    let count = accessor.map_or(Ok(values.len() / stride), |n| {
        integer(n, "count", values.len() / stride)
    })?;
    let required = count
        .checked_sub(1)
        .and_then(|n| n.checked_mul(stride))
        .and_then(|n| n.checked_add(offset))
        .and_then(|n| n.checked_add(3));
    if count > 0 && required.map_or(true, |n| n > values.len()) {
        return Err(invalid("position accessor exceeds array"));
    }
    let positions: Vec<_> = (0..count)
        .map(|i| {
            let n = offset + i * stride;
            DVec3::new(values[n], values[n + 1], values[n + 2])
        })
        .collect();
    let mut loops = vec![];
    for primitive in mesh.children().filter(|n| n.is_element()) {
        let kind = primitive.tag_name().name();
        if !matches!(kind, "triangles" | "polylist" | "polygons") {
            continue;
        }
        let inputs = primitive
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "input")
            .collect::<Vec<_>>();
        let vertex = inputs
            .iter()
            .find(|n| n.attribute("semantic") == Some("VERTEX"))
            .ok_or_else(|| invalid("primitive missing VERTEX"))?;
        if vertex
            .attribute("source")
            .map(|s| s.trim_start_matches('#'))
            != vertices.attribute("id")
        {
            return Err(invalid("primitive refers to unknown vertices"));
        }
        let vertex_offset = integer(*vertex, "offset", 0)?;
        let stride = inputs
            .iter()
            .map(|n| integer(*n, "offset", 0))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| invalid("index stride overflow"))?;
        let mut emit = |stream: &[usize], count: usize| -> Result<()> {
            if count < 3
                || stream.len()
                    != count
                        .checked_mul(stride)
                        .ok_or_else(|| invalid("primitive size overflow"))?
            {
                return Err(invalid("invalid polygon index count"));
            }
            let polygon = (0..count)
                .map(|i| {
                    positions
                        .get(stream[i * stride + vertex_offset])
                        .copied()
                        .ok_or_else(|| invalid("vertex index outside position accessor"))
                })
                .collect::<Result<Vec<_>>>()?;
            loops.push(polygon);
            Ok(())
        };
        if kind == "polygons" {
            for p in primitive
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "p")
            {
                let stream = indices(p)?;
                if stream.len() % stride != 0 {
                    return Err(invalid("incomplete polygon index tuple"));
                }
                emit(&stream, stream.len() / stride)?;
            }
        } else {
            let stream =
                indices(child(primitive, "p").ok_or_else(|| invalid("missing index stream"))?)?;
            let counts = if kind == "triangles" {
                vec![3; integer(primitive, "count", stream.len() / stride / 3)?]
            } else {
                indices(child(primitive, "vcount").ok_or_else(|| invalid("missing vcount"))?)?
            };
            if integer(primitive, "count", counts.len())? != counts.len() {
                return Err(invalid("primitive count mismatch"));
            }
            let mut cursor = 0usize;
            for count in counts {
                let end = count
                    .checked_mul(stride)
                    .and_then(|n| cursor.checked_add(n))
                    .ok_or_else(|| invalid("primitive size overflow"))?;
                let part = stream
                    .get(cursor..end)
                    .ok_or_else(|| invalid("truncated primitive stream"))?;
                emit(part, count)?;
                cursor = end;
            }
            if cursor != stream.len() {
                return Err(invalid("extra primitive indices"));
            }
        }
    }
    Ok(loops)
}

fn transform(node: Node<'_, '_>) -> Result<glam::DMat4> {
    use glam::DMat4;
    let mut m = DMat4::IDENTITY;
    for el in node.children().filter(|n| n.is_element()) {
        let kind = el.tag_name().name();
        if !matches!(kind, "matrix" | "translate" | "rotate" | "scale") {
            continue;
        }
        let v = numbers(el)?;
        let next = match kind {
            "matrix" => {
                let values: [f64; 16] = v
                    .try_into()
                    .map_err(|_| invalid("matrix requires16 values"))?;
                DMat4::from_cols_array(&values).transpose()
            }
            "translate" | "scale" => {
                let values: [f64; 3] = v
                    .try_into()
                    .map_err(|_| invalid("translation/scale requires3 values"))?;
                let p = DVec3::from_array(values);
                if kind == "scale" {
                    DMat4::from_scale(p)
                } else {
                    DMat4::from_translation(p)
                }
            }
            "rotate" => {
                let values: [f64; 4] = v
                    .try_into()
                    .map_err(|_| invalid("rotation requires4 values"))?;
                let axis = DVec3::new(values[0], values[1], values[2]);
                if axis.length() < 1e-12 {
                    return Err(invalid("zero rotation axis"));
                }
                DMat4::from_axis_angle(axis.normalize(), values[3].to_radians())
            }
            _ => unreachable!(),
        };
        m *= next;
    }
    if !m.is_finite()
        || (m.x_axis.w != 0. || m.y_axis.w != 0. || m.z_axis.w != 0. || m.w_axis.w != 1.)
    {
        return Err(invalid("node matrix must be finite affine"));
    }
    Ok(m)
}
pub(crate) fn scene_from_xml(xml: &str) -> Result<ingetrazo_core::scene::Scene> {
    use glam::{DMat4, DVec4};
    use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
    let doc = roxmltree::Document::parse(xml).map_err(|e| invalid(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "COLLADA" {
        return Err(invalid("root is not COLLADA"));
    }
    let mut ids = HashMap::new();
    for el in root.descendants().filter(|n| n.is_element()) {
        if let Some(id) = el.attribute("id") {
            if ids.insert(id, el).is_some() {
                return Err(invalid("duplicate XML identity"));
            }
        }
    }
    let asset = child(root, "asset");
    let unit = asset
        .and_then(|n| child(n, "unit"))
        .and_then(|n| n.attribute("meter"))
        .unwrap_or("1")
        .parse::<f64>()
        .map_err(|_| invalid("invalid unit scale"))?;
    if !unit.is_finite() || unit <= 0. {
        return Err(invalid("unit scale must be positive finite"));
    }
    let up = asset
        .and_then(|n| child(n, "up_axis"))
        .and_then(|n| n.text())
        .unwrap_or("Z_UP")
        .trim();
    let axis = match up {
        "Y_UP" => DMat4::from_cols(DVec4::X, DVec4::Z, -DVec4::Y, DVec4::W),
        "X_UP" => DMat4::from_cols(DVec4::Y, DVec4::X, DVec4::Z, DVec4::W),
        "Z_UP" => DMat4::IDENTITY,
        _ => return Err(invalid("unknown up_axis")),
    };
    let world = axis * DMat4::from_scale(DVec3::splat(unit));
    let mut scene = Scene::new();
    fn mesh_group(node: Node<'_, '_>, world: DMat4, name: &str) -> Result<Group> {
        let mut mesh = Mesh::new();
        for polygon in geometry(node)? {
            let mut vertices: Vec<_> = polygon
                .into_iter()
                .map(|p| mesh.add_vertex(world.transform_point3(p)))
                .collect();
            if world.determinant() < 0. {
                vertices.reverse();
            }
            mesh.add_face(vertices, vec![])
                .ok_or_else(|| invalid("invalid transformed polygon"))?;
        }
        Ok(Group::with_mesh(name, mesh))
    }
    fn walk<'a, 'i>(
        node: Node<'a, 'i>,
        parent: DMat4,
        ids: &HashMap<&'i str, Node<'a, 'i>>,
        stack: &mut Vec<roxmltree::NodeId>,
        scene: &mut Scene,
    ) -> Result<()> {
        if stack.len() >= 64 || stack.contains(&node.id()) {
            return Err(invalid("cyclic or excessive node hierarchy"));
        }
        stack.push(node.id());
        let world = parent * transform(node)?;
        let name = node
            .attribute("name")
            .or(node.attribute("id"))
            .unwrap_or("COLLADA part");
        for el in node.children().filter(|n| n.is_element()) {
            match el.tag_name().name() {
                "node" => walk(el, world, ids, stack, scene)?,
                "instance_node" => {
                    let target = el
                        .attribute("url")
                        .filter(|s| s.starts_with('#'))
                        .and_then(|s| ids.get(&s[1..]))
                        .ok_or_else(|| invalid("unresolved instance_node"))?;
                    walk(*target, world, ids, stack, scene)?;
                }
                "instance_geometry" => {
                    let target = el
                        .attribute("url")
                        .filter(|s| s.starts_with('#'))
                        .and_then(|s| ids.get(&s[1..]))
                        .ok_or_else(|| invalid("unresolved geometry"))?;
                    let mesh =
                        child(*target, "mesh").ok_or_else(|| invalid("geometry has no mesh"))?;
                    let group = mesh_group(mesh, world, name)?;
                    if !group.mesh.faces.is_empty() {
                        scene.groups.push(group);
                    }
                }
                "instance_controller" => {
                    return Err(invalid("controller geometry assembly is not implemented"))
                }
                _ => {}
            }
        }
        stack.pop();
        Ok(())
    }
    let nodes: Vec<_> = root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "visual_scene")
        .flat_map(|n| {
            n.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "node")
        })
        .collect();
    if nodes.is_empty() {
        for geometry_node in root
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "geometry")
        {
            if let Some(mesh) = child(geometry_node, "mesh") {
                let group = mesh_group(
                    mesh,
                    world,
                    geometry_node
                        .attribute("name")
                        .unwrap_or("COLLADA geometry"),
                )?;
                if !group.mesh.faces.is_empty() {
                    scene.groups.push(group);
                }
            }
        }
    } else {
        for node in nodes {
            walk(node, world, &ids, &mut vec![], &mut scene)?;
        }
    }
    if scene.groups.is_empty() {
        return Err(invalid("no geometry"));
    }
    scene.bump_version();
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> String {
        r##"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema"><asset><unit meter="0.01"/><up_axis>Y_UP</up_axis></asset><library_geometries><geometry id="g"><mesh><source id="p"><float_array count="12">99 0 0 0 9 1 0 0 9 0 1 0</float_array><technique_common><accessor count="3" offset="1" stride="4"/></technique_common></source><vertices id="v"><input semantic="POSITION" source="#p"/></vertices><triangles count="1"><input semantic="NORMAL" offset="0"/><input semantic="VERTEX" source="#v" offset="1"/><p>0 0 0 1 0 2</p></triangles></mesh></geometry></library_geometries><library_nodes><node id="shared"><scale>-2 3 4</scale><instance_geometry url="#g"/></node></library_nodes><library_visual_scenes><visual_scene><node name="parent"><translate>10 20 30</translate><instance_node url="#shared"/></node></visual_scene></library_visual_scenes></COLLADA>"##.to_owned()
    }
    #[test]
    fn strided_accessors_instances_units_upaxis_and_reflection_are_composed() {
        let scene = scene_from_xml(&document()).unwrap();
        assert_eq!(scene.groups.len(), 1);
        let mesh = &scene.groups[0].mesh;
        let expected = [
            DVec3::new(0.10, -0.30, 0.20),
            DVec3::new(0.08, -0.30, 0.20),
            DVec3::new(0.10, -0.30, 0.23),
        ];
        for p in expected {
            assert!(mesh
                .vertices
                .iter()
                .any(|v| (v.position - p).length() < 1e-10));
        }
        assert!(mesh.face_normal(0).y < 0.);
    }
    #[test]
    fn truncated_indices_accessors_and_node_cycles_fail_without_partial_scene() {
        let xml = document();
        assert!(scene_from_xml(&xml.replace("0 0 0 1 0 2", "0 0 0 1 0 99")).is_err());
        assert!(scene_from_xml(
            &xml.replace("count=\"3\" offset=\"1\"", "count=\"4\" offset=\"1\"")
        )
        .is_err());
        assert!(scene_from_xml(
            &xml.replace("<scale>-2 3 4</scale>", "<instance_node url=\"#shared\"/>")
        )
        .is_err());
    }
}
