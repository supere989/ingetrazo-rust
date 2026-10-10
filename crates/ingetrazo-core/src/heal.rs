// SPDX-License-Identifier: GPL-3.0-or-later
//! Original mother-face, nested-hole and flat partial-overlap healing rules.
use crate::{
    mesh::Mesh,
    planar::{self, Frame},
};
use glam::DVec2;
use manifold_rust::{cross_section::CrossSection, linalg::Vec2};
fn area(r: &[DVec2]) -> f64 {
    (0..r.len())
        .map(|i| {
            let a = r[i];
            let b = r[(i + 1) % r.len()];
            a.x * b.y - a.y * b.x
        })
        .sum::<f64>()
        * 0.5
}
fn section(rings: &[Vec<DVec2>]) -> CrossSection {
    let mut polygons = vec![];
    for (i, r) in rings.iter().enumerate() {
        let mut ring = r.clone();
        if (area(&ring) > 0.) != (i == 0) {
            ring.reverse();
        }
        polygons.push(ring.into_iter().map(|p| Vec2::new(p.x, p.y)).collect());
    }
    CrossSection::new(polygons)
}
fn projected(mesh: &Mesh, face: usize, frame: Frame) -> Vec<Vec<DVec2>> {
    planar::rings(mesh, &mesh.faces[face])
        .iter()
        .map(|r| r.iter().map(|&p| frame.project(p)).collect())
        .collect()
}
fn same_plane(mesh: &Mesh, face: usize, frame: Frame) -> bool {
    planar::rings(mesh, &mesh.faces[face])
        .iter()
        .flatten()
        .all(|&p| frame.on_plane(p))
        && mesh.face_normal(face).dot(frame.normal).abs() > 0.999
}
/// Returns (prepared mesh, original indices of removed redundant mothers).
pub fn heal_faces(
    mesh: &Mesh,
    coverage: f64,
    partial: Option<bool>,
) -> Result<(Mesh, Vec<usize>), String> {
    if !coverage.is_finite() || !(0. ..=1.).contains(&coverage) {
        return Err("Heal coverage must be between zero and one".into());
    }
    planar::validate_mesh(mesh)?;
    if mesh.faces.len() > 3000 || mesh.faces.is_empty() {
        return Ok((mesh.clone(), vec![]));
    }
    let dominant = (0..mesh.faces.len())
        .max_by(|&a, &b| mesh.face_area(a).total_cmp(&mesh.face_area(b)))
        .unwrap();
    let main = Frame::new(&planar::rings(mesh, &mesh.faces[dominant])[0])?;
    let flat = mesh.vertices.iter().all(|v| main.on_plane(v.position));
    let partial = partial.unwrap_or(flat);
    let mut clean = mesh.clone();
    // Nested holes: retain only maximal contours, independently of winding.
    for i in 0..clean.faces.len() {
        let frame = Frame::new(&planar::rings(&clean, &clean.faces[i])[0])?;
        let holes = projected(&clean, i, frame);
        let mut keep = vec![];
        for j in 1..holes.len() {
            let nested = (1..holes.len()).any(|k| {
                k != j
                    && area(&holes[k]).abs() >= area(&holes[j]).abs()
                    && holes[j]
                        .iter()
                        .all(|&p| planar::inside_or_boundary(p, &holes[k]))
                    && (area(&holes[k]).abs() > area(&holes[j]).abs() + 1e-8 || k < j)
            });
            if !nested {
                keep.push(clean.faces[i].holes[j - 1].clone());
            }
        }
        clean.faces[i].holes = keep;
        if flat {
            clean.faces[i].interior = false;
        }
        if flat && clean.face_normal(i).dot(main.normal) < 0. {
            clean.faces[i].vertices.reverse();
            for hole in &mut clean.faces[i].holes {
                hole.reverse();
            }
        }
    }
    let areas: Vec<_> = (0..clean.faces.len()).map(|i| clean.face_area(i)).collect();
    let mut gone = vec![false; clean.faces.len()];
    let mut removed = vec![];
    for i in 0..clean.faces.len() {
        if areas[i] < 1e-8 {
            continue;
        }
        let frame = Frame::new(&planar::rings(&clean, &clean.faces[i])[0])?;
        let base = section(&projected(&clean, i, frame));
        let mut covered = 0.;
        for j in 0..clean.faces.len() {
            if gone[j] || i == j || areas[j] >= areas[i] || !same_plane(&clean, j, frame) {
                continue;
            }
            let other = section(&projected(&clean, j, frame));
            if (base.intersection(&other).area() - areas[j]).abs() < areas[j].max(1.) * 1e-6 {
                covered += areas[j];
            }
        }
        if covered > coverage * areas[i] {
            gone[i] = true;
            removed.push(i);
        }
    }
    let mut punch = vec![Vec::<usize>::new(); clean.faces.len()];
    if partial {
        for i in 0..clean.faces.len() {
            if gone[i] {
                continue;
            }
            let probes: Vec<_> = clean
                .triangulate_face(i)
                .iter()
                .map(|&(a, b, c)| (a + b + c) / 3.)
                .collect();
            if probes.is_empty() {
                continue;
            }
            for j in 0..clean.faces.len() {
                if gone[j] || i == j || areas[j] <= areas[i] {
                    continue;
                }
                let frame = Frame::new(&planar::rings(&clean, &clean.faces[j])[0])?;
                if !same_plane(&clean, i, frame) {
                    continue;
                }
                let rings = projected(&clean, j, frame);
                let inside = probes
                    .iter()
                    .filter(|&&p| planar::inside_rings(frame.project(p), &rings))
                    .count();
                if inside * 2 > probes.len() {
                    punch[j].push(i);
                    break;
                }
            }
        }
    }
    let mut result = Mesh::new();
    for i in 0..clean.faces.len() {
        if gone[i] {
            continue;
        }
        if punch[i].is_empty() {
            planar::add_face(
                &mut result,
                &planar::rings(&clean, &clean.faces[i]),
                &clean.faces[i],
            )?;
            continue;
        }
        let frame = Frame::new(&planar::rings(&clean, &clean.faces[i])[0])?;
        let mut body = section(&projected(&clean, i, frame));
        for &j in &punch[i] {
            body = body.difference(&section(&projected(&clean, j, frame)));
        }
        for part in body.decompose() {
            let rings: Vec<Vec<_>> = part
                .to_polygons()
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|p| frame.unproject(DVec2::new(p.x, p.y)))
                        .collect()
                })
                .collect();
            if !rings.is_empty() {
                planar::add_face(&mut result, &rings, &clean.faces[i])?;
            }
        }
    }
    planar::preserve_edges(&mut result, &clean);
    let mut result = planar::stitch(&result)?;
    if !flat {
        orient_closed_components(&mut result)?;
    }
    planar::validate_mesh(&result)?;
    if planar::equivalent(mesh, &result) {
        return Ok((mesh.clone(), removed));
    }
    Ok((result, removed))
}
/// Orient a flat drawing consistently, or orient closed 3D components.
pub fn orient_faces(mesh: &Mesh) -> Result<Mesh, String> {
    planar::validate_mesh(mesh)?;
    if mesh.faces.is_empty() {
        return Ok(mesh.clone());
    }
    let mut out = mesh.clone();
    let dominant = (0..mesh.faces.len())
        .max_by(|&a, &b| mesh.face_area(a).total_cmp(&mesh.face_area(b)))
        .unwrap();
    let frame = Frame::new(&planar::rings(mesh, &mesh.faces[dominant])[0])?;
    if mesh.vertices.iter().all(|v| frame.on_plane(v.position)) {
        for i in 0..out.faces.len() {
            out.faces[i].interior = false;
            if out.face_normal(i).dot(frame.normal) < 0. {
                out.faces[i].vertices.reverse();
                for h in &mut out.faces[i].holes {
                    h.reverse();
                }
            }
        }
    } else {
        orient_closed_components(&mut out)?;
    }
    Ok(out)
}

