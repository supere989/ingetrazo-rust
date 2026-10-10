// SPDX-License-Identifier: GPL-3.0-or-later
//! Material-compatible edge-connected facet simplification with a root-normal drift guard.
use crate::{
    mesh::{Mesh, MeshFace},
    planar::{self, Frame},
};
use glam::DVec3;
use std::collections::{BTreeMap, HashMap};
fn compatible(a: &MeshFace, b: &MeshFace) -> bool {
    if a.layer != b.layer {
        return false;
    }
    if a.attrs == b.attrs {
        return true;
    }
    let mut aa = a.attrs.clone();
    let mut bb = b.attrs.clone();
    let ta = aa.remove("texture");
    let tb = bb.remove("texture");
    if aa != bb {
        return false;
    }
    let (Some(ta), Some(tb)) = (ta, tb) else {
        return false;
    };
    // An omitted path is not a shared resource: embedded image identities
    // and all persisted mapping metadata must also agree.
    let mut metadata_a = ta.clone();
    let mut metadata_b = tb.clone();
    if let Some(map) = metadata_a.as_object_mut() {
        map.remove("uvw");
    }
    if let Some(map) = metadata_b.as_object_mut() {
        map.remove("uvw");
    }
    if metadata_a != metadata_b {
        return false;
    }
    let (Some(ua), Some(ub)) = (
        ta.get("uvw").and_then(|v| v.as_array()),
        tb.get("uvw").and_then(|v| v.as_array()),
    ) else {
        return false;
    };
    ua.len() == ub.len()
        && ua
            .iter()
            .zip(ub)
            .all(|(a, b)| match (a.as_f64(), b.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() <= 0.02 + 0.01 * a.abs().max(b.abs()),
                _ => false,
            })
}
// The source kernel stores QVector3D normals in float32. Near the zero-angle
// threshold its normalization/dot rounding decides whether triangles fuse.
fn source_normal(mesh: &Mesh, id: usize) -> DVec3 {
    let ring = &mesh.faces[id].vertices;
    let mut n = glam::Vec3::ZERO;
    for i in 0..ring.len() {
        let a = mesh.vertices[ring[i]].position;
        let b = mesh.vertices[ring[(i + 1) % ring.len()]].position;
        n += glam::Vec3::new(
            ((a.y - b.y) * (a.z + b.z)) as f32,
            ((a.z - b.z) * (a.x + b.x)) as f32,
            ((a.x - b.x) * (a.y + b.y)) as f32,
        );
    }
    let length = n.length();
    if length < 1e-10 {
        DVec3::ZERO
    } else {
        (n / length).as_dvec3()
    }
}
fn source_dot(a: DVec3, b: DVec3) -> f64 {
    a.as_vec3().dot(b.as_vec3()) as f64
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
fn collapse(mut ring: Vec<DVec3>) -> Vec<DVec3> {
    loop {
        if ring.len() <= 3 {
            break;
        }
        let remove = (0..ring.len()).find(|&i| {
            let a = ring[i] - ring[(i + ring.len() - 1) % ring.len()];
            let b = ring[(i + 1) % ring.len()] - ring[i];
            a.length() > 1e-4 && b.length() > 1e-4 && a.normalize().dot(b.normalize()) >= 0.999
        });
        if let Some(i) = remove {
            ring.remove(i);
        } else {
            break;
        }
    }
    ring
}
/// Prepare simplification. A nonzero facet angle can change curved surfaces;
/// no new coordinates are introduced and protected holes/partitions stay intact.
pub fn simplify_mesh(
    mesh: &Mesh,
    max_angle_degrees: f64,
    principal_planes: bool,
) -> Result<(Mesh, usize), String> {
    if !max_angle_degrees.is_finite() || !(0. ..=5.).contains(&max_angle_degrees) {
        return Err("Simplification angle must be between zero and five degrees".into());
    }
    planar::validate_mesh(mesh)?;
    if mesh.faces.is_empty() {
        return Ok((mesh.clone(), 0));
    }
    let mut eligible = vec![];
    let mut triangles = BTreeMap::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        if !f.holes.is_empty() || f.interior {
            continue;
        }
        if f.vertices.len() == 3 {
            let mut keys: Vec<_> = f
                .vertices
                .iter()
                .map(|&v| crate::math::PointKey::from_vec3(mesh.vertices[v].position))
                .collect();
            keys.sort();
            let rank = |f: &MeshFace| {
                if f.attrs.contains_key("texture") {
                    2
                } else if !f.attrs.is_empty() {
                    1
                } else {
                    0
                }
            };
            if let Some(&old) = triangles.get(&keys) {
                if rank(f) > rank(&mesh.faces[eligible[old]]) {
                    eligible[old] = i;
                }
                continue;
            }
            triangles.insert(keys, eligible.len());
        }
        eligible.push(i);
    }
    let normals: Vec<_> = eligible.iter().map(|&i| source_normal(mesh, i)).collect();
    let mut parent: Vec<_> = (0..eligible.len()).collect();
    let mut edges = BTreeMap::<(usize, usize), Vec<usize>>::new();
    let mut edge_order = vec![];
    for (j, &i) in eligible.iter().enumerate() {
        let ring = &mesh.faces[i].vertices;
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            let key = (a.min(b), a.max(b));
            if !edges.contains_key(&key) {
                edge_order.push(key);
            }
            edges.entry(key).or_default().push(j);
        }
    }
    let cos = max_angle_degrees.to_radians().max(0.00045).cos();
    for key in edge_order {
        let adjacent = &edges[&key];
        if adjacent.len() != 2 {
            continue;
        }
        let (a, b) = (adjacent[0], adjacent[1]);
        let principal = |n: DVec3| n.abs().max_element() >= 1. - 1e-7;
        if principal_planes && (!principal(normals[a]) || !principal(normals[b])) {
            continue;
        }
        if source_dot(normals[a], normals[b]).abs() < cos
            || !compatible(&mesh.faces[eligible[a]], &mesh.faces[eligible[b]])
        {
            continue;
        }
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        if ra != rb
            && source_dot(normals[ra], normals[rb]).abs() >= cos
            && compatible(&mesh.faces[eligible[ra]], &mesh.faces[eligible[rb]])
        {
            parent[rb] = ra;
        }
    }
    let mut regions = BTreeMap::<usize, Vec<usize>>::new();
    for j in 0..eligible.len() {
        regions
            .entry(root(&mut parent, j))
            .or_default()
            .push(eligible[j]);
    }
    let mut out = Mesh::new();
    for ids in regions.values() {
        let source = &mesh.faces[ids[0]];
        let mut boundary = HashMap::<(usize, usize), usize>::new();
        for &id in ids {
            let ring = &mesh.faces[id].vertices;
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                *boundary.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let f = Frame::new(&planar::rings(mesh, source)[0])?;
        let segs: Vec<_> = boundary
            .iter()
            .filter(|(_, count)| **count == 1)
            .map(|(&(a, b), _)| {
                (
                    f.project(mesh.vertices[a].position),
                    f.project(mesh.vertices[b].position),
                )
            })
            .collect();
        let original: Vec<_> = ids
            .iter()
            .flat_map(|&i| mesh.faces[i].vertices.iter().copied())
            .map(|i| mesh.vertices[i].position)
            .collect();
        let prepared = (|| -> Result<Vec<Vec<Vec<DVec3>>>, String> {
            let mut cells = vec![];
            for (outer, holes) in planar::arrange_2d(&segs)? {
                let mut rings = vec![];
                for ring in std::iter::once(outer).chain(holes) {
                    let points = ring
                        .iter()
                        .map(|&p| {
                            original
                                .iter()
                                .copied()
                                .find(|&q| f.project(q).distance(p) < 1e-6)
                                .ok_or("Simplification introduced a crossing vertex")
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    rings.push(collapse(points));
                }
                cells.push(rings);
            }
            if cells.is_empty() {
                return Err("No valid merged boundary".into());
            }
            Ok(cells)
        })();
        let mut candidate = out.clone();
        let success = prepared.and_then(|cells| {
            for cell in cells {
                planar::add_face(&mut candidate, &cell, source)?;
            }
            Ok(())
        });
        if success.is_ok() {
            out = candidate;
        } else {
            for &id in ids {
                planar::add_face(
                    &mut out,
                    &planar::rings(mesh, &mesh.faces[id]),
                    &mesh.faces[id],
                )?;
            }
        }
    }
    for face in &mesh.faces {
        if !face.holes.is_empty() || face.interior {
            planar::add_face(&mut out, &planar::rings(mesh, face), face)?;
        }
    }
    out = planar::stitch_boundaries(&out)?;
    // Original simplification rebuilds surfaces and drops orphan wire geometry.
    for edge in &mut out.edges {
        let a = out.vertices[edge.v0].position;
        let b = out.vertices[edge.v1].position;
        if let Some(old) = mesh.edges.iter().find(|e| {
            let x = mesh.vertices[e.v0].position;
            let y = mesh.vertices[e.v1].position;
            (a.distance(x) < 1e-6 && b.distance(y) < 1e-6)
                || (a.distance(y) < 1e-6 && b.distance(x) < 1e-6)
        }) {
            edge.attrs = old.attrs.clone();
            edge.soft = old.soft;
            edge.curve_id = old.curve_id;
            edge.layer = old.layer.clone();
            edge.hidden = old.hidden;
        }
    }
    for i in 0..out.edges.len() {
        let edge = &out.edges[i];
        if edge.faces.len() == 2 {
            let d = out
                .face_normal(edge.faces[0])
                .dot(out.face_normal(edge.faces[1]))
                .abs();
            if d > 0.85 {
                out.edges[i].soft = true;
            }
        }
    }
    out.preserve_vertex_uids_from(mesh);
    planar::validate_mesh(&out)?;
    let removed = mesh.faces.len().saturating_sub(out.faces.len());
    Ok((out, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn grid(plane: usize) -> Mesh {
        let mut m = Mesh::new();
        let p = |x: f64, y: f64| match plane {
            0 => DVec3::new(x, y, 0.),
            1 => DVec3::new(x, 0., y),
            2 => DVec3::new(0., x, y),
            _ => DVec3::new(x, y, x + y),
        };
        for x in 0..12 {
            for y in 0..8 {
                let (x, y) = (x as f64, y as f64);
                let v: [usize; 4] = [p(x, y), p(x + 1., y), p(x + 1., y + 1.), p(x, y + 1.)]
                    .map(|p| m.add_vertex(p));
                m.add_face(vec![v[0], v[1], v[2]], vec![]);
                m.add_face(vec![v[0], v[2], v[3]], vec![]);
            }
        }
        m
    }
    #[test]
    fn original_planar_grid_all_orientations_collapses_to_four_corners() {
        for plane in 0..4 {
            let m = grid(plane);
            let (out, removed) = simplify_mesh(&m, 0., false).unwrap();
            assert_eq!(removed, 191);
            assert_eq!(out.faces.len(), 1);
            assert_eq!(out.vertices.len(), 4);
            assert!(
                (out.face_area(0) - (0..m.faces.len()).map(|i| m.face_area(i)).sum::<f64>()).abs()
                    < 1e-7
            );
        }
    }
    #[test]
    fn gentle_curvature_and_material_seams_are_preserved() {
        let mut m = Mesh::new();
        for i in 0..90 {
            let a = std::f64::consts::FRAC_PI_2 * i as f64 / 90.;
            let b = std::f64::consts::FRAC_PI_2 * (i + 1) as f64 / 90.;
            let v = [
                DVec3::new(10. * a.cos(), 10. * a.sin(), 0.),
                DVec3::new(10. * b.cos(), 10. * b.sin(), 0.),
                DVec3::new(10. * b.cos(), 10. * b.sin(), 2.),
                DVec3::new(10. * a.cos(), 10. * a.sin(), 2.),
            ]
            .map(|p| m.add_vertex(p));
            m.add_face(vec![v[0], v[1], v[2]], vec![]);
            m.add_face(vec![v[0], v[2], v[3]], vec![]);
        }
        let (out, removed) = simplify_mesh(&m, 1., false).unwrap();
        assert!(removed > 90);
        assert!(out.faces.len() < 90);
        assert!(out
            .vertices
            .iter()
            .all(|v| m.vertices.iter().any(|old| old.position == v.position)));
        let mut m = Mesh::new();
        let v = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.), DVec3::Y].map(|p| m.add_vertex(p));
        m.add_face(vec![v[0], v[1], v[2]], vec![]);
        m.add_face(vec![v[0], v[2], v[3]], vec![]);
        m.faces[0].layer = Some("one".into());
        m.faces[1].layer = Some("two".into());
        assert_eq!(simplify_mesh(&m, 0., false).unwrap().0.faces.len(), 2);
        for d in [f64::NAN, -1., 6.] {
            assert!(simplify_mesh(&m, d, false).is_err());
        }
    }
    #[test]
    fn differing_embedded_texture_resources_never_merge() {
        let mut m = Mesh::new();
        let v = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.), DVec3::Y].map(|p| m.add_vertex(p));
        m.add_face(vec![v[0], v[1], v[2]], vec![]);
        m.add_face(vec![v[0], v[2], v[3]], vec![]);
        let uv = serde_json::json!([1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.]);
        m.faces[0].attrs.insert(
            "texture".into(),
            serde_json::json!({"embed":"first_image","uvw":uv}),
        );
        m.faces[1].attrs.insert(
            "texture".into(),
            serde_json::json!({"embed":"second_image","uvw":uv}),
        );
        assert_eq!(simplify_mesh(&m, 0., false).unwrap().0.faces.len(), 2);
        m.faces[1].attrs = m.faces[0].attrs.clone();
        assert_eq!(simplify_mesh(&m, 0., false).unwrap().0.faces.len(), 1);
    }
}
