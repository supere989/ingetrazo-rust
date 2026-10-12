//! Camera-facing render-export figures, source formats.meshexport law.
//! Cold export only: the persistent Blender worker owns later camera turns.
use super::GltfError;
use glam::{DVec3, Mat4, Vec3, Vec4};
use ingetrazo_core::{camera::OrbitCamera, groups::Group, mesh::Mesh};
use serde_json::{json, Value};
fn error(s: &str) -> GltfError {
    GltfError::Invalid(s.into())
}
pub(super) struct View {
    eye: Vec3,
    target: Vec3,
    perspective: bool,
}
impl View {
    pub fn from_camera(camera: &OrbitCamera) -> Result<Self, GltfError> {
        ingetrazo_core::render_blender::validate_camera(camera).map_err(|e| error(&e))?;
        let data = ingetrazo_core::render_blender::camera_dict(camera);
        let vector = |key: &str| {
            Vec3::from_array(std::array::from_fn(|i| {
                data[key][i].as_f64().expect("validated camera") as f32
            }))
        };
        Ok(Self {
            eye: vector("eye"),
            target: vector("target"),
            perspective: camera.perspective,
        })
    }
    fn toward(&self, anchor: Vec3) -> Vec3 {
        if self.perspective {
            self.eye - anchor
        } else {
            self.eye - self.target
        }
    }
}
pub(super) struct Figure {
    pub mesh: Mesh,
    pub extra: Value,
}
fn qt_length(v: Vec3) -> f32 {
    let scale = v.abs().max_element();
    if scale == 0. {
        0.
    } else {
        let r = v / scale;
        scale * (r.x * r.x + r.y * r.y + r.z * r.z).sqrt()
    }
}
fn qt_normalized(v: Vec3) -> Vec3 {
    let length = qt_length(v);
    if (length - 1.).abs() <= 1e-5 {
        v
    } else if length.abs() <= 1e-5 {
        Vec3::ZERO
    } else {
        v / length
    }
}
fn newell(points: &[DVec3]) -> Vec3 {
    let mut n = DVec3::ZERO;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    n.as_vec3()
}
fn bounds(mesh: &Mesh) -> Option<(Vec3, Vec3, Vec3)> {
    if mesh.vertices.is_empty() {
        return None;
    }
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        let p = v.position.as_vec3();
        lo = lo.min(p);
        hi = hi.max(p);
    }
    Some((
        lo,
        hi,
        Vec3::new(
            ((lo.x as f64 + hi.x as f64) / 2.) as f32,
            ((lo.y as f64 + hi.y as f64) / 2.) as f32,
            lo.z,
        ),
    ))
}
fn remap_attrs(mesh: &mut Mesh, m: Mat4) -> Result<(), GltfError> {
    let inverse = m.as_dmat4().inverse().as_mat4().as_dmat4();
    if !inverse.is_finite() {
        return Err(error("Nonfinite figure inverse"));
    }
    let t = m.w_axis.truncate().as_dvec3();
    for face in &mut mesh.faces {
        if let Some(texture) = face.attrs.get_mut("texture") {
            if let Some(values) = texture
                .get("uvw")
                .and_then(Value::as_array)
                .filter(|a| a.len() == 8)
                .and_then(|a| a.iter().map(Value::as_f64).collect::<Option<Vec<_>>>())
            {
                let mut out = values.clone();
                for base in [0, 4] {
                    let g = DVec3::new(values[base], values[base + 1], values[base + 2]);
                    let columns = inverse.to_cols_array_2d();
                    let mapped = DVec3::from_array(std::array::from_fn(|i| {
                        columns[i][0] * g.x + columns[i][1] * g.y + columns[i][2] * g.z
                    }));
                    out[base] = mapped.x;
                    out[base + 1] = mapped.y;
                    out[base + 2] = mapped.z;
                    out[base + 3] =
                        values[base + 3] - (mapped.x * t.x + mapped.y * t.y + mapped.z * t.z);
                }
                if !out.iter().all(|v| v.is_finite()) {
                    return Err(error("Nonfinite figure UVW"));
                }
                texture["uvw"] = json!(out);
            }
        }
    }
    Ok(())
}
// Export does not mutate the document. Reject the source reconstruction case
// where mapped rows weld; a moved-index mesh would otherwise emit stale rings.
fn preserve_mapped_topology(mesh: &Mesh) -> Result<(), GltfError> {
    let mut cells =
        std::collections::HashMap::<[u64; 3], usize>::with_capacity(mesh.vertices.len());
    let bits = |v: f64| if v == 0. { 0 } else { v.to_bits() };
    for (i, v) in mesh.vertices.iter().enumerate() {
        let p = v.position.as_vec3();
        let k = p
            .to_array()
            .map(|x| (f64::from(x) * 10000.).round_ties_even());
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let key = [
                        bits(k[0] + f64::from(dx)),
                        bits(k[1] + f64::from(dy)),
                        bits(k[2] + f64::from(dz)),
                    ];
                    if let Some(&other) = cells.get(&key) {
                        let d = p - mesh.vertices[other].position.as_vec3();
                        if (dx == 0 && dy == 0 && dz == 0)
                            || d.x * d.x + d.y * d.y + d.z * d.z < 1e-8
                        {
                            return Err(error("Figure transform changes welded topology"));
                        }
                    }
                }
            }
        }
        cells.insert(k.map(bits), i);
    }
    Ok(())
}
fn apply_transform(mesh: &mut Mesh, m: Mat4) -> Result<(), GltfError> {
    let bulk = mesh.edges.len() * 2 + mesh.faces.len() * 4 >= 1024;
    for v in &mut mesh.vertices {
        let p = v.position.as_vec3();
        v.position = if bulk {
            m.as_dmat4()
                .transform_point3(p.as_dvec3())
                .as_vec3()
                .as_dvec3()
        } else {
            m.transform_point3(p).as_dvec3()
        };
        if !v.position.is_finite() {
            return Err(error("Nonfinite figure geometry"));
        }
    }
    preserve_mapped_topology(mesh)?;
    remap_attrs(mesh, m)?;
    mesh.rebuild_spatial_index();
    Ok(())
}
fn world_mesh(src: &Mesh, m: Mat4, placed: bool) -> Result<Mesh, GltfError> {
    if !m.is_finite()
        || src
            .vertices
            .iter()
            .any(|v| !v.position.as_vec3().is_finite())
        || src.faces.iter().any(|f| {
            f.vertices
                .iter()
                .chain(f.holes.iter().flatten())
                .any(|&v| v >= src.vertices.len())
        })
        || src
            .edges
            .iter()
            .any(|e| e.v0 >= src.vertices.len() || e.v1 >= src.vertices.len())
    {
        return Err(error("Invalid figure placement or references"));
    }
    let mut mesh = src.clone();
    if placed {
        // The source's transformed_mesh copies faces and edges, not orphan rows.
        let mut used = vec![false; mesh.vertices.len()];
        for f in &mesh.faces {
            for &v in f.vertices.iter().chain(f.holes.iter().flatten()) {
                used[v] = true;
            }
        }
        for e in &mesh.edges {
            used[e.v0] = true;
            used[e.v1] = true;
        }
        let mut remap = vec![usize::MAX; used.len()];
        let mut next = 0;
        for (i, &keep) in used.iter().enumerate() {
            if keep {
                remap[i] = next;
                next += 1;
            }
        }
        let mut i = 0;
        mesh.vertices.retain(|_| {
            let keep = used[i];
            i += 1;
            keep
        });
        for f in &mut mesh.faces {
            for v in f.vertices.iter_mut().chain(f.holes.iter_mut().flatten()) {
                *v = remap[*v];
            }
        }
        for e in &mut mesh.edges {
            e.v0 = remap[e.v0];
            e.v1 = remap[e.v1];
        }
        apply_transform(&mut mesh, m)?;
    } else {
        for v in &mut mesh.vertices {
            v.position = v.position.as_vec3().as_dvec3();
        }
        mesh.rebuild_spatial_index();
    }
    Ok(mesh)
}

