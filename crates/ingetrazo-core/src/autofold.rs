// SPDX-License-Identifier: GPL-3.0-or-later
//! Cold source-compatible autofold for warped loose faces. Document coordinates
//! remain f64; only the source planarity and angular decisions use Qt floats.
use crate::mesh::{Mesh, MeshFace};
use glam::{DVec2, Vec3};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct FoldReport {
    pub folded_history_ids: Vec<u64>,
    pub created_history_ids: Vec<u64>,
}

fn qlength(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let n = Vec3::new(v.x / m, v.y / m, v.z / m);
    m * (n.x * n.x + n.y * n.y + n.z * n.z).sqrt()
}
fn qnormalized(v: Vec3) -> Vec3 {
    let len = qlength(v);
    if (len - 1.).abs() < 1e-5 {
        return v;
    }
    if len.abs() < 1e-5 {
        return Vec3::ZERO;
    }
    Vec3::new(v.x / len, v.y / len, v.z / len)
}
fn planar(mesh: &Mesh, face: &MeshFace) -> bool {
    let points = face
        .vertices
        .iter()
        .chain(face.holes.iter().flatten())
        .map(|&v| mesh.vertices[v].position.as_vec3())
        .collect::<Vec<_>>();
    if points.len() <= 3 {
        return true;
    }
    let origin = points[0];
    let mut normal = None;
    'outer: for i in 1..points.len() - 1 {
        for j in i + 1..points.len() {
            let c = (points[i] - origin).cross(points[j] - origin);
            if qlength(c) > 1e-6 {
                normal = Some(qnormalized(c));
                break 'outer;
            }
        }
    }
    normal.is_some_and(|n| points.iter().all(|&p| n.dot(p - origin).abs() <= 1e-3))
}
/// Cold predicate: callers need not retain a topology snapshot for planar moves.
pub fn needs_fold(mesh: &Mesh) -> bool {
    mesh.faces.iter().any(|f| !planar(mesh, f))
}
fn normal(mesh: &Mesh, ring: &[usize]) -> Vec3 {
    let mut xyz = [0.; 3];
    for i in 0..ring.len() {
        let a = mesh.vertices[ring[i]].position.as_vec3();
        let b = mesh.vertices[ring[(i + 1) % ring.len()]].position.as_vec3();
        xyz[0] += (a.y as f64 - b.y as f64) * (a.z as f64 + b.z as f64);
        xyz[1] += (a.z as f64 - b.z as f64) * (a.x as f64 + b.x as f64);
        xyz[2] += (a.x as f64 - b.x as f64) * (a.y as f64 + b.y as f64);
    }
    {
        let v = Vec3::new(xyz[0] as f32, xyz[1] as f32, xyz[2] as f32);
        let len = qlength(v);
        if len < 1e-9 {
            Vec3::Z
        } else {
            Vec3::new(v.x / len, v.y / len, v.z / len)
        }
    }
}
fn triangles(mesh: &Mesh, f: &MeshFace) -> Result<Vec<Vec<usize>>, String> {
    let n = qnormalized(normal(mesh, &f.vertices));
    let mut u = Vec3::X - n * n.dot(Vec3::X);
    if qlength(u) < 0.1 {
        u = Vec3::Y - n * n.dot(Vec3::Y);
    }
    u = qnormalized(u);
    let v = qnormalized(n.cross(u));
    let origin = mesh.vertices[f.vertices[0]].position.as_vec3();
    let project = |id: usize| {
        let p = mesh.vertices[id].position.as_vec3() - origin;
        DVec2::new(p.dot(u) as f64, p.dot(v) as f64)
    };
    let mut ids = f.vertices.clone();
    let mut points = ids.iter().map(|&id| project(id)).collect::<Vec<_>>();
    if f.holes.is_empty()
        && crate::triangulate::is_convex(&points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
    {
        return Ok((1..ids.len() - 1)
            .map(|i| vec![ids[0], ids[i], ids[i + 1]])
            .collect());
    }
    let mut holes = Vec::new();
    for h in &f.holes {
        if h.len() >= 3 {
            holes.push(ids.len());
            ids.extend(h);
            points.extend(h.iter().map(|&id| project(id)));
        }
    }
    Ok(crate::drafting_earcut::earcut(&points, &holes)?
        .into_iter()
        .map(|t| t.into_iter().map(|i| ids[i]).collect())
        .collect())
}
fn splice(a: &[usize], b: &[usize]) -> Option<Vec<usize>> {
    for i in 0..a.len() {
        for j in 0..b.len() {
            if a[i] == b[(j + 1) % b.len()] && a[(i + 1) % a.len()] == b[j] {
                let mut r = (0..a.len() - 1)
                    .map(|k| a[(i + 1 + k) % a.len()])
                    .collect::<Vec<_>>();
                r.extend((0..b.len() - 1).map(|k| b[(j + 1 + k) % b.len()]));
                if r.iter().copied().collect::<HashSet<_>>().len() == r.len() {
                    return Some(r);
                }
            }
        }
    }
    None
}
/// Atomic cold publication. Untouched vertices, edges and faces retain their
/// original order, attributes and runtime identity. New pieces are fresh faces.
pub fn fold_nonplanar_faces(mesh: &mut Mesh) -> Result<FoldReport, String> {
    for f in &mesh.faces {
        for &id in f.vertices.iter().chain(f.holes.iter().flatten()) {
            let p = mesh
                .vertices
                .get(id)
                .ok_or("Autofold face references a missing vertex")?
                .position;
            if !p.is_finite() || !p.as_vec3().is_finite() {
                return Err("Autofold requires finite source-compatible coordinates".into());
            }
        }
    }
    let ids = mesh
        .faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (!planar(mesh, f)).then_some(i))
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(FoldReport::default());
    }
    let mut candidate = mesh.clone();
    let mut report = FoldReport::default();
    let mut replacements = Vec::new();
    for id in ids {
        let mother = &mesh.faces[id];
        let triangles = triangles(mesh, mother)?;
        if triangles.len() < 2 {
            continue;
        }
        let mut pieces = triangles;
        loop {
            let mut joined = None;
            'search: for a in 0..pieces.len() {
                for b in a + 1..pieces.len() {
                    if normal(mesh, &pieces[a]).dot(normal(mesh, &pieces[b])) > 0.999 {
                        if let Some(r) = splice(&pieces[a], &pieces[b]) {
                            joined = Some((a, b, r));
                            break 'search;
                        }
                    }
                }
            }
            let Some((a, b, r)) = joined else {
                break;
            };
            pieces.remove(b);
            pieces.remove(a);
            pieces.push(r);
        }
        report.folded_history_ids.push(mother.history_id);
        replacements.push((id, pieces));
    }
    if replacements.is_empty() {
        return Ok(report);
    }
    let removed = replacements
        .iter()
        .map(|(id, _)| *id)
        .collect::<HashSet<_>>();
    candidate.faces = mesh
        .faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (!removed.contains(&i)).then_some(f.clone()))
        .collect();
    candidate.rebuild_incidence();
    for (id, pieces) in replacements {
        for ring in pieces {
            let f = candidate
                .add_face(ring, vec![])
                .ok_or("Autofold produced an invalid face")?;
            candidate.faces[f].attrs = mesh.faces[id].attrs.clone();
            candidate.faces[f].layer = mesh.faces[id].layer.clone();
            report
                .created_history_ids
                .push(candidate.faces[f].history_id);
        }
    }
    candidate.rebuild_incidence();
    *mesh = candidate;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    #[test]
    fn folds_one_corner_without_replacing_neighbor_or_boundary() {
        let mut m = Mesh::new();
        let ids = [[0., 0., 0.], [4., 0., 0.], [4., 4., 1.], [0., 4., 0.]]
            .into_iter()
            .map(|p| m.add_vertex(DVec3::from_array(p)))
            .collect();
        let f = m.add_face(ids, vec![]).unwrap();
        m.faces[f]
            .attrs
            .insert("vendor".into(), serde_json::json!(71));
        let v = [
            m.add_vertex(DVec3::new(10., 0., 0.)),
            m.add_vertex(DVec3::new(11., 0., 0.)),
            m.add_vertex(DVec3::new(10., 1., 0.)),
        ];
        m.add_face(v.to_vec(), vec![]);
        let old = m.clone();
        let report = fold_nonplanar_faces(&mut m).unwrap();
        assert_eq!(report.folded_history_ids, vec![old.faces[0].history_id]);
        assert_eq!(m.faces.len(), 3);
        assert_eq!(m.faces[0].history_id, old.faces[1].history_id);
        assert_eq!(
            m.vertices
                .iter()
                .map(|v| (v.uid, v.position))
                .collect::<Vec<_>>(),
            old.vertices
                .iter()
                .map(|v| (v.uid, v.position))
                .collect::<Vec<_>>()
        );
        for e in &old.edges {
            assert!(m.edges.iter().any(|v| v.history_id == e.history_id));
        }
        assert!(m.faces[1..]
            .iter()
            .all(|f| f.attrs.get("vendor") == Some(&serde_json::json!(71))));
        assert!(!needs_fold(&m));
    }
    #[test]
    fn invalid_coordinate_refuses_atomically() {
        let mut m = Mesh::new();
        let v = [
            m.add_vertex(DVec3::ZERO),
            m.add_vertex(DVec3::X),
            m.add_vertex(DVec3::Y),
        ];
        m.add_face(v.to_vec(), vec![]);
        m.vertices[0].position.x = f64::INFINITY;
        let old = m.clone();
        assert!(fold_nonplanar_faces(&mut m).is_err());
        assert_eq!(m, old);
    }
}
