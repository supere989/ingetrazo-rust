//! Exact parallel edge visibility by projected triangle intervals.
//! Raster viewport depth sorting is separate from this paper/vector pass.
use glam::{DMat4, DVec2, DVec3};
use ingetrazo_core::{camera::OrbitCamera, groups::Group, mesh::Mesh, scene::Scene};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Edge,
    Profile,
    Cut,
}
#[derive(Clone, Debug)]
pub struct VectorSegment {
    pub a: DVec3,
    pub b: DVec3,
    pub kind: LineKind,
}
fn project(p: DVec3, camera: &OrbitCamera) -> DVec3 {
    DVec3::new(
        p.dot(camera.right()),
        p.dot(camera.right().cross(camera.forward())),
        p.dot(camera.forward()),
    )
}
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
fn occluded_interval(a: DVec3, b: DVec3, triangle: [DVec3; 3], epsilon: f64) -> Option<(f64, f64)> {
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
        if !clip_linear(&mut lo, &mut hi, wa[i], wb[i]) {
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
        let Some(&(a, b, c)) = triangles.first() else {
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
/// Return only visible portions; split edges remain referenced to world coordinates.
pub fn parallel_visible_segments(scene: &Scene, camera: &OrbitCamera) -> Vec<VectorSegment> {
    let mut surfaces = crate::section_clip::contour_geometry(scene);
    let mut edges = gather_edges(scene, camera);
    let mut caps = vec![];
    if let Some(plane) = scene.active_section() {
        let cuts = crate::section_clip::cap_segments(std::iter::once(surfaces.clone()), plane);
        caps = cap_triangles(&cuts, plane);
        for (a, b) in cuts {
            edges.push(VectorSegment {
                a,
                b,
                kind: LineKind::Cut,
            });
        }
        crate::section_clip::mesh(&mut surfaces, plane);
    }
    let mut triangles = surfaces
        .indices
        .chunks_exact(3)
        .map(|ids| {
            ids.iter()
                .map(|&i| {
                    project(
                        DVec3::from_array(surfaces.vertices[i as usize].position.map(f64::from)),
                        camera,
                    )
                })
                .collect::<Vec<_>>()
        })
        .map(|v| [v[0], v[1], v[2]])
        .collect::<Vec<_>>();
    triangles.extend(caps.into_iter().map(|tri| tri.map(|p| project(p, camera))));
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
    let mut result = vec![];
    for edge in edges {
        let a = project(edge.a, camera);
        let b = project(edge.b, camera);
        if (a.truncate() - b.truncate()).length() < 1e-10 {
            continue;
        }
        let mut intervals = triangles
            .iter()
            .filter_map(|&tri| occluded_interval(a, b, tri, epsilon))
            .collect::<Vec<_>>();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut start: f64 = 0.;
        for (lo, hi) in intervals {
            if lo > start + 1e-10 {
                result.push(VectorSegment {
                    a: edge.a.lerp(edge.b, start),
                    b: edge.a.lerp(edge.b, lo),
                    kind: edge.kind,
                });
            }
            start = start.max(hi);
        }
        if start < 1. - 1e-10 {
            result.push(VectorSegment {
                a: edge.a.lerp(edge.b, start),
                b: edge.b,
                kind: edge.kind,
            });
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
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