fn orient_closed_components(mesh: &mut Mesh) -> Result<(), String> {
    use glam::DVec3;
    use std::collections::{BTreeMap, VecDeque};
    let triangles: Vec<_> = (0..mesh.faces.len())
        .map(|i| mesh.triangulate_face(i))
        .collect();
    let mut incident = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (id, face) in mesh.faces.iter().enumerate() {
        for ring in std::iter::once(&face.vertices).chain(&face.holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                incident.entry((a.min(b), a.max(b))).or_default().push(id);
            }
        }
    }
    let mut component = vec![usize::MAX; mesh.faces.len()];
    let mut groups = vec![];
    for start in 0..mesh.faces.len() {
        if component[start] != usize::MAX {
            continue;
        }
        let id = groups.len();
        let mut faces = vec![];
        let mut queue = VecDeque::from([start]);
        component[start] = id;
        while let Some(f) = queue.pop_front() {
            faces.push(f);
            let face = &mesh.faces[f];
            for ring in std::iter::once(&face.vertices).chain(&face.holes) {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    for &other in &incident[&(a.min(b), a.max(b))] {
                        if component[other] == usize::MAX {
                            component[other] = id;
                            queue.push_back(other);
                        }
                    }
                }
            }
        }
        groups.push(faces);
    }
    let probes: Vec<_> = (0..mesh.faces.len())
        .map(|i| {
            let face = &mesh.faces[i];
            let frame = Frame::new(&planar::rings(mesh, face)[0])?;
            let average = mesh.face_centroid(i);
            if face.holes.is_empty()
                && planar::inside_ring(
                    frame.project(average),
                    &face
                        .vertices
                        .iter()
                        .map(|&v| frame.project(mesh.vertices[v].position))
                        .collect::<Vec<_>>(),
                )
            {
                return Ok((average, mesh.face_normal(i)));
            }
            let probe = triangles[i]
                .iter()
                .max_by(|(a, b, c), (x, y, z)| {
                    (b - a)
                        .cross(*c - *a)
                        .length_squared()
                        .total_cmp(&(y - x).cross(*z - *x).length_squared())
                })
                .map(|&(a, b, c)| (a + b + c) / 3.)
                .ok_or("Cannot probe a degenerate orientation face")?;
            Ok((probe, mesh.face_normal(i)))
        })
        .collect::<Result<Vec<_>, String>>()?;
    fn outside(
        origin: DVec3,
        normal: DVec3,
        faces: &[usize],
        tris: &[Vec<(DVec3, DVec3, DVec3)>],
        boundary: &[bool],
        exclude: usize,
    ) -> bool {
        let n = normal.normalize();
        let u = n
            .cross(if n.z.abs() < 0.8 { DVec3::Z } else { DVec3::Y })
            .normalize();
        let v = n.cross(u);
        // Seven small deterministic jitter rays, including the exact normal.
        // Jitter is bounded to the original 3e-4 angular cone.
        let jitter = [
            (0., 0.),
            (0.73, 0.31),
            (-0.41, 0.89),
            (0.19, -0.67),
            (-0.83, -0.23),
            (0.47, -0.91),
            (-0.59, 0.53),
        ];
        let votes = jitter
            .into_iter()
            .filter(|&(x, y)| {
                let direction = (n + u * (x * 3e-4) + v * (y * 3e-4)).normalize();
                let mut count = 0;
                for &id in faces {
                    if id == exclude || !boundary[id] {
                        continue;
                    }
                    for &(a, b, c) in &tris[id] {
                        let e1 = b - a;
                        let e2 = c - a;
                        let p = direction.cross(e2);
                        let det = e1.dot(p);
                        if det.abs() < 1e-7 {
                            continue;
                        }
                        let rel = origin - a;
                        let inv = 1. / det;
                        let uu = rel.dot(p) * inv;
                        if uu < -1e-7 || uu > 1. + 1e-7 {
                            continue;
                        }
                        let q = rel.cross(e1);
                        let vv = direction.dot(q) * inv;
                        if vv < -1e-7 || uu + vv > 1. + 1e-7 {
                            continue;
                        }
                        if e2.dot(q) * inv > 1e-6 {
                            count += 1;
                        }
                    }
                }
                count % 2 == 0
            })
            .count();
        votes >= 4
    }
    let state = |id: usize, boundary: &[bool]| {
        let (p, n) = probes[id];
        let faces = &groups[component[id]];
        if outside(p, n, faces, &triangles, boundary, id) {
            0
        } else if outside(p, -n, faces, &triangles, boundary, id) {
            1
        } else {
            2
        }
    };
    let mut boundary = vec![true; mesh.faces.len()];
    for _ in 0..4 {
        let next: Vec<_> = (0..mesh.faces.len())
            .map(|i| state(i, &boundary) != 2)
            .collect();
        if next == boundary {
            break;
        }
        boundary = next;
    }
    let closed = incident.values().all(|faces| faces.len() >= 2)
        && mesh.edges.iter().all(|e| !e.faces.is_empty());
    for i in 0..mesh.faces.len() {
        mesh.faces[i].interior = !boundary[i];
    }
    if closed {
        for i in 0..mesh.faces.len() {
            if boundary[i] && state(i, &boundary) == 1 {
                mesh.faces[i].vertices.reverse();
                for hole in &mut mesh.faces[i].holes {
                    hole.reverse();
                }
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    fn square(mesh: &mut Mesh, x: f64, y: f64, w: f64, h: f64, z: f64) -> usize {
        let ids = [
            DVec3::new(x, y, z),
            DVec3::new(x + w, y, z),
            DVec3::new(x + w, y + h, z),
            DVec3::new(x, y + h, z),
        ]
        .into_iter()
        .map(|p| mesh.add_vertex(p))
        .collect();
        mesh.add_face(ids, vec![]).unwrap()
    }
    fn box_at(mesh: &mut Mesh, x: f64, size: f64) {
        let v: Vec<_> = [
            (0., 0., 0.),
            (1., 0., 0.),
            (1., 1., 0.),
            (0., 1., 0.),
            (0., 0., 1.),
            (1., 0., 1.),
            (1., 1., 1.),
            (0., 1., 1.),
        ]
        .into_iter()
        .map(|(a, b, c)| mesh.add_vertex(DVec3::new(x + a * size, b * size, c * size)))
        .collect();
        for f in [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ] {
            mesh.add_face(f.into_iter().map(|i| v[i]).collect(), vec![]);
        }
    }
    #[test]
    fn orient_marks_shared_partition_and_keeps_nested_shells_independent() {
        let mut m = Mesh::new();
        box_at(&mut m, 0., 1.);
        box_at(&mut m, 1., 1.);
        // Two adjacent boxes share a single retained wall; remove duplicate.
        let mut faces = m.faces.clone();
        faces.remove(11);
        let mut network = Mesh::new();
        for face in &faces {
            planar::add_face(&mut network, &planar::rings(&m, face), face).unwrap();
        }
        network.faces[0].vertices.reverse();
        network.faces[7].vertices.reverse();
        assert!(!network.is_manifold_solid());
        let out = orient_faces(&network).unwrap();
        crate::solids::validate_solid_mesh(&out).unwrap();
        assert_eq!(out.faces.iter().filter(|f| f.interior).count(), 1);
        assert!(out.is_manifold_solid());
        assert!((out.volume().unwrap() - 2.).abs() < 1e-8);
        let again = orient_faces(&out).unwrap();
        assert_eq!(out, again);
        assert!(!serde_json::to_string(&out).unwrap().contains("interior"));
        // Original component-local parity treats the inner shell as its own
        // outward object, rather than reversing it as a global cavity.
        let mut nested = Mesh::new();
        box_at(&mut nested, 0., 4.);
        let mut inner = Mesh::new();
        box_at(&mut inner, 1., 1.);
        for v in &mut inner.vertices {
            v.position.y += 1.;
            v.position.z += 1.;
        }
        for face in &mut inner.faces {
            face.vertices.reverse();
        }
        for face in &inner.faces {
            planar::add_face(&mut nested, &planar::rings(&inner, face), face).unwrap();
        }
        let out = orient_faces(&nested).unwrap();
        assert!(out.faces.iter().all(|f| !f.interior));
        assert!((out.volume().unwrap() - 65.).abs() < 1e-8);
    }

    #[test]
    fn mother_removed_small_island_holed_and_3d_partial_gated() {
        let mut m = Mesh::new();
        square(&mut m, 0., 0., 8., 8., 0.);
        square(&mut m, 0.2, 0.2, 3.6, 7.6, 0.);
        square(&mut m, 4.2, 0.2, 3.6, 7.6, 0.);
        let (healed, removed) = heal_faces(&m, 0.5, None).unwrap();
        assert_eq!(removed, vec![0]);
        assert_eq!(healed.faces.len(), 2);
        let mut m = Mesh::new();
        square(&mut m, 0., 0., 10., 10., 0.);
        square(&mut m, 4., 4., 2., 2., 0.);
        let (unchanged, removed) = heal_faces(&m, 0.5, Some(false)).unwrap();
        assert!(removed.is_empty());
        assert_eq!(unchanged, m);
        let (healed, removed) = heal_faces(&m, 0.5, None).unwrap();
        assert!(removed.is_empty());
        assert_eq!(healed.faces.len(), 2);
        assert!(healed.faces.iter().any(|f| f.holes.len() == 1));
        assert!(
            (healed
                .faces
                .iter()
                .enumerate()
                .map(|(i, _)| healed.face_area(i))
                .sum::<f64>()
                - 100.)
                .abs()
                < 1e-8
        );
        square(&mut m, 0., 0., 1., 1., 3.);
        let (healed, _) = heal_faces(&m, 0.5, None).unwrap();
        assert!(healed.faces.iter().all(|f| f.holes.is_empty()));
    }
    #[test]
    fn reversed_faces_and_nested_holes_preserve_the_actual_ring() {
        let mut m = Mesh::new();
        let outer = square(&mut m, 0., 0., 10., 10., 0.);
        let h = square(&mut m, 2., 2., 6., 6., 0.);
        let nested = [
            DVec3::new(2., 2., 0.),
            DVec3::new(8., 2., 0.),
            DVec3::new(8., 5., 0.),
            DVec3::new(2., 5., 0.),
        ]
        .into_iter()
        .map(|p| m.add_vertex(p))
        .collect();
        m.faces[outer].holes = vec![m.faces[h].vertices.clone(), nested];
        let (healed, removed) = heal_faces(&m, 0.5, None).unwrap();
        assert!(removed.is_empty());
        assert!(healed.faces.iter().any(|f| f.holes.len() == 1));
        let mut m = Mesh::new();
        square(&mut m, 0., 0., 10., 10., 0.);
        let small = square(&mut m, 2., 2., 2., 2., 0.);
        m.faces[small].vertices.reverse();
        let (healed, _) = heal_faces(&m, 0.5, None).unwrap();
        assert!((0..healed.faces.len()).all(|i| healed.face_normal(i).z > 0.));
    }
}
