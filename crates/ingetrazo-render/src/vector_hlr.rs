//! Exact parallel edge visibility by projected triangle intervals.
//! Raster viewport depth sorting is separate from this paper/vector pass.
use glam::{DMat4, DVec2, DVec3};
use ingetrazo_core::{camera::OrbitCamera, groups::Group, mesh::Mesh, scene::Scene};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Edge,
    Profile,
    Cut,
    Hidden,
}
#[derive(Clone, Debug)]
pub struct VectorSegment {
    pub a: DVec3,
    pub b: DVec3,
    pub kind: LineKind,
}
/// Paper-only precision boundary of the pinned original Qt HLR camera.
/// This does not alter the Scene, viewport projection, or general f64 HLR API.
/// Offset construction and target addition are separate QVector3D f32 boundaries;
/// camera_basis then performs its cross products and normalization in f64.
#[derive(Clone, Copy, Debug)]
pub struct OriginalQtPaperBasis {
    pub eye: DVec3,
    pub right: DVec3,
    pub up: DVec3,
    pub forward: DVec3,
}
impl OriginalQtPaperBasis {
    pub fn from_camera(camera: &OrbitCamera) -> Self {
        let target = camera.target.as_vec3();
        let offset = DVec3::new(
            camera.distance * camera.pitch.cos() * camera.yaw.cos(),
            camera.distance * camera.pitch.cos() * camera.yaw.sin(),
            camera.distance * camera.pitch.sin(),
        )
        .as_vec3();
        let eye = (target + offset).as_dvec3();
        let delta = target.as_dvec3() - eye;
        let forward = delta / delta.length().max(1e-12);
        let qt_forward = (target - (target + offset)).normalize_or_zero();
        let mut up = camera.up.as_vec3();
        if qt_forward.dot(up).abs() >= 0.9999 {
            let sign = if camera.pitch > 0. { -1. } else { 1. };
            up = DVec3::new(sign * camera.yaw.cos(), sign * camera.yaw.sin(), 0.).as_vec3();
        }
        let right = forward.cross(up.as_dvec3());
        let right = right / right.length().max(1e-12);
        let up = right.cross(forward);
        Self {
            eye,
            right,
            up,
            forward,
        }
    }
    pub fn project(&self, point: DVec3) -> DVec3 {
        let delta = point - self.eye;
        DVec3::new(
            delta.dot(self.right),
            delta.dot(self.up),
            delta.dot(self.forward),
        )
    }
}
trait PaperProjection { fn project_point(&self, p: DVec3) -> DVec3; }
impl PaperProjection for OrbitCamera {
    fn project_point(&self, p: DVec3) -> DVec3 {
        let p = p-self.eye();
        DVec3::new(p.dot(self.right()),p.dot(self.right().cross(self.forward())),p.dot(self.forward()))
    }
}
impl PaperProjection for OriginalQtPaperBasis {
    fn project_point(&self,p:DVec3)->DVec3 { self.project(p) }
}
fn project(p:DVec3,camera:&impl PaperProjection)->DVec3 { camera.project_point(p) }
fn gather_edges(scene: &Scene, camera: &OrbitCamera) -> Vec<VectorSegment> {
    fn mesh(
        out: &mut Vec<VectorSegment>,
        mesh: &Mesh,
        transform: DMat4,
        scene: &Scene,
        camera: &OrbitCamera,
    ) {
        for (id, edge) in mesh.edges.iter().enumerate() {
            if !scene.mesh_edge_visible(mesh, id) {
                continue;
            }
            let kind = if edge.soft {
                let faces = edge
                    .faces
                    .iter()
                    .copied()
                    .filter(|&id| scene.mesh_face_visible(mesh, id))
                    .collect::<Vec<_>>();
                if faces.is_empty() {
                    continue;
                }
                let normal = |id| {
                    transform
                        .inverse()
                        .transpose()
                        .transform_vector3(mesh.face_normal(id))
                };
                if faces.len() > 1
                    && (normal(faces[0]).dot(camera.forward()) < 0.)
                        == (normal(faces[1]).dot(camera.forward()) < 0.)
                {
                    continue;
                }
                LineKind::Profile
            } else {
                LineKind::Edge
            };
            let a = transform.transform_point3(mesh.vertices[edge.v0].position);
            let b = transform.transform_point3(mesh.vertices[edge.v1].position);
            if let Some((a, b)) = scene.section_segment(a, b) {
                out.push(VectorSegment { a, b, kind });
            }
        }
    }
    fn groups(
        out: &mut Vec<VectorSegment>,
        items: &[Group],
        parent: DMat4,
        scene: &Scene,
        camera: &OrbitCamera,
    ) {
        for item in items {
            if item.hidden || !scene.is_layer_visible(item.layer.as_deref()) {
                continue;
            }
            let transform = parent * item.transform;
            mesh(out, &item.mesh, transform, scene, camera);
            groups(out, &item.children, transform, scene, camera);
        }
    }
    let mut result = vec![];
    mesh(&mut result, &scene.mesh, DMat4::IDENTITY, scene, camera);
    groups(&mut result, &scene.groups, DMat4::IDENTITY, scene, camera);
    result
}
fn clip_linear(lo: &mut f64, hi: &mut f64, a: f64, b: f64) -> bool {
    if a >= 0. && b >= 0. {
        return true;
    }
    if a < 0. && b < 0. {
        return false;
    }
    let t = a / (a - b);
    if a < 0. {
        *lo = lo.max(t);
    } else {
        *hi = hi.min(t);
    }
    *lo < *hi
}
pub(crate) fn occluded_interval(a: DVec3, b: DVec3, triangle: [DVec3; 3], epsilon: f64) -> Option<(f64, f64)> {
    let p = triangle.map(|p| p.truncate());
    let cross = |a: DVec2, b: DVec2| a.x * b.y - a.y * b.x;
    let area = cross(p[1] - p[0], p[2] - p[0]);
    if area.abs() < 1e-15 {
        return None;
    }
    let bary = |q: DVec2| {
        [
            cross(p[1] - q, p[2] - q) / area,
            cross(p[2] - q, p[0] - q) / area,
            cross(p[0] - q, p[1] - q) / area,
        ]
    };
    let wa = bary(a.truncate());
    let wb = bary(b.truncate());
    let mut lo: f64 = 0.;
    let mut hi: f64 = 1.;
    for i in 0..3 {
        if !clip_linear(&mut lo, &mut hi, wa[i] + 1e-12, wb[i] + 1e-12) {
            return None;
        }
    }
    let za = wa[0] * triangle[0].z + wa[1] * triangle[1].z + wa[2] * triangle[2].z;
    let zb = wb[0] * triangle[0].z + wb[1] * triangle[1].z + wb[2] * triangle[2].z;
    if !clip_linear(&mut lo, &mut hi, a.z - za - epsilon, b.z - zb - epsilon) {
        return None;
    }
    if hi - lo > 1e-10 {
        Some((lo, hi))
    } else {
        None
    }
}
fn cap_triangles(
    segments: &[(DVec3, DVec3)],
    plane: &ingetrazo_core::section::SectionPlane,
) -> Vec<[DVec3; 3]> {
    // Original paper caps use even-odd rings, independent of skin winding.
    // Coincident triangle chords describe one boundary, rather than two rings.
    let mut unique: Vec<(DVec3, DVec3)> = vec![];
    for &(a, b) in segments {
        if !unique.iter().any(|&(x, y)| {
            ((a - x).length() < 1e-7 && (b - y).length() < 1e-7)
                || ((a - y).length() < 1e-7 && (b - x).length() < 1e-7)
        }) {
            unique.push((a, b));
        }
    }
    let segments = &unique[..];
    let mut wire = Mesh::new();
    for &(a, b) in segments {
        let a = wire.add_vertex(a);
        let b = wire.add_vertex(b);
        wire.add_edge(a, b);
    }
    let Ok(regions) = ingetrazo_core::planar::rebuild_faces(&wire, &[]) else {
        return vec![];
    };
    let (u, v) = crate::pipeline::projection_basis(plane.normal);
    let mut result = vec![];
    for id in 0..regions.faces.len() {
        let triangles = regions.triangulate_face(id);
        // A convex fan may begin with a collinear boundary chord. Probe a
        // nondegenerate triangle, never a point on the cut boundary itself.
        let Some(&(a, b, c)) = triangles.iter().find(|&&(a,b,c)| (b-a).cross(c-a)!=DVec3::ZERO) else {
            continue;
        };
        let p = (a + b + c) / 3.;
        let px = p.dot(u);
        let py = p.dot(v);
        let mut winding: i32 = 0;
        for &(a, b) in segments {
            let ax = a.dot(u);
            let ay = a.dot(v);
            let bx = b.dot(u);
            let by = b.dot(v);
            if (ay <= py && by > py) || (by <= py && ay > py) {
                let x = ax + (bx - ax) * (py - ay) / (by - ay);
                if x > px {
                    winding += if by > ay { 1 } else { -1 };
                }
            }
        }
        if winding.abs() % 2 != 0 {
            result.extend(triangles.into_iter().map(|(a, b, c)| [a, b, c]));
        }
    }
    result
}
fn bary(q: DVec2, t: [DVec3; 3]) -> Option<[f64; 3]> {
    let p = t.map(|p| p.truncate());
    let cross = |a: DVec2, b: DVec2| a.x * b.y - a.y * b.x;
    let area = cross(p[1] - p[0], p[2] - p[0]);
    if area.abs() < 1e-18 {
        return None;
    }
    Some([
        cross(p[1] - q, p[2] - q) / area,
        cross(p[2] - q, p[0] - q) / area,
        cross(p[0] - q, p[1] - q) / area,
    ])
}
fn surface_at(p: DVec2, depth: f64, triangles: &[[DVec3; 3]], index:&crate::triangle_index::TriangleIndex, tolerance: f64) -> bool {
    let mut candidates=vec![];index.query(p,p,&mut candidates);
    candidates.into_iter().any(|id| {let t=triangles[id];
        bary(p, t).is_some_and(|w| {
            w.iter().all(|&v| v >= -1e-9)
                && w[0] * t[0].z + w[1] * t[1].z + w[2] * t[2].z <= depth + tolerance
        })
    })
}
fn cap_boundary(caps: &[[DVec3; 3]], camera: &impl PaperProjection) -> Vec<(DVec3, DVec3)> {
    let mut mesh = Mesh::new();
    for tri in caps {
        let ids = tri.map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]);
    }
    mesh.edges
        .iter()
        .filter(|e| e.faces.len() == 1)
        .map(|e| {
            (
                project(mesh.vertices[e.v0].position, camera),
                project(mesh.vertices[e.v1].position, camera),
            )
        })
        .collect()
}
fn cap_intervals(
    a: DVec3,
    b: DVec3,
    boundary: &[(DVec3, DVec3)],
    plane: Option<[DVec3; 3]>,
    eps: f64,
) -> Vec<(f64, f64)> {
    let Some(triangle) = plane else {
        return vec![];
    };
    let Some(wa) = bary(a.truncate(), triangle) else {
        return vec![];
    };
    let Some(wb) = bary(b.truncate(), triangle) else {
        return vec![];
    };
    let za = wa[0] * triangle[0].z + wa[1] * triangle[1].z + wa[2] * triangle[2].z;
    let zb = wb[0] * triangle[0].z + wb[1] * triangle[1].z + wb[2] * triangle[2].z;
    let mut lo = 0.;
    let mut hi = 1.;
    if !clip_linear(&mut lo, &mut hi, a.z - za - eps, b.z - zb - eps) {
        return vec![];
    }
    let mut cuts = vec![lo, hi];
    let d = (b - a).truncate();
    let cross = |a: DVec2, b: DVec2| a.x * b.y - a.y * b.x;
    for &(p, q) in boundary {
        let e = (q - p).truncate();
        let w = (p - a).truncate();
        let den = cross(d, e);
        if den.abs() < 1e-15 {
            continue;
        }
        let t = cross(w, e) / den;
        let u = cross(w, d) / den;
        if (0. ..=1.).contains(&u) && t > lo && t < hi {
            cuts.push(t);
        }
    }
    cuts.sort_by(f64::total_cmp);
    let mut result = vec![];
    for pair in cuts.windows(2) {
        let (t0, t1) = (pair[0], pair[1]);
        if t1 - t0 < 1e-10 {
            continue;
        }
        let p = a.lerp(b, (t0 + t1) / 2.);
        let crossings = boundary
            .iter()
            .filter(|&&(a, b)| {
                if (a.y > p.y) == (b.y > p.y) {
                    return false;
                }
                let x = a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y);
                x > p.x
            })
            .count();
        if crossings % 2 == 1 {
            result.push((t0, t1));
        }
    }
    result
}
fn complement(mut intervals: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut result = vec![];
    let mut start: f64 = 0.;
    for (lo, hi) in intervals {
        if lo > start + 1e-10 {
            result.push((start, lo));
        }
        start = start.max(hi);
    }
    if start < 1. - 1e-10 {
        result.push((start, 1.));
    }
    result
}
/// Return only visible portions; split edges remain referenced to world coordinates.
// Remove coincident dashed ink under any solid ink, then deduplicate
// projected hidden lines even if the corresponding world edges differ in depth.
fn drop_hidden_under_visible(lines:Vec<VectorSegment>,camera:&impl PaperProjection,tol:f64)->Vec<VectorSegment> {
    let mut result=lines.iter().filter(|e|e.kind!=LineKind::Hidden).cloned().collect::<Vec<_>>();
    // Retain exact source projection arithmetic once per accepted segment.
    let mut projected=result.iter().map(|e|(project(e.a,camera).truncate(),project(e.b,camera).truncate())).collect::<Vec<_>>();
    for line in lines.iter().filter(|e|e.kind==LineKind::Hidden) {
        let a=project(line.a,camera).truncate(); let b=project(line.b,camera).truncate();
        let d=b-a; let length=d.length(); if length<tol {continue;}
        let direction=d/length;
        let mut blocked=vec![];
        for &(other_a,other_b) in &projected {
            let p=other_a-a;
            let q=other_b-a;
            if direction.perp_dot(p).abs()>tol || direction.perp_dot(q).abs()>tol {continue;}
            let x=p.dot(direction)/length; let y=q.dot(direction)/length;
            let lo=x.min(y).max(0.); let hi=x.max(y).min(1.);
            if hi>lo {blocked.push((lo,hi));}
        }
        for (lo,hi) in complement(blocked) {
            if (hi-lo)*length>tol {let segment=VectorSegment{a:line.a.lerp(line.b,lo),b:line.a.lerp(line.b,hi),kind:LineKind::Hidden};projected.push((project(segment.a,camera).truncate(),project(segment.b,camera).truncate()));result.push(segment);}
        }
    }
    result
}
#[cfg(test)]
fn exhaustive_hidden_ink_reference(lines:Vec<VectorSegment>,camera:&impl PaperProjection,tol:f64)->Vec<VectorSegment> {
    let mut result=lines.iter().filter(|e|e.kind!=LineKind::Hidden).cloned().collect::<Vec<_>>();
    for line in lines.iter().filter(|e|e.kind==LineKind::Hidden) {
        let a=project(line.a,camera).truncate(); let b=project(line.b,camera).truncate();
        let d=b-a; let length=d.length(); if length<tol {continue;}
        let direction=d/length;
        let mut blocked=vec![];
        for other in &result {
            let p=project(other.a,camera).truncate()-a;
            let q=project(other.b,camera).truncate()-a;
            if direction.perp_dot(p).abs()>tol || direction.perp_dot(q).abs()>tol {continue;}
            let x=p.dot(direction)/length; let y=q.dot(direction)/length;
            let lo=x.min(y).max(0.); let hi=x.max(y).min(1.);
            if hi>lo {blocked.push((lo,hi));}
        }
        for (lo,hi) in complement(blocked) {
            if (hi-lo)*length>tol {result.push(VectorSegment{a:line.a.lerp(line.b,lo),b:line.a.lerp(line.b,hi),kind:LineKind::Hidden});}
        }
    }
    result
}
// Original hlr._merge_collinear joins only world-collinear, end-to-end ink
// of the same class. Projected coincident lines at different depths stay distinct.
fn merge_collinear(lines: Vec<VectorSegment>, tolerance: f64) -> Vec<VectorSegment> {
    // Conservative endpoint broad phase; final predicates and source order stay exact.
    let indexed = tolerance.is_finite() && tolerance >= 0. && lines.iter().all(|l| l.a.is_finite() && l.b.is_finite());
    let mut endpoints = if indexed { lines.iter().enumerate().flat_map(|(i,l)| [(l.a.x,i),(l.b.x,i)]).collect::<Vec<_>>() } else { vec![] };
    endpoints.sort_by(|a,b| a.0.total_cmp(&b.0));
    let mut used = vec![false; lines.len()];
    let mut result = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if used[i] { continue; }
        used[i] = true;
        let direction = (line.b-line.a).normalize_or_zero();
        let mut ends = [line.a, line.b];
        for end in 0..2 {
            let forward = if end == 0 { -direction } else { direction };
            loop {
                let mut next = None;
                let candidates = if indexed {
                    let x = ends[end].x;
                    let pad = (x.abs() + tolerance).max(1.) * f64::EPSILON * 8.;
                    let low = x - tolerance - pad; let high = x + tolerance + pad;
                    let begin = endpoints.partition_point(|p| p.0 < low);
                    let finish = endpoints.partition_point(|p| p.0 <= high);
                    let mut ids = endpoints[begin..finish].iter().map(|p|p.1).collect::<Vec<_>>();
                    ids.sort_unstable(); ids.dedup(); ids
                } else { (0..lines.len()).collect() };
                for j in candidates {
                    let candidate = &lines[j];
                    if used[j] || candidate.kind != line.kind { continue; }
                    for (start, other) in [(candidate.a,candidate.b),(candidate.b,candidate.a)] {
                        let delta = other-start;
                        if (start-ends[end]).abs().max_element() <= tolerance
                            && forward.cross(delta.normalize_or_zero()).length_squared() <= 1e-14
                            && forward.dot(delta) > 0. {
                            next = Some((j,other)); break;
                        }
                    }
                    if next.is_some() { break; }
                }
                match next { Some((j,p)) => { used[j]=true; ends[end]=p; }, None => break }
            }
        }
        result.push(VectorSegment {a:ends[0],b:ends[1],kind:line.kind});
    }
    result
}
#[cfg(test)]
fn exhaustive_merge_reference(lines: Vec<VectorSegment>, tolerance: f64) -> Vec<VectorSegment> {
    let mut used = vec![false; lines.len()];
    let mut result = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if used[i] { continue; }
        used[i] = true;
        let direction = (line.b-line.a).normalize_or_zero();
        let mut ends = [line.a, line.b];
        for end in 0..2 {
            let forward = if end == 0 { -direction } else { direction };
            loop {
                let mut next = None;
                for (j, candidate) in lines.iter().enumerate() {
                    if used[j] || candidate.kind != line.kind { continue; }
                    for (start, other) in [(candidate.a,candidate.b),(candidate.b,candidate.a)] {
                        let delta = other-start;
                        if (start-ends[end]).abs().max_element() <= tolerance
                            && forward.cross(delta.normalize_or_zero()).length_squared() <= 1e-14
                            && forward.dot(delta) > 0. {
                            next = Some((j,other)); break;
                        }
                    }
                    if next.is_some() { break; }
                }
                match next { Some((j,p)) => { used[j]=true; ends[end]=p; }, None => break }
            }
        }
        result.push(VectorSegment {a:ends[0],b:ends[1],kind:line.kind});
    }
    result
}
/// Closed even-odd section material for paper poché. Unlike viewport stencil,
/// paper fill is view-independent and deliberately ignores open cut chains.
pub fn paper_section_triangles(scene:&Scene)->Vec<[DVec3;3]> {
    let Some(plane)=scene.active_section() else{return vec![];};
    let surfaces=crate::section_clip::contour_geometry(scene);
    let cuts=crate::section_clip::cap_segments(std::iter::once(surfaces),plane);
    cap_triangles(&cuts,plane)
}
/// Observe the source Qt point mapping independently of HLR and the page writer.
/// Small leaf meshes use QMatrix4x4::map scalar float operations; nested containers
/// and larger leaves use the source NumPy affine path, rounded at QVector3D storage.
pub fn original_qt_paper_point(matrix: DMat4, point: DVec3, bulk: bool) -> DVec3 {
    let m = matrix.to_cols_array().map(|v| v as f32);
    let p = point.as_vec3().to_array();
    let component = |row: usize| {
        if bulk {
            ((m[row] as f64 * p[0] as f64
                + m[4 + row] as f64 * p[1] as f64
                + m[8 + row] as f64 * p[2] as f64)
                + m[12 + row] as f64) as f32
        } else {
            ((m[row] * p[0] + m[4 + row] * p[1]) + m[8 + row] * p[2]) + m[12 + row]
        }
    };
    DVec3::new(
        component(0) as f64,
        component(1) as f64,
        component(2) as f64,
    )
}
fn original_qt_matrix_product(a: DMat4, b: DMat4) -> DMat4 {
    let a = a.to_cols_array().map(|v| v as f32);
    let b = b.to_cols_array().map(|v| v as f32);
    let mut out = [0.; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = (((a[row] * b[col * 4] + a[4 + row] * b[col * 4 + 1])
                + a[8 + row] * b[col * 4 + 2])
                + a[12 + row] * b[col * 4 + 3]) as f64;
        }
    }
    DMat4::from_cols_array(&out)
}
fn original_qt_paper_geometry(
    scene: &crate::paper_scene::PaperSceneView<'_>,
    basis: &OriginalQtPaperBasis,
) -> (
    crate::buffer::MeshRenderBuffer,
    Vec<VectorSegment>,
    Vec<[DVec3; 3]>,
) {
    fn add(
        scene: &crate::paper_scene::PaperSceneView<'_>,
        mesh: &Mesh,
        matrix: DMat4,
        bulk: bool,
        basis: &OriginalQtPaperBasis,
        _surfaces: &mut crate::buffer::MeshRenderBuffer,
        edges: &mut Vec<VectorSegment>,
        triangles: &mut Vec<[DVec3; 3]>,
    ) {
        let mapped = mesh
            .vertices
            .iter()
            .map(|v| original_qt_paper_point(matrix, v.position, bulk))
            .collect::<Vec<_>>();
        for id in 0..mesh.faces.len() {
            if !scene.mesh_face_visible(mesh, id) {
                continue;
            }
            for (a, b, c) in mesh.triangulate_face(id) {
                let m = DMat4::from_cols_array(&matrix.to_cols_array().map(|v| v as f32 as f64));
                let a = a.as_vec3().as_dvec3();
                let b = b.as_vec3().as_dvec3();
                let c = c.as_vec3().as_dvec3();
                let origin = m.transform_point3(a);
                triangles.push([
                    origin,
                    origin + m.transform_vector3(b - a),
                    origin + m.transform_vector3(c - a),
                ]);

            }
        }
        let normal = |id: usize| {
            let face = &mesh.faces[id];
            let mut n = DVec3::ZERO;
            for i in 0..face.vertices.len() {
                let a = mesh.vertices[face.vertices[i]]
                    .position
                    .as_vec3()
                    .as_dvec3();
                let b = mesh.vertices[face.vertices[(i + 1) % face.vertices.len()]]
                    .position
                    .as_vec3()
                    .as_dvec3();
                n += DVec3::new(
                    (a.y - b.y) * (a.z + b.z),
                    (a.z - b.z) * (a.x + b.x),
                    (a.x - b.x) * (a.y + b.y),
                );
            }
            let n = n.as_vec3();
            let n = n.normalize_or_zero().as_dvec3();
            let m = DMat4::from_cols_array(&matrix.to_cols_array().map(|v| v as f32 as f64));
            m.inverse()
                .transpose()
                .transform_vector3(n)
                .normalize_or_zero()
        };
        for (id, e) in mesh.edges.iter().enumerate() {
            if !scene.mesh_edge_visible(mesh, id) {
                continue;
            }
            let kind = if e.soft {
                let faces = e
                    .faces
                    .iter()
                    .copied()
                    .filter(|&id| scene.mesh_face_visible(mesh, id))
                    .collect::<Vec<_>>();
                if faces.is_empty() {
                    continue;
                }
                if faces.len() > 1
                    && (normal(faces[0]).dot(basis.forward) < 0.)
                        == (normal(faces[1]).dot(basis.forward) < 0.)
                {
                    continue;
                }
                LineKind::Profile
            } else {
                LineKind::Edge
            };
            if let Some((a, b)) = scene.section_segment(mapped[e.v0], mapped[e.v1]) {
                edges.push(VectorSegment { a, b, kind });
            }
        }
    }
    fn group(
        scene: &crate::paper_scene::PaperSceneView<'_>,
        g: &Group,
        parent: DMat4,
        _in_container: bool,
        basis: &OriginalQtPaperBasis,
        s: &mut crate::buffer::MeshRenderBuffer,
        e: &mut Vec<VectorSegment>,
        t: &mut Vec<[DVec3; 3]>,
    ) {
        if !scene.group_visible(g) {
            return;
        }
        let matrix = original_qt_matrix_product(parent, g.transform);
        let bulk = true; // Actual Viewport._instance_entry cached arrays use NumPy affine for every placement.
        add(scene, &g.mesh, matrix, bulk, basis, s, e, t);
        for c in &g.children {
            group(scene, c, matrix, true, basis, s, e, t);
        }
    }
    let mut surfaces = crate::buffer::MeshRenderBuffer::new();
    let mut edges = vec![];
    let mut triangles = vec![];
    add(
        scene,
        &scene.mesh,
        DMat4::IDENTITY,
        false,
        basis,
        &mut surfaces,
        &mut edges,
        &mut triangles,
    );
    for g in &scene.groups {
        group(
            scene,
            g,
            DMat4::IDENTITY,
            false,
            basis,
            &mut surfaces,
            &mut edges,
            &mut triangles,
        );
    }
    (surfaces, edges, triangles)
}
/// Observation of precisely the WORLD arrays consumed by the paper interval
/// publication path. No Scene copies or worker jobs are performed by this probe.
pub fn original_qt_paper_observation(
    scene: &Scene,
    camera: &OrbitCamera,
) -> (Vec<[DVec3; 3]>, Vec<VectorSegment>) {
    let (_, edges, triangles) =
        original_qt_paper_geometry(&crate::paper_scene::PaperSceneView::new(scene, None), &OriginalQtPaperBasis::from_camera(camera));
    (triangles, edges)
}
pub fn parallel_visible_segments(scene: &Scene, camera: &OrbitCamera) -> Vec<VectorSegment> {
    parallel_segments(scene, camera, true, false)
}
/// Paper HLR options match original profiles and hidden-line controls.
pub fn parallel_segments(scene: &Scene, camera: &OrbitCamera, profiles: bool, hidden: bool) -> Vec<VectorSegment> {
    segments_impl(scene,camera,profiles,hidden,None,false).expect("serial HLR has no process errors")
}
/// Production HLR using persistent native CPU processes for quadratic edge/triangle work.
/// Caller retains the pool across frames; process failure is explicit, never silent fallback.
pub fn parallel_segments_with_pool(scene:&Scene,camera:&OrbitCamera,profiles:bool,hidden:bool,pool:&mut crate::process_hlr::HlrProcessPool)->std::io::Result<Vec<VectorSegment>> {
    segments_impl(scene,camera,profiles,hidden,Some(pool),false)
}
/// Pinned-original Qt WORLD precision for physical paper only. The binary interval
/// protocol is unchanged: projected f64 data is published to the caller's same pool.
pub fn parallel_original_qt_paper_with_pool(scene:&Scene,camera:&OrbitCamera,profiles:bool,hidden:bool,pool:&mut crate::process_hlr::HlrProcessPool)->std::io::Result<Vec<VectorSegment>> {
    segments_impl(scene,camera,profiles,hidden,Some(pool),true)
}
/// Completed paper ink and the closed section material computed by the same cold
/// geometry pass. Callers can cache admission/fill data without rebuilding contours.
pub struct OriginalQtPaperDrawing {
    pub segments: Vec<VectorSegment>,
    pub section_material: Vec<[DVec3; 3]>,
    pub section_loops: Vec<Vec<DVec3>>,
}
pub fn original_qt_paper_drawing_with_pool(
    scene: &Scene, camera: &OrbitCamera, profiles: bool, hidden: bool,
    pool: &mut crate::process_hlr::HlrProcessPool,
) -> std::io::Result<OriginalQtPaperDrawing> {
    original_qt_paper_drawing_for_view_with_pool(
        &crate::paper_scene::PaperSceneView::new(scene, None), camera, profiles, hidden, pool)
}
pub fn original_qt_paper_drawing_for_view_with_pool(
    scene: &crate::paper_scene::PaperSceneView<'_>, camera: &OrbitCamera,
    profiles: bool, hidden: bool, pool: &mut crate::process_hlr::HlrProcessPool,
) -> std::io::Result<OriginalQtPaperDrawing> {
    let basis = OriginalQtPaperBasis::from_camera(camera);
    let (surfaces, edges, triangles) = original_qt_paper_geometry(scene, &basis);
    let mut section_material = Vec::new();
    let mut section_loops = Vec::new();
    let segments = segments_projected(scene, &basis, profiles, hidden, Some(pool),
        surfaces, edges, triangles, true, Some(&mut section_material), Some(&mut section_loops))?;
    Ok(OriginalQtPaperDrawing { segments, section_material, section_loops })
}
fn segments_impl(scene:&Scene,camera:&OrbitCamera,profiles:bool,hidden:bool,pool:Option<&mut crate::process_hlr::HlrProcessPool>,original_qt:bool)->std::io::Result<Vec<VectorSegment>> {
    let view = crate::paper_scene::PaperSceneView::new(scene, None);
    if original_qt {
        let basis=OriginalQtPaperBasis::from_camera(camera);
        let(surfaces,edges,triangles)=original_qt_paper_geometry(&view,&basis);
        segments_projected(&view,&basis,profiles,hidden,pool,surfaces,edges,triangles,true,None,None)
    }else{
        segments_projected(&view,camera,profiles,hidden,pool,crate::section_clip::contour_geometry(scene),gather_edges(scene,camera),vec![],false,None,None)
    }
}
fn segments_projected(scene:&crate::paper_scene::PaperSceneView<'_>,camera:&impl PaperProjection,profiles:bool,hidden:bool,pool:Option<&mut crate::process_hlr::HlrProcessPool>,mut surfaces:crate::buffer::MeshRenderBuffer,edges:Vec<VectorSegment>,original_triangles:Vec<[DVec3;3]>,original_qt:bool,section_material:Option<&mut Vec<[DVec3;3]>>,section_loops:Option<&mut Vec<Vec<DVec3>>>)->std::io::Result<Vec<VectorSegment>>{
    let mut edges=edges;
    let mut caps = vec![];
    let mut original_triangles=original_triangles;
    let mut loops=Vec::new();
    if let Some(plane) = scene.active_section() {
        let cuts = if original_qt {
            let (kept,cuts)=crate::paper_section::clip_triangles(original_triangles,plane);
            original_triangles=kept;
            loops=crate::paper_section::closed_loops(&cuts);
            cuts
        } else {
            let cuts=crate::section_clip::cap_segments(std::iter::once(surfaces.clone()),plane);
            crate::section_clip::mesh(&mut surfaces,plane);
            cuts
        };
        caps=cap_triangles(&cuts,plane);
        for (a,b) in cuts {edges.push(VectorSegment {a,b,kind:LineKind::Cut});}
    }
    if let Some(material)=section_material {material.extend_from_slice(&caps);}
    if let Some(output)=section_loops {output.extend(loops.iter().cloned());}
    let triangles = if original_qt {
        original_triangles.into_iter().map(|t|t.map(|p|project(p,camera))).collect::<Vec<_>>()
    } else {
        surfaces.indices.chunks_exact(3).map(|ids| std::array::from_fn(|i| project(DVec3::from_array(surfaces.vertices[ids[i] as usize].position.map(f64::from)),camera))).collect::<Vec<_>>()
    };
    let triangle_index=crate::triangle_index::TriangleIndex::new(triangles.iter().copied());
    let boundary = if original_qt {
        loops.iter().flat_map(|ring| (0..ring.len()).map(|i|
            (project(ring[i],camera),project(ring[(i+1)%ring.len()],camera)))).collect()
    } else {cap_boundary(&caps,camera)};
    let cap_plane = if original_qt {
        scene.active_section().filter(|_|!loops.is_empty()).map(|plane|
            crate::paper_section::depth_triangle(plane).map(|p|project(p,camera)))
            .filter(|t|((t[1].x-t[0].x)*(t[2].y-t[0].y)
                -(t[1].y-t[0].y)*(t[2].x-t[0].x)).abs()>=1e-9)
    } else {caps.iter().map(|tri|tri.map(|p|project(p,camera)))
        .find(|&t|bary(DVec2::ZERO,t).is_some())};
    let min = triangles
        .iter()
        .flatten()
        .map(|p| p.z)
        .fold(f64::INFINITY, f64::min);
    let max = triangles
        .iter()
        .flatten()
        .map(|p| p.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let epsilon = if min.is_finite() {
        (if max > min { max - min } else { 1. } * 1e-4).max(1e-9)
    } else {
        1e-9
    };
    let extent = triangles.iter().flatten().fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(p.truncate()), hi.max(p.truncate())),
    );
    let delta = ((extent.1 - extent.0).max_element() * 1e-4).max(1e-9);
    let ptol = epsilon + delta * 10.;
    let depths = edges
        .iter()
        .flat_map(|e| [project(e.a, camera).z, project(e.b, camera).z])
        .collect::<Vec<_>>();
    let cap_eps = (depths.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - depths.iter().copied().fold(f64::INFINITY, f64::min))
        * 1e-4;
    let cap_eps = cap_eps.max(epsilon);
    let process_intervals=if let Some(pool)=pool {
        let projected=edges.iter().map(|e|(project(e.a,camera),project(e.b,camera))).collect::<Vec<_>>();
        Some(pool.intervals(&projected,&triangles,epsilon)?)
    } else {None};
    let mut process_intervals=process_intervals.map(|rows|rows.into_iter());
    let mut result = vec![];
    for edge in edges {
        let prepared=process_intervals.as_mut().map(|rows|rows.next().expect("one result per edge"));
        let a = project(edge.a, camera);
        let b = project(edge.b, camera);
        if (a.truncate() - b.truncate()).length() < 1e-10 {
            continue;
        }
        let mut intervals = if let Some(row)=prepared {row} else {
            triangles.iter().filter_map(|&tri|occluded_interval(a,b,tri,epsilon)).collect::<Vec<_>>()
        };
        let cap_hidden = cap_intervals(a, b, &boundary, cap_plane, cap_eps);
        if hidden && edge.kind == LineKind::Edge {
            for (lo, hi) in complement(complement(intervals.clone())) {
                // Cap material suppresses both solid and dashed lines.
                let blocked = cap_hidden.iter().filter_map(|&(x,y)| {
                    let x=x.max(lo); let y=y.min(hi);
                    (y>x).then_some(((x-lo)/(hi-lo),(y-lo)/(hi-lo)))
                }).collect::<Vec<_>>();
                for (x,y) in complement(blocked) {
                    result.push(VectorSegment {
                        a:edge.a.lerp(edge.b,lo+x*(hi-lo)),
                        b:edge.a.lerp(edge.b,lo+y*(hi-lo)),kind:LineKind::Hidden,
                    });
                }
            }
        }
        intervals.extend(cap_hidden);
        for (lo, hi) in complement(intervals) {
            let mut kind = edge.kind;
            if profiles && kind == LineKind::Edge {
                let mid = a.lerp(b, (lo + hi) / 2.);
                let direction = (b - a).truncate().normalize();
                let normal = DVec2::new(-direction.y, direction.x) * delta;
                if !surface_at(mid.truncate() + normal, mid.z, &triangles,&triangle_index, ptol)
                    || !surface_at(mid.truncate() - normal, mid.z, &triangles,&triangle_index, ptol)
                {
                    kind = LineKind::Profile;
                }
            }
            result.push(VectorSegment {
                a: edge.a.lerp(edge.b, lo),
                b: edge.a.lerp(edge.b, hi),
                kind,
            });
        }
    }
    let result = merge_collinear(result, delta * 1e-5);
    Ok(if hidden { drop_hidden_under_visible(result,camera,delta * 1e-2) } else { result })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_merge_preserves_source_order_and_exact_endpoints() {
        for scale in [1e-4,1.,1e4] { for offset in [0.,1e6] {
            let mut lines=vec![];
            for i in 0..160 { for kind in [LineKind::Edge,LineKind::Hidden,LineKind::Profile] {
                let a=DVec3::new(offset+i as f64*scale,scale,0.);
                let b=a+DVec3::X*scale;
                lines.push(VectorSegment{a,b,kind});
                if i%7==0 {lines.push(VectorSegment{a:b,b:a,kind});}
            }}
            lines.reverse();lines.push(VectorSegment{a:DVec3::ZERO,b:DVec3::ZERO,kind:LineKind::Edge});
            for tolerance in [0.,scale*1e-5,scale*0.1] {
                let old=exhaustive_merge_reference(lines.clone(),tolerance);
                let new=merge_collinear(lines.clone(),tolerance);
                assert_eq!(old.len(),new.len());
                for (a,b) in old.iter().zip(new) { assert_eq!(a.a,b.a);assert_eq!(a.b,b.b);assert_eq!(a.kind,b.kind); }
            }
        }}
    }
    #[test]
    fn cached_hidden_projections_preserve_growing_result_order_bit_exactly(){
        let mut camera=OrbitCamera::default();
        let mut lines=vec![];
        for depth in [0.,0.2,2.] {for i in 0..100{let x=i as f64*0.1;for kind in [LineKind::Edge,LineKind::Hidden]{lines.push(VectorSegment{a:DVec3::new(x,0.,depth),b:DVec3::new(x+0.15,0.,depth),kind});}}}
        lines.push(VectorSegment{a:DVec3::ZERO,b:DVec3::ZERO,kind:LineKind::Hidden});
        for yaw in [0.,0.7,1.2]{camera.yaw=yaw;for tol in [1e-9,1e-4,0.1]{
            let original=exhaustive_hidden_ink_reference(lines.clone(),&camera,tol);
            let cached=drop_hidden_under_visible(lines.clone(),&camera,tol);
            assert_eq!(original.len(),cached.len());for(a,b)in original.iter().zip(cached){assert_eq!(a.a,b.a);assert_eq!(a.b,b.b);assert_eq!(a.kind,b.kind);}
        }}
    }
    #[test]
    fn projected_hidden_ink_deduplicates_depth_and_subtracts_solid_overlap() {
        let mut camera=OrbitCamera::default(); camera.yaw=-std::f64::consts::FRAC_PI_2; camera.pitch=0.;
        let line=|a,b,kind| VectorSegment{a,b,kind};
        let lines=vec![
            line(DVec3::new(0.,0.,0.),DVec3::new(1.,0.,0.),LineKind::Profile),
            line(DVec3::new(0.,2.,0.),DVec3::new(2.,2.,0.),LineKind::Hidden),
            line(DVec3::new(0.,3.,0.),DVec3::new(2.,3.,0.),LineKind::Hidden),
        ];
        let result=drop_hidden_under_visible(lines,&camera,1e-9);
        assert_eq!(result.len(),2);
        assert_eq!(result[1].kind,LineKind::Hidden);
        assert!((result[1].a.x-1.).abs()<1e-9);
        assert!((result[1].b.x-2.).abs()<1e-9);
    }
    #[test]
    fn triangle_cut_runs_merge_but_depth_and_line_class_remain_distinct() {
        let segment = |a, b, kind| VectorSegment { a, b, kind };
        let lines = vec![
            segment(DVec3::ZERO,DVec3::X,LineKind::Cut),
            segment(DVec3::X*2.,DVec3::X,LineKind::Cut),
            segment(DVec3::Z,DVec3::Z+DVec3::X,LineKind::Cut),
            segment(DVec3::X*2.,DVec3::X*3.,LineKind::Edge),
        ];
        let result = merge_collinear(lines,1e-9);
        assert_eq!(result.len(),3);
        assert_eq!(result[0].a,DVec3::ZERO);
        assert_eq!(result[0].b,DVec3::X*2.);
        assert_eq!(result[1].a,DVec3::Z);
        assert_eq!(result[2].kind,LineKind::Edge);
    }
    #[test]
    fn actual_triangle_occlusion_splits_rear_line_and_preserves_coplanar_front() {
        let mut scene = Scene::new();
        let ids = [
            DVec3::new(-1., -1., 2.),
            DVec3::new(1., -1., 2.),
            DVec3::new(1., 1., 2.),
            DVec3::new(-1., 1., 2.),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        scene.mesh.add_face(ids.to_vec(), vec![]);
        let a = scene.mesh.add_vertex(DVec3::new(-2., 0., 1.));
        let b = scene.mesh.add_vertex(DVec3::new(2., 0., 1.));
        scene.mesh.add_edge(a, b);
        let mut camera = OrbitCamera::default();
        camera.pitch = std::f64::consts::FRAC_PI_2;
        camera.up = DVec3::Y;
        camera.perspective = false;
        let lines = parallel_visible_segments(&scene, &camera);
        let rear = lines
            .iter()
            .filter(|s| (s.a.z - 1.).abs() < 1e-10)
            .collect::<Vec<_>>();
        assert_eq!(rear.len(), 2);
        assert!((rear.iter().map(|s| (s.b - s.a).length()).sum::<f64>() - 2.).abs() < 1e-8);
        assert_eq!(
            lines.iter().filter(|s| (s.a.z - 2.).abs() < 1e-10).count(),
            4
        );
    }
    #[test]
    fn section_cap_occludes_internal_lines_and_cut_chords_remain_visible() {
        let mut scene = Scene::new();
        let ids = [
            (-1., -1., -1.),
            (1., -1., -1.),
            (1., 1., -1.),
            (-1., 1., -1.),
            (-1., -1., 1.),
            (1., -1., 1.),
            (1., 1., 1.),
            (-1., 1., 1.),
        ]
        .map(|(x, y, z)| scene.mesh.add_vertex(DVec3::new(x, y, z)));
        for ring in [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ] {
            scene.mesh.add_face(ring.map(|i| ids[i]).to_vec(), vec![]);
        }
        let a = scene.mesh.add_vertex(DVec3::new(-0.5, 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(0.5, 0., 0.));
        scene.mesh.add_edge(a, b);
        scene
            .section_planes
            .push(ingetrazo_core::section::SectionPlane::new(
                DVec3::new(0., 0., 0.5),
                DVec3::Z,
                "Cut",
                "A",
            ));
        scene.show_section_cuts = true;
        let mut camera = OrbitCamera::default();
        camera.pitch = std::f64::consts::FRAC_PI_2;
        camera.up = DVec3::Y;
        camera.perspective = false;
        let lines = parallel_visible_segments(&scene, &camera);
        assert!(lines
            .iter()
            .all(|s| s.a.z.abs() > 1e-8 || s.b.z.abs() > 1e-8));
        let cut_length = lines
            .iter()
            .filter(|s| s.kind == LineKind::Cut)
            .map(|s| (s.b - s.a).length())
            .sum::<f64>();
        assert!((cut_length - 8.).abs() < 1e-7);
    }
    #[test]
    fn paper_section_hollow_rings_follow_even_odd_without_reversed_winding() {
        let mut scene = Scene::new();
        for size in [2., 1.] {
            let ids = [
                (-1., -1., -1.),
                (1., -1., -1.),
                (1., 1., -1.),
                (-1., 1., -1.),
                (-1., -1., 1.),
                (1., -1., 1.),
                (1., 1., 1.),
                (-1., 1., 1.),
            ]
            .map(|(x, y, z)| scene.mesh.add_vertex(DVec3::new(x, y, z) * size));
            for ring in [
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
            ] {
                scene.mesh.add_face(ring.map(|i| ids[i]).to_vec(), vec![]);
            }
        }
        let a = scene.mesh.add_vertex(DVec3::new(-1.5, 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(1.5, 0., 0.));
        scene.mesh.add_edge(a, b);
        scene
            .section_planes
            .push(ingetrazo_core::section::SectionPlane::new(
                DVec3::new(0., 0., 0.5),
                DVec3::Z,
                "Cut",
                "A",
            ));
        scene.show_section_cuts = true;
        let mut camera = OrbitCamera::default();
        camera.pitch = std::f64::consts::FRAC_PI_2;
        camera.up = DVec3::Y;
        camera.perspective = false;
        let lines = parallel_visible_segments(&scene, &camera);
        let length = lines
            .iter()
            .filter(|s| s.a.z.abs() < 1e-9 && s.b.z.abs() < 1e-9)
            .map(|s| (s.b - s.a).length())
            .sum::<f64>();
        assert!(
            (length - 2.).abs() < 1e-7,
            "inner ring must leave a see-through hole independent of orientation: {length}"
        );
    }
    #[test]
    fn face_hole_does_not_occlude_line_through_void() {
        let mut scene = Scene::new();
        let outer = [(-2., -2.), (2., -2.), (2., 2.), (-2., 2.)]
            .map(|(x, y)| scene.mesh.add_vertex(DVec3::new(x, y, 2.)));
        let hole = [(-1., -1.), (-1., 1.), (1., 1.), (1., -1.)]
            .map(|(x, y)| scene.mesh.add_vertex(DVec3::new(x, y, 2.)));
        scene.mesh.add_face(outer.to_vec(), vec![hole.to_vec()]);
        let a = scene.mesh.add_vertex(DVec3::new(-3., 0., 1.));
        let b = scene.mesh.add_vertex(DVec3::new(3., 0., 1.));
        scene.mesh.add_edge(a, b);
        let mut camera = OrbitCamera::default();
        camera.pitch = std::f64::consts::FRAC_PI_2;
        camera.up = DVec3::Y;
        camera.perspective = false;
        let rear = parallel_visible_segments(&scene, &camera)
            .into_iter()
            .filter(|s| (s.a.z - 1.).abs() < 1e-10)
            .collect::<Vec<_>>();
        assert_eq!(rear.len(), 3);
        assert!((rear.iter().map(|s| (s.b - s.a).length()).sum::<f64>() - 4.).abs() < 1e-8);
    }
}
