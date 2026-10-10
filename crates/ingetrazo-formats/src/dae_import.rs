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
struct Polygon {
    uv: Option<Vec<glam::DVec2>>,
    points: Vec<DVec3>,
    material: Option<String>,
}
/// Fit a world-space affine UV map, retaining every authored polygon sample.
fn affine_uv(points: &[DVec3], uv: &[glam::DVec2]) -> Result<[f64; 8]> {
    if points.len() != uv.len()
        || points.len() < 3
        || points.iter().any(|p| !p.is_finite())
        || uv.iter().any(|p| !p.is_finite())
    {
        return Err(invalid("invalid polygon UV samples"));
    }
    let mut basis = None;
    for i in 1..points.len() {
        for j in i + 1..points.len() {
            let a = points[i] - points[0];
            let b = points[j] - points[0];
            let n = a.cross(b);
            if n.length_squared() > 1e-24 {
                basis = Some((
                    i,
                    j,
                    glam::DMat3::from_cols(a, b, n.normalize()).transpose(),
                ));
                break;
            }
        }
        if basis.is_some() {
            break;
        }
    }
    let (i, j, m) = basis.ok_or_else(|| invalid("degenerate polygon UV plane"))?;
    let gu = m.inverse() * DVec3::new(uv[i].x - uv[0].x, uv[j].x - uv[0].x, 0.);
    let gv = m.inverse() * DVec3::new(uv[i].y - uv[0].y, uv[j].y - uv[0].y, 0.);
    let u0 = uv[0].x - gu.dot(points[0]);
    let v0 = uv[0].y - gv.dot(points[0]);
    for (p, t) in points.iter().zip(uv) {
        let predicted = glam::DVec2::new(gu.dot(*p) + u0, gv.dot(*p) + v0);
        if predicted.distance(*t) > 1e-6 * (1. + t.length()) {
            return Err(invalid("non-affine polygon UV mapping"));
        }
    }
    Ok([gu.x, gu.y, gu.z, u0, gv.x, gv.y, gv.z, v0])
}
fn geometry(mesh: Node<'_, '_>) -> Result<Vec<Polygon>> {
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
        let uv_input = inputs
            .iter()
            .find(|n| n.attribute("semantic") == Some("TEXCOORD"));
        let uv_source = if let Some(input) = uv_input {
            let offset = integer(*input, "offset", 0)?;
            let source = input
                .attribute("source")
                .and_then(|s| sources.get(s.trim_start_matches('#')))
                .ok_or_else(|| invalid("unresolved texture coordinate source"))?;
            let array = child(*source, "float_array")
                .ok_or_else(|| invalid("missing texture float_array"))?;
            let values = numbers(array)?;
            if integer(array, "count", values.len())? != values.len() {
                return Err(invalid("texture array count mismatch"));
            }
            let accessor = child(*source, "technique_common").and_then(|n| child(n, "accessor"));
            let step = accessor.map_or(Ok(2), |n| integer(n, "stride", 2))?;
            if step < 2 {
                return Err(invalid("texture stride below2"));
            }
            let start = accessor.map_or(Ok(0), |n| integer(n, "offset", 0))?;
            let count = accessor.map_or(Ok(values.len() / step), |n| {
                integer(n, "count", values.len() / step)
            })?;
            let mut coordinates = vec![];
            for i in 0..count {
                let index = i
                    .checked_mul(step)
                    .and_then(|n| n.checked_add(start))
                    .ok_or_else(|| invalid("texture accessor overflow"))?;
                let end = index
                    .checked_add(2)
                    .ok_or_else(|| invalid("texture accessor overflow"))?;
                let pair = values
                    .get(index..end)
                    .ok_or_else(|| invalid("texture accessor exceeds array"))?;
                coordinates.push(glam::DVec2::new(pair[0], pair[1]));
            }
            Some((offset, coordinates))
        } else {
            None
        };
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
            let uv = uv_source
                .as_ref()
                .map(|(offset, coordinates)| {
                    (0..count)
                        .map(|i| {
                            coordinates
                                .get(stream[i * stride + offset])
                                .copied()
                                .ok_or_else(|| invalid("texture index outside accessor"))
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?;
            loops.push(Polygon {
                uv,
                points: polygon,
                material: primitive.attribute("material").map(str::to_owned),
            });
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

/// Resolve an authored local image URI without fetching remote resources.
fn image_bytes(image: Node<'_, '_>, base: &std::path::Path) -> Result<(String, Vec<u8>)> {
    let reference = child(image, "init_from")
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("missing image URI"))?;
    if reference.contains("://") || reference.starts_with("data:") {
        return Err(invalid("COLLADA image must reference a local file"));
    }
    let input = reference.as_bytes();
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' {
            if i + 2 >= input.len() {
                return Err(invalid("truncated image URI escape"));
            }
            let hex = std::str::from_utf8(&input[i + 1..i + 3])
                .map_err(|_| invalid("invalid image URI escape"))?;
            bytes.push(
                u8::from_str_radix(hex, 16).map_err(|_| invalid("invalid image URI escape"))?,
            );
            i += 3;
        } else {
            bytes.push(input[i]);
            i += 1;
        }
    }
    let decoded = String::from_utf8(bytes).map_err(|_| invalid("image URI is not UTF-8"))?;
    if decoded.contains('\0') {
        return Err(invalid("image URI contains NUL"));
    }
    let path = base.join(decoded);
    let bytes = std::fs::read(&path)?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    Ok((
        format!("textures/dae-{}.{}", uuid::Uuid::new_v4(), extension),
        bytes,
    ))
}
/// Resolve exporter-specific direct image and sampler/surface chains within one effect.
fn diffuse_image<'a, 'i>(
    effect: Node<'a, 'i>,
    texture: Node<'a, 'i>,
    ids: &HashMap<&'i str, Node<'a, 'i>>,
) -> Result<Node<'a, 'i>> {
    let reference = texture
        .attribute("texture")
        .ok_or_else(|| invalid("missing texture sampler"))?;
    if let Some(image) = ids
        .get(reference)
        .filter(|n| n.tag_name().name() == "image")
    {
        return Ok(*image);
    }
    let parameter = |sid: &str| -> Result<Node<'a, 'i>> {
        let found: Vec<_> = effect
            .descendants()
            .filter(|n| {
                n.is_element()
                    && n.tag_name().name() == "newparam"
                    && n.attribute("sid") == Some(sid)
            })
            .collect();
        if found.len() != 1 {
            return Err(invalid("missing or ambiguous effect parameter"));
        }
        Ok(found[0])
    };
    let sampler = parameter(reference)?;
    let surface_id = child(sampler, "sampler2D")
        .and_then(|n| child(n, "source"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("missing sampler surface"))?;
    let surface = parameter(surface_id)?;
    let image_id = child(surface, "surface")
        .and_then(|n| child(n, "init_from"))
        .and_then(|n| n.text())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("missing surface image"))?;
    ids.get(image_id)
        .filter(|n| n.tag_name().name() == "image")
        .copied()
        .ok_or_else(|| invalid("unresolved texture image"))
}
fn representative_image_color(bytes: &[u8]) -> Option<[f64; 3]> {
    image::load_from_memory(bytes).ok().map(|decoded| {
        let decoded = decoded.to_rgba8();
        if decoded.pixels().all(|p| p[3] == 255) {
            // Qt smooth downsampling uses 14-bit area weights,
            // truncating horizontal accumulation before vertical.
            let weights = |n: u32| {
                let step = 16384_u64.div_ceil(u64::from(n));
                let mut left = 16384_u64;
                (0..n)
                    .map(|_| {
                        let w = step.min(left);
                        left -= w;
                        w
                    })
                    .collect::<Vec<_>>()
            };
            let wx = weights(decoded.width());
            let wy = weights(decoded.height());
            let mut total = [0_u64; 3];
            for (y, row) in decoded.rows().enumerate() {
                let mut horizontal = [0_u64; 3];
                for (x, pixel) in row.enumerate() {
                    for i in 0..3 {
                        horizontal[i] += u64::from(pixel[i]) * wx[x];
                    }
                }
                for i in 0..3 {
                    total[i] += (horizontal[i] >> 4) * wy[y];
                }
            }
            return total.map(|v| (v >> 24) as f64 / 255.);
        }
        let mut sums = [0_u64; 3];
        let mut alpha = 0_u64;
        for pixel in decoded.pixels() {
            alpha += u64::from(pixel[3]);
            for i in 0..3 {
                sums[i] += u64::from(pixel[i]) * u64::from(pixel[3]);
            }
        }
        // Smooth image reduction averages premultiplied colors:
        // transparent RGB must not tint the material fallback.
        sums.map(|sum| ((sum + alpha / 2) / alpha.max(1)) as f64 / 255.)
    })
}
#[derive(Default)]
struct DaeMaterial {
    color: Option<[f64; 3]>,
    image: Option<String>,
}
fn material_colors<'a, 'i>(
    instance: Node<'a, 'i>,
    ids: &HashMap<&'i str, Node<'a, 'i>>,
    base: Option<&std::path::Path>,
    assets: &mut HashMap<String, Vec<u8>>,
) -> Result<HashMap<String, DaeMaterial>> {
    let mut colors = HashMap::new();
    for binding in instance
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "instance_material")
    {
        let material = binding
            .attribute("target")
            .and_then(|s| ids.get(s.trim_start_matches('#')))
            .ok_or_else(|| invalid("unresolved material binding"))?;
        let effect = child(*material, "instance_effect")
            .and_then(|n| n.attribute("url"))
            .and_then(|s| ids.get(s.trim_start_matches('#')))
            .ok_or_else(|| invalid("unresolved material effect"))?;
        for shader in effect.descendants().filter(|n| {
            n.is_element()
                && matches!(
                    n.tag_name().name(),
                    "lambert" | "phong" | "blinn" | "constant"
                )
        }) {
            let Some(diffuse) = child(shader, "diffuse") else {
                continue;
            };
            if let Some(texture) = child(diffuse, "texture") {
                let image = diffuse_image(*effect, texture, ids)?;
                let base =
                    base.ok_or_else(|| invalid("textured COLLADA requires document directory"))?;
                let (key, bytes) = image_bytes(image, base)?;
                let symbol = binding
                    .attribute("symbol")
                    .ok_or_else(|| invalid("missing material symbol"))?;
                let fallback = representative_image_color(&bytes);
                assets.insert(key.clone(), bytes);
                colors.insert(
                    symbol.to_owned(),
                    DaeMaterial {
                        color: fallback,
                        image: Some(key),
                    },
                );
                break;
            }
            if let Some(color) = child(diffuse, "color") {
                let values = numbers(color)?;
                if values.len() < 3 {
                    return Err(invalid("diffuse color requiresRGB"));
                }
                let symbol = binding
                    .attribute("symbol")
                    .ok_or_else(|| invalid("missing material symbol"))?;
                colors.insert(
                    symbol.to_owned(),
                    DaeMaterial {
                        color: Some([values[0], values[1], values[2]]),
                        image: None,
                    },
                );
                break;
            }
        }
    }
    Ok(colors)
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
pub fn load_dae(path: &std::path::Path) -> Result<ingetrazo_core::scene::Scene> {
    let xml = std::fs::read_to_string(path)?;
    let mut scene = scene_from_xml_at(
        &xml,
        Some(path.parent().unwrap_or_else(|| std::path::Path::new("."))),
    )?;
    let count: usize = scene.groups.iter().map(|g| g.mesh.faces.len()).sum();
    if count <= 400 {
        let mut mesh = ingetrazo_core::mesh::Mesh::new();
        for group in &scene.groups {
            for face in &group.mesh.faces {
                let outer = face
                    .vertices
                    .iter()
                    .map(|&v| mesh.add_vertex(group.mesh.vertices[v].position))
                    .collect();
                let holes = face
                    .holes
                    .iter()
                    .map(|ring| {
                        ring.iter()
                            .map(|&v| mesh.add_vertex(group.mesh.vertices[v].position))
                            .collect()
                    })
                    .collect();
                let id = mesh
                    .add_face(outer, holes)
                    .ok_or_else(|| invalid("invalid editable COLLADA face"))?;
                mesh.faces[id].attrs = face.attrs.clone();
                mesh.faces[id].layer = face.layer.clone();
            }
        }
        let (mut fused, _) =
            ingetrazo_core::simplify::simplify_mesh(&mesh, 0., false).map_err(invalid)?;
        // Import stitching retains the source boundary stations even when
        // same-plane triangle edges disappear from the editable surface.
        for vertex in &mesh.vertices {
            fused.add_vertex(vertex.position);
        }
        let fused = ingetrazo_core::planar::stitch_boundaries(&fused).map_err(invalid)?;
        scene.mesh = ingetrazo_core::heal::orient_faces(&fused).map_err(invalid)?;
        scene.groups.clear();
        scene.bump_version();
    } else {
        let document = roxmltree::Document::parse(&xml).map_err(|e| invalid(e.to_string()))?;
        let bare = !document.descendants().any(|n| {
            n.is_element()
                && n.tag_name().name() == "visual_scene"
                && n.children()
                    .any(|c| c.is_element() && c.tag_name().name() == "node")
        });
        if bare {
            let mut combined = ingetrazo_core::mesh::Mesh::new();
            for group in &scene.groups {
                for face in &group.mesh.faces {
                    let rings = std::iter::once(&face.vertices)
                        .chain(face.holes.iter())
                        .map(|ring| {
                            ring.iter()
                                .map(|&v| combined.add_vertex(group.mesh.vertices[v].position))
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>();
                    let id = combined
                        .add_face(rings[0].clone(), rings[1..].to_vec())
                        .ok_or_else(|| invalid("invalid reference face"))?;
                    combined.faces[id].attrs = face.attrs.clone();
                }
            }
            scene.groups = vec![ingetrazo_core::groups::Group::with_mesh(
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("COLLADA reference"),
                combined,
            )];
        }
        for group in &mut scene.groups {
            let source = group.mesh.clone();
            let (mut fused, _) = ingetrazo_core::simplify::simplify_mesh(
                &source,
                0.99999_f64.acos().to_degrees(),
                false,
            )
            .map_err(invalid)?;
            // Reference fusion keeps boundary stations; editable simplification
            // removes collinear vertices, so restore them before stitching.
            for vertex in &source.vertices {
                fused.add_vertex(vertex.position);
            }
            group.mesh = ingetrazo_core::planar::stitch_boundaries(&fused).map_err(invalid)?;
            for i in 0..group.mesh.edges.len() {
                let faces = &group.mesh.edges[i].faces;
                let soft = if faces.len() == 2 {
                    let a = &group.mesh.faces[faces[0]].attrs;
                    let b = &group.mesh.faces[faces[1]].attrs;
                    let image = |attrs: &HashMap<String, serde_json::Value>| {
                        attrs.get("texture").and_then(|t| t.get("embed")).cloned()
                    };
                    a.get("color") == b.get("color")
                        && image(a) == image(b)
                        && group
                            .mesh
                            .face_normal(faces[0])
                            .dot(group.mesh.face_normal(faces[1]))
                            .abs()
                            > 0.85
                } else {
                    false
                };
                group.mesh.edges[i].soft = soft;
            }
        }
        let mut fused_definitions: HashMap<uuid::Uuid, ingetrazo_core::mesh::Mesh> = HashMap::new();
        for group in &mut scene.groups {
            if let Some(id) = group.component_definition {
                group.mesh = fused_definitions
                    .entry(id)
                    .or_insert_with(|| group.mesh.clone())
                    .clone();
            }
        }
        for (id, mesh) in fused_definitions {
            scene.component_definitions.insert(
                id,
                ingetrazo_core::components::ComponentDefinition {
                    mesh,
                    children: vec![],
                },
            );
        }
        scene.bump_version();
    }
    Ok(scene)
}
pub(crate) fn scene_from_xml(xml: &str) -> Result<ingetrazo_core::scene::Scene> {
    scene_from_xml_at(xml, None)
}
pub(crate) fn scene_from_xml_at(
    xml: &str,
    base: Option<&std::path::Path>,
) -> Result<ingetrazo_core::scene::Scene> {
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
    if let Some(geo) = asset
        .and_then(|n| child(n, "coverage"))
        .and_then(|n| child(n, "geographic_location"))
    {
        let scalar = |name: &str| {
            child(geo, name)
                .and_then(|n| n.text())
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|n| n.is_finite())
        };
        if let (Some(lat), Some(lon)) = (scalar("latitude"), scalar("longitude")) {
            let alt = scalar("altitude").unwrap_or(0.);
            scene.geo_datum = Some(ingetrazo_core::georef::SceneDatum::new(lat, lon, alt, 0.));
        }
    }

    fn mesh_group(
        node: Node<'_, '_>,
        world: DMat4,
        name: &str,
        colors: &HashMap<String, DaeMaterial>,
    ) -> Result<Group> {
        let mut mesh = Mesh::new();
        for polygon in geometry(node)? {
            let points: Vec<_> = polygon
                .points
                .iter()
                .map(|p| world.transform_point3(*p))
                .collect();
            let vertices: Vec<_> = polygon
                .points
                .into_iter()
                .map(|p| mesh.add_vertex(world.transform_point3(p)))
                .collect();
            // The original COLLADA loader keeps authored loop order after
            // transforms, including reflections; open sheets have no outward side.
            let id = mesh
                .add_face(vertices, vec![])
                .ok_or_else(|| invalid("invalid transformed polygon"))?;
            if let Some(material) = polygon
                .material
                .as_ref()
                .and_then(|symbol| colors.get(symbol))
            {
                if let Some(color) = material
                    .color
                    .filter(|_| material.image.is_none() || polygon.uv.is_none())
                {
                    mesh.faces[id]
                        .attrs
                        .insert("color".into(), serde_json::json!(color));
                }
                if let (Some(image), Some(uv)) = (&material.image, &polygon.uv) {
                    let map = affine_uv(&points, uv)?;
                    let gu = DVec3::new(map[0], map[1], map[2]).length();
                    let gv = DVec3::new(map[4], map[5], map[6]).length();
                    mesh.faces[id].attrs.insert("texture".into(),serde_json::json!({"embed":image,"uvw":map,"sw":if gu>1e-9{1./gu}else{1.},"sh":if gv>1e-9{1./gv}else{1.}}));
                }
                mesh.faces[id]
                    .attrs
                    .insert("back".into(), serde_json::json!(true));
            }
        }
        Ok(Group::with_mesh(name, mesh))
    }
    struct ReferenceInstance {
        start: usize,
        end: usize,
        key: roxmltree::NodeId,
        name: String,
        placement: DMat4,
    }
    fn combine_meshes(groups: &[Group]) -> Result<Mesh> {
        let mut mesh = Mesh::new();
        for group in groups {
            for face in &group.mesh.faces {
                let rings = std::iter::once(&face.vertices)
                    .chain(face.holes.iter())
                    .map(|ring| {
                        ring.iter()
                            .map(|&v| mesh.add_vertex(group.mesh.vertices[v].position))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();
                let id = mesh
                    .add_face(rings[0].clone(), rings[1..].to_vec())
                    .ok_or_else(|| invalid("invalid reference assembly face"))?;
                mesh.faces[id].attrs = face.attrs.clone();
                mesh.faces[id].layer = face.layer.clone();
            }
        }
        Ok(mesh)
    }
    fn walk<'a, 'i>(
        node: Node<'a, 'i>,
        parent: DMat4,
        ids: &HashMap<&'i str, Node<'a, 'i>>,
        stack: &mut Vec<roxmltree::NodeId>,
        scene: &mut Scene,
        base: Option<&std::path::Path>,
        basis: DMat4,
        instances: &mut Vec<ReferenceInstance>,
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
                "node" => walk(el, world, ids, stack, scene, base, basis, instances)?,
                "instance_node" => {
                    let target = el
                        .attribute("url")
                        .filter(|s| s.starts_with('#'))
                        .and_then(|s| ids.get(&s[1..]))
                        .ok_or_else(|| invalid("unresolved instance_node"))?;
                    let start = scene.groups.len();
                    walk(*target, world, ids, stack, scene, base, basis, instances)?;
                    instances.push(ReferenceInstance {
                        start,
                        end: scene.groups.len(),
                        key: target.id(),
                        name: el
                            .attribute("name")
                            .or(el.attribute("id"))
                            .unwrap_or(name)
                            .to_owned(),
                        placement: world * basis.inverse(),
                    });
                }
                "instance_geometry" => {
                    let target = el
                        .attribute("url")
                        .filter(|s| s.starts_with('#'))
                        .and_then(|s| ids.get(&s[1..]))
                        .ok_or_else(|| invalid("unresolved geometry"))?;
                    let mesh =
                        child(*target, "mesh").ok_or_else(|| invalid("geometry has no mesh"))?;
                    let colors = material_colors(el, ids, base, &mut scene.embedded_textures)?;
                    let group = mesh_group(mesh, world, name, &colors)?;
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
    let mut instances = vec![];
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
                    &HashMap::new(),
                )?;
                if !group.mesh.faces.is_empty() {
                    scene.groups.push(group);
                }
            }
        }
    } else {
        for node in nodes {
            walk(
                node,
                world,
                &ids,
                &mut vec![],
                &mut scene,
                base,
                world,
                &mut instances,
            )?;
        }
    }
    if scene.groups.is_empty() {
        return Err(invalid("no geometry"));
    }
    if scene
        .groups
        .iter()
        .map(|g| g.mesh.faces.len())
        .sum::<usize>()
        > 400
        && !instances.is_empty()
    {
        let groups = std::mem::take(&mut scene.groups);
        let mut prototypes: HashMap<roxmltree::NodeId, (uuid::Uuid, Mesh)> = HashMap::new();
        let mut index = 0;
        while index < groups.len() {
            // An outer reused assembly owns its nested geometry as one prototype.
            if let Some(instance) = instances
                .iter()
                .filter(|i| i.start == index && i.end > i.start)
                .max_by_key(|i| i.end)
            {
                if !prototypes.contains_key(&instance.key) {
                    let world_mesh = combine_meshes(&groups[instance.start..instance.end])?;
                    let mesh = ingetrazo_core::containers::transform_mesh(
                        &world_mesh,
                        instance.placement.inverse(),
                    )
                    .map_err(invalid)?;
                    prototypes.insert(instance.key, (uuid::Uuid::new_v4(), mesh));
                }
                let (definition, mesh) = &prototypes[&instance.key];
                let mut group = Group::with_mesh(&instance.name, mesh.clone());
                group.transform = instance.placement;
                group.component_definition = Some(*definition);
                scene.groups.push(group);
                index = instance.end;
            } else {
                scene.groups.push(groups[index].clone());
                index += 1;
            }
        }
        for (_, (id, mesh)) in prototypes {
            scene.component_definitions.insert(
                id,
                ingetrazo_core::components::ComponentDefinition {
                    mesh,
                    children: vec![],
                },
            );
        }
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
    fn textured_scene_assembly_keeps_bytes_and_world_uv_through_reflection() {
        let folder =
            std::env::temp_dir().join(format!("ingetrazo-dae-assembly-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("paint.png"), [1, 2, 3, 255]).unwrap();
        let xml=document().replace("<vertices id=\"v\">",r##"<source id="uv"><float_array count="6">0 0 1 0 0 1</float_array><technique_common><accessor count="3" stride="2"/></technique_common></source><vertices id="v">"##).replace("semantic=\"NORMAL\" offset=\"0\"","semantic=\"TEXCOORD\" offset=\"0\" source=\"#uv\"").replace("<p>0 0 0 1 0 2</p>","<p>0 0 1 1 2 2</p>").replace("<triangles count=\"1\">","<triangles count=\"1\" material=\"paint\">").replace("<library_nodes>",r##"<image id="image"><init_from>paint.png</init_from></image><effect id="fx"><profile_COMMON><technique><lambert><diffuse><texture texture="image"/></diffuse></lambert></technique></profile_COMMON></effect><material id="mat"><instance_effect url="#fx"/></material><library_nodes>"##).replace("<instance_geometry url=\"#g\"/>",r##"<instance_geometry url="#g"><bind_material><technique_common><instance_material symbol="paint" target="#mat"/></technique_common></bind_material></instance_geometry>"##);
        std::fs::write(folder.join("model.dae"), &xml).unwrap();
        let scene = crate::load_file(&folder.join("model.dae")).unwrap();
        let texture = &scene.mesh.faces[0].attrs["texture"];
        assert_eq!(
            scene.embedded_textures[texture["embed"].as_str().unwrap()],
            [1, 2, 3, 255]
        );
        let m: Vec<_> = texture["uvw"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let point = DVec3::new(0.08, -0.30, 0.20);
        assert!((DVec3::new(m[0], m[1], m[2]).dot(point) + m[3] - 1.).abs() < 1e-10);
        assert!(scene_from_xml(&xml).is_err());
        std::fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn geographic_asset_preserves_site_anchor() {
        let xml=document().replace("<unit meter=\"0.01\"/>","<unit meter=\"0.01\"/><coverage><geographic_location><latitude>51.5</latitude><longitude>-0.1</longitude><altitude>32</altitude></geographic_location></coverage>");
        let scene = scene_from_xml(&xml).unwrap();
        let datum = scene.geo_datum.unwrap();
        assert_eq!((datum.lat, datum.lon, datum.alt), (51.5, -0.1, 32.));
        assert_eq!(datum.north, 0.);
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
        assert!(mesh.face_normal(0).y > 0.);
    }
    #[test]
    fn interleaved_texture_coordinates_keep_authored_indices_and_reject_truncation() {
        let xml=document().replace("<vertices id=\"v\">",r##"<source id="uv"><float_array count="9">0 0 9 1 0 9 0 1 9</float_array><technique_common><accessor count="3" stride="3"/></technique_common></source><vertices id="v">"##).replace("semantic=\"NORMAL\" offset=\"0\"","semantic=\"TEXCOORD\" offset=\"0\" source=\"#uv\"").replace("<p>0 0 0 1 0 2</p>","<p>2 0 0 1 1 2</p>");
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let mesh = doc.descendants().find(|n| n.has_tag_name("mesh")).unwrap();
        let polygons = geometry(mesh).unwrap();
        assert_eq!(
            polygons[0].uv.as_ref().unwrap(),
            &vec![glam::DVec2::Y, glam::DVec2::ZERO, glam::DVec2::X]
        );
        let broken = xml.replace("<p>2 0 0 1 1 2</p>", "<p>9 0 0 1 1 2</p>");
        assert!(scene_from_xml(&broken).is_err());
        let truncated = xml.replace("count=\"3\" stride=\"3\"", "count=\"4\" stride=\"3\"");
        assert!(scene_from_xml(&truncated).is_err());
    }
    #[test]
    fn diffuse_effect_is_bound_by_primitive_symbol_not_global_material_order() {
        let xml=document().replace("<library_nodes>",r##"<library_effects><effect id="fx"><profile_COMMON><technique><phong><diffuse><color>0.2 0.4 0.8 1</color></diffuse></phong></technique></profile_COMMON></effect></library_effects><library_materials><material id="blue"><instance_effect url="#fx"/></material></library_materials><library_nodes>"##).replace("<triangles count=\"1\">","<triangles count=\"1\" material=\"paint\">").replace("<instance_geometry url=\"#g\"/>",r##"<instance_geometry url="#g"><bind_material><technique_common><instance_material symbol="paint" target="#blue"/></technique_common></bind_material></instance_geometry>"##);
        let scene = scene_from_xml(&xml).unwrap();
        assert_eq!(
            scene.groups[0].mesh.faces[0].attrs["color"],
            serde_json::json!([0.2, 0.4, 0.8])
        );
        assert_eq!(scene.groups[0].mesh.faces[0].attrs["back"], true);
        assert!(scene_from_xml(&xml.replace("target=\"#blue\"", "target=\"#missing\"")).is_err());
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

#[cfg(test)]
mod uv_affine_tests {
    use super::*;
    use glam::DVec2;
    #[test]
    fn translated_reflected_world_uv_and_non_affine_rejection() {
        let p = [
            DVec3::new(10., 20., 30.),
            DVec3::new(8., 20., 30.),
            DVec3::new(8., 23., 30.),
            DVec3::new(10., 23., 30.),
        ];
        let mut uv = [DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y];
        let m = affine_uv(&p, &uv).unwrap();
        for (p, t) in p.iter().zip(uv) {
            assert!((DVec3::new(m[0], m[1], m[2]).dot(*p) + m[3] - t.x).abs() < 1e-10);
            assert!((DVec3::new(m[4], m[5], m[6]).dot(*p) + m[7] - t.y).abs() < 1e-10);
        }
        uv[3].x = 0.5;
        assert!(affine_uv(&p, &uv).is_err());
        assert!(affine_uv(&[DVec3::ZERO; 3], &[DVec2::ZERO; 3]).is_err());
        assert!(affine_uv(&p, &uv[..3]).is_err());
    }
}

#[cfg(test)]
mod image_binding_tests {
    use super::*;
    #[test]
    fn direct_image_and_scoped_sampler_chains_reject_broken_binding() {
        let xml = r#"<COLLADA><image id="img"><init_from>textures/a.png</init_from></image><effect id="e"><newparam sid="surf"><surface><init_from>img</init_from></surface></newparam><newparam sid="sampler"><sampler2D><source>surf</source></sampler2D></newparam><texture texture="sampler"/><texture texture="img"/><texture texture="missing"/></effect></COLLADA>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let ids: HashMap<_, _> = doc
            .descendants()
            .filter_map(|n| n.attribute("id").map(|id| (id, n)))
            .collect();
        let effect = ids["e"];
        let textures: Vec<_> = effect
            .children()
            .filter(|n| n.tag_name().name() == "texture")
            .collect();
        assert_eq!(
            diffuse_image(effect, textures[0], &ids)
                .unwrap()
                .attribute("id"),
            Some("img")
        );
        assert_eq!(
            diffuse_image(effect, textures[1], &ids)
                .unwrap()
                .attribute("id"),
            Some("img")
        );
        assert!(diffuse_image(effect, textures[2], &ids).is_err());
        let broken = xml.replace(
            "<init_from>img</init_from>",
            "<init_from>noimage</init_from>",
        );
        let doc = roxmltree::Document::parse(&broken).unwrap();
        let ids: HashMap<_, _> = doc
            .descendants()
            .filter_map(|n| n.attribute("id").map(|id| (id, n)))
            .collect();
        let effect = ids["e"];
        let texture = effect
            .children()
            .find(|n| n.tag_name().name() == "texture")
            .unwrap();
        assert!(diffuse_image(effect, texture, &ids).is_err());
    }
}

#[cfg(test)]
mod image_file_tests {
    use super::*;
    #[test]
    fn escaped_local_image_bytes_are_exact_and_remote_and_malformed_uris_refused() {
        let folder =
            std::env::temp_dir().join(format!("ingetrazo-dae-image-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        let payload = [0, 1, 128, 255, 42];
        std::fs::write(folder.join("paint blue.png"), payload).unwrap();
        let xml = "<image><init_from>paint%20blue.png</init_from></image>";
        let doc = roxmltree::Document::parse(xml).unwrap();
        let (key, bytes) = image_bytes(doc.root_element(), &folder).unwrap();
        assert!(key.starts_with("textures/dae-"));
        assert!(key.ends_with(".png"));
        assert_eq!(bytes, payload);
        for uri in [
            "https://example.test/image.png",
            "data:image/png;base64,abc",
            "paint%2.png",
            "paint%GG.png",
            "paint%00.png",
            "missing.png",
        ] {
            let xml = format!("<image><init_from>{uri}</init_from></image>");
            let doc = roxmltree::Document::parse(&xml).unwrap();
            assert!(image_bytes(doc.root_element(), &folder).is_err());
        }
        std::fs::remove_dir_all(folder).unwrap();
    }
}

#[cfg(test)]
mod representative_image_tests {
    use super::*;
    fn color(pixels: [[u8; 4]; 4]) -> [f64; 3] {
        let image =
            image::RgbaImage::from_fn(2, 2, |x, y| image::Rgba(pixels[(y * 2 + x) as usize]));
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut output, image::ImageFormat::Png)
            .unwrap();
        representative_image_color(output.get_ref()).unwrap()
    }
    #[test]
    fn original_qt_opaque_fixed_point_and_alpha_weighted_controls() {
        assert_eq!(
            color([
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255]
            ]),
            [127. / 255.; 3]
        );
        assert_eq!(
            color([[203, 71, 29, 255]; 4]),
            [203. / 255., 71. / 255., 29. / 255.]
        );
        assert_eq!(
            color([
                [255, 0, 0, 255],
                [0, 255, 0, 128],
                [0, 0, 255, 64],
                [255, 255, 255, 0]
            ]),
            [145. / 255., 73. / 255., 37. / 255.]
        );
    }
}

#[cfg(test)]
mod reference_threshold_tests {
    use super::*;
    #[test]
    fn original_editable_threshold_and_reference_boundary_stations() {
        let dir = std::env::temp_dir().join(format!("dae-reference-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, xml, editable, area, edges) in [
            (
                "threshold400",
                include_str!("../../../docs/audits/dae-reference-corpus/threshold400.dae"),
                true,
                200.,
                60,
            ),
            (
                "threshold401",
                include_str!("../../../docs/audits/dae-reference-corpus/threshold401.dae"),
                false,
                200.5,
                63,
            ),
            (
                "bare512",
                include_str!("../../../docs/audits/dae-reference-corpus/bare512.dae"),
                false,
                256.,
                64,
            ),
        ] {
            let path = dir.join(format!("{name}.dae"));
            std::fs::write(&path, xml).unwrap();
            let scene = load_dae(&path).unwrap();
            let mesh = if editable {
                assert!(scene.groups.is_empty());
                &scene.mesh
            } else {
                assert!(scene.mesh.faces.is_empty());
                assert_eq!(scene.groups.len(), 1);
                if name == "bare512" {
                    assert_eq!(scene.groups[0].name, name);
                }
                &scene.groups[0].mesh
            };
            assert!(
                (mesh
                    .faces
                    .iter()
                    .enumerate()
                    .map(|(i, _)| mesh.face_area(i))
                    .sum::<f64>()
                    - area)
                    .abs()
                    < 1e-8
            );
            assert_eq!(mesh.faces.len(), if name == "threshold401" { 2 } else { 1 });
            assert_eq!(mesh.edges.len(), edges);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