fn turn(mut mesh: Mesh, view: &View) -> Result<Mesh, GltfError> {
    let Some((_, _, anchor)) = bounds(&mesh) else {
        return Ok(mesh);
    };
    let mut largest = None;
    let mut area = -1.;
    for (i, f) in mesh.faces.iter().enumerate() {
        let pts = f
            .vertices
            .iter()
            .map(|&v| mesh.vertices[v].position)
            .collect::<Vec<_>>();
        let n = newell(&pts);
        let a = 0.5 * qt_length(n) as f64;
        if a > area {
            area = a;
            largest = Some((i, n));
        }
    }
    let Some((_, n)) = largest else {
        return Ok(mesh);
    };
    let len = qt_length(n);
    let n = if len < 1e-9 { Vec3::Z } else { n / len };
    let normal_length = (n.x as f64).hypot(n.y as f64);
    if normal_length < 1e-9 {
        return Ok(mesh);
    }
    let d = view.toward(anchor);
    let toward_length = (d.x as f64).hypot(d.y as f64);
    if toward_length < 1e-9 {
        return Ok(mesh);
    }
    let nx = n.x as f64 / normal_length;
    let ny = n.y as f64 / normal_length;
    let dx = d.x as f64 / toward_length;
    let dy = d.y as f64 / toward_length;
    let angle = (nx * dy - ny * dx).atan2(nx * dx + ny * dy).to_degrees() as f32;
    let (c, s) = if angle == 90. || angle == -270. {
        (0., 1.)
    } else if angle == -90. || angle == 270. {
        (0., -1.)
    } else if angle == 180. || angle == -180. {
        (-1., 0.)
    } else {
        let a = angle * (std::f64::consts::PI / 180.) as f32;
        (a.cos(), a.sin())
    };
    let mut m = Mat4::from_cols(
        Vec4::new(c, s, 0., 0.),
        Vec4::new(-s, c, 0., 0.),
        Vec4::Z,
        anchor.extend(1.),
    );
    let neg = -anchor;
    m.w_axis.x = m.x_axis.x * neg.x + m.y_axis.x * neg.y + m.z_axis.x * neg.z + m.w_axis.x;
    m.w_axis.y = m.x_axis.y * neg.x + m.y_axis.y * neg.y + m.z_axis.y * neg.z + m.w_axis.y;
    m.w_axis.z = m.x_axis.z * neg.x + m.y_axis.z * neg.y + m.z_axis.z * neg.z + m.w_axis.z;
    apply_transform(&mut mesh, m)?;
    Ok(mesh)
}
fn affine(corners: &[Vec3; 4]) -> Option<[f64; 8]> {
    let p = corners.map(|p| p.as_dvec3());
    let delta = [p[1] - p[0], p[2] - p[0], p[3] - p[0]];
    let mut best = 1e-24;
    let mut pair = None;
    for i in 0..3 {
        for j in i + 1..3 {
            let a = delta[i];
            let b = delta[j];
            let cross = DVec3::new(
                a.y * b.z - a.z * b.y,
                a.z * b.x - a.x * b.z,
                a.x * b.y - a.y * b.x,
            );
            let c2 = cross.x * cross.x + cross.y * cross.y + cross.z * cross.z;
            if c2 > best {
                best = c2;
                pair = Some((i, j));
            }
        }
    }
    let (i, j) = pair?;
    let e1 = delta[i];
    let e2 = delta[j];
    let dot = |a: DVec3, b: DVec3| a.x * b.x + a.y * b.y + a.z * b.z;
    let g11 = dot(e1, e1);
    let g12 = dot(e1, e2);
    let g22 = dot(e2, e2);
    let det = g11 * g22 - g12 * g12;
    if det.abs() < 1e-18 {
        return None;
    }
    let uv = [[0., 0.], [1., 0.], [1., 1.], [0., 1.]];
    let mut out = [0.; 8];
    for k in 0..2 {
        let d1 = uv[i + 1][k] - uv[0][k];
        let d2 = uv[j + 1][k] - uv[0][k];
        let a = (d1 * g22 - d2 * g12) / det;
        let b = (d2 * g11 - d1 * g12) / det;
        let g = e1 * a + e2 * b;
        out[k * 4] = g.x;
        out[k * 4 + 1] = g.y;
        out[k * 4 + 2] = g.z;
        out[k * 4 + 3] = uv[0][k] - (g.x * p[0].x + g.y * p[0].y + g.z * p[0].z);
    }
    Some(out)
}
fn quad(mesh: Mesh, view: &View, scene: &ingetrazo_core::scene::Scene) -> Result<Mesh, GltfError> {
    let texture = mesh
        .faces
        .iter()
        .filter_map(|f| f.attrs.get("texture"))
        .find(|t| {
            t.get("path").and_then(Value::as_str).is_some_and(|s| !s.is_empty())
                // The native IGZ loader registers embedded source images instead
                // of extracting them to the original loader's temporary paths.
                || t.get("embed").and_then(Value::as_str).is_some_and(|s| scene.embedded_textures.contains_key(s))
        })
        .cloned();
    let Some(mut texture) = texture else {
        return turn(mesh, view);
    };
    let Some((lo, hi, anchor)) = bounds(&mesh) else {
        return turn(mesh, view);
    };
    let width = (hi.x as f64 - lo.x as f64).hypot(hi.y as f64 - lo.y as f64);
    let height = hi.z as f64 - lo.z as f64;
    let toward = view.toward(anchor);
    let toward = Vec3::new(toward.x, toward.y, 0.);
    if width < 1e-9 || height < 1e-9 || qt_length(toward) < 1e-6 {
        return Ok(mesh);
    }
    let d = qt_normalized(toward);
    let r = Vec3::new(-d.y, d.x, 0.);
    let c0 = anchor - r * (width / 2.) as f32;
    let c1 = anchor + r * (width / 2.) as f32;
    let corners = [
        c0,
        c1,
        c1 + Vec3::Z * height as f32,
        c0 + Vec3::Z * height as f32,
    ];
    texture["uvw"] = affine(&corners).map_or(Value::Null, |v| json!(v));
    let mut out = Mesh::new();
    let vertices = corners
        .iter()
        .map(|p| out.add_vertex(p.as_dvec3()))
        .collect();
    let face = out
        .add_face(vertices, Vec::new())
        .ok_or_else(|| error("Figure quad admission failed"))?;
    out.faces[face].attrs = mesh.faces[0]
        .attrs
        .iter()
        .filter(|(k, _)| k.as_str() != "texture")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    out.faces[face].attrs.insert("texture".into(), texture);
    Ok(out)
}
pub(super) fn prepare(
    scene: &ingetrazo_core::scene::Scene,
    g: &Group,
    world: Mat4,
    placed: bool,
    view: &View,
) -> Result<Option<Figure>, GltfError> {
    let mesh = world_mesh(&g.mesh, world, placed)?;
    let Some((_, _, feet)) = bounds(&mesh) else {
        return Ok(None);
    };
    let toward = view.toward(feet);
    let yaw = (toward.y as f64).atan2(toward.x as f64);
    let kind = g.native_document.get("billboard").and_then(Value::as_str);
    let mesh = if kind == Some("mesh") {
        turn(mesh, view)?
    } else {
        quad(mesh, view, scene)?
    };
    Ok(Some(Figure {
        mesh,
        extra: json!([feet.x, feet.y, feet.z, yaw]),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_figure_refuses_before_writer_and_preserves_scene() {
        let mut scene = ingetrazo_core::scene::Scene::new();
        let mut mesh = Mesh::new();
        let ids = [Vec3::ZERO, Vec3::X, Vec3::Z].map(|p| mesh.add_vertex(p.as_dvec3()));
        mesh.add_face(ids.to_vec(), Vec::new()).unwrap();
        let mut group = Group::new("figure");
        group.mesh = mesh;
        group.billboard = true;
        group.mesh.faces[0].vertices[0] = usize::MAX;
        scene.groups.push(group);
        let old = scene.clone();
        let mut output = b"sentinel".to_vec();
        assert!(
            crate::gltf::export_glb_with_camera(&scene, &OrbitCamera::default(), &mut output)
                .is_err()
        );
        assert_eq!(output, b"sentinel");
        assert_eq!(scene, old);
    }
    #[test]
    fn source_weld_changing_transform_is_explicit_refusal() {
        let mut mesh = Mesh::new();
        let ids = [Vec3::ZERO, Vec3::X, Vec3::Z].map(|p| mesh.add_vertex(p.as_dvec3()));
        mesh.add_face(ids.to_vec(), Vec::new()).unwrap();
        assert!(apply_transform(&mut mesh, Mat4::from_scale(Vec3::new(0., 1., 1.))).is_err());
    }
    #[test]
    fn invalid_camera_refuses_before_writer() {
        let scene = ingetrazo_core::scene::Scene::new();
        let mut camera = OrbitCamera::default();
        camera.target.x = f64::NAN;
        let mut output = b"sentinel".to_vec();
        assert!(crate::gltf::export_glb_with_camera(&scene, &camera, &mut output).is_err());
        assert_eq!(output, b"sentinel");
    }
}
