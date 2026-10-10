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
    let maximum = n.abs().max_element();
    let scaled = if maximum > 0. { n / maximum } else { n };
    // Qt qHypot(float,float,float) uses the scaled std::hypot path,
    // not sqrt(dot(n,n)); that rounding matters at the source threshold.
    let length = maximum * (scaled.x * scaled.x + scaled.y * scaled.y + scaled.z * scaled.z).sqrt();
    if length < 1e-10 {
        DVec3::ZERO
    } else {
        (n / length).as_dvec3()
    }
}
fn source_dot(a: DVec3, b: DVec3) -> f64 {
    let (a, b) = (a.as_vec3(), b.as_vec3());
    (a.x * b.x + a.y * b.y + a.z * b.z) as f64
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
// Compatibility target: 64-bit CPython 3.13 used by the retained original
// oracle. Numeric hashes are stable there, but this is not a promise of the
// same contour iteration on every Python version or word size.
// CPython's numeric tuple/frozenset iteration determines where the source
// boundary walk starts. Corner collapse is order-dependent on curved loops,
// so retain that deterministic source order rather than substituting a new one.
fn float_hash(x: f64) -> u64 {
    // Python round(x,4) rounds the exact binary value at a decimal digit,
    // ties to even; multiplying first can erase which side of a tie it was.
    let x = format!("{x:.4}").parse::<f64>().unwrap_or(x);
    if x == 0. {
        return 0;
    }
    let bits = x.abs().to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) | if exp == 0 { 0 } else { 1u64 << 52 };
    let exponent = if exp == 0 { -1074 } else { exp - 1023 - 52 };
    let shift = exponent.rem_euclid(61) as u32;
    let modulus = (1u64 << 61) - 1;
    let value = mantissa % modulus;
    let hash = if shift == 0 {
        value
    } else {
        ((value << shift) & modulus) | (value >> (61 - shift))
    };
    let hash = if x < 0. {
        (hash as i64).wrapping_neg() as u64
    } else {
        hash
    };
    if hash == u64::MAX {
        u64::MAX - 1
    } else {
        hash
    }
}
fn point_hash(p: DVec3) -> u64 {
    let mut acc = 2870177450012600261u64;
    for x in p.to_array() {
        acc = acc.wrapping_add(float_hash(x).wrapping_mul(14029467366897019727));
        acc = acc.rotate_left(31).wrapping_mul(11400714785074694791);
    }
    acc = acc.wrapping_add(3 ^ (2870177450012600261u64 ^ 3527539));
    if acc == u64::MAX {
        1546275796
    } else {
        acc
    }
}
fn edge_hash(a: u64, b: u64) -> u64 {
    let shuffle = |h: u64| ((h ^ 89869747) ^ (h << 16)).wrapping_mul(3644798167);
    let mut h = shuffle(a) ^ shuffle(b);
    h ^= 3u64.wrapping_mul(1927868237);
    h ^= (h >> 11) ^ (h >> 25);
    h = h.wrapping_mul(69069).wrapping_add(907133923);
    if h == u64::MAX {
        590923713
    } else {
        h
    }
}
fn insert_slot(table: &mut [Option<usize>], hashes: &[u64], key: usize) {
    let mask = table.len() - 1;
    let mut perturb = hashes[key];
    let mut i = perturb as usize & mask;
    loop {
        let probes = if i + 9 <= mask { 9 } else { 0 };
        for j in i..=i + probes {
            if table[j].is_none() {
                table[j] = Some(key);
                return;
            }
        }
        perturb >>= 5;
        i = (i
            .wrapping_mul(5)
            .wrapping_add(1)
            .wrapping_add(perturb as usize))
            & mask;
    }
}
fn source_set_slots(hashes: &[u64]) -> Vec<usize> {
    let mut table = vec![None; 8];
    for key in 0..hashes.len() {
        insert_slot(&mut table, hashes, key);
        let used = key + 1;
        if used * 5 >= (table.len() - 1) * 3 {
            let mut size = 8;
            while size <= used * 4 {
                size *= 2;
            }
            let mut next = vec![None; size];
            for key in table.into_iter().flatten() {
                insert_slot(&mut next, hashes, key);
            }
            table = next;
        }
    }
    let mut slots = vec![0; hashes.len()];
    for (slot, key) in table.iter().enumerate() {
        if let Some(key) = key {
            slots[*key] = slot;
        }
    }
    slots
}
fn source_walk_start(
    mut ring: Vec<DVec3>,
    boundary: &[(DVec3, DVec3)],
    slots: &[usize],
) -> Vec<DVec3> {
    let edge = boundary
        .iter()
        .enumerate()
        .filter_map(|(i, &(a, b))| {
            let x = ring.iter().position(|p| p.distance(a) < 1e-7)?;
            let y = ring.iter().position(|p| p.distance(b) < 1e-7)?;
            if (x + 1) % ring.len() != y && (y + 1) % ring.len() != x {
                return None;
            }
            Some((slots[i], a, b, x, y))
        })
        .min_by_key(|x| x.0);
    if let Some((_, a, b, x, y)) = edge {
        let h = [point_hash(a), point_hash(b)];
        let tiny = source_set_slots(&h);
        let (first, second) = if tiny[0] < tiny[1] { (x, y) } else { (y, x) };
        let start = if (first + 1) % ring.len() == second {
            first
        } else {
            (first + 1) % ring.len()
        };
        ring.rotate_left(start);
    }
    ring
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
        let mut boundary_order = vec![];
        for &id in ids {
            let ring = &mesh.faces[id].vertices;
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                let key = (a.min(b), a.max(b));
                if !boundary.contains_key(&key) {
                    boundary_order.push((a, b));
                }
                *boundary.entry(key).or_default() += 1;
            }
        }
        let source_boundary: Vec<_> = boundary_order
            .iter()
            .filter(|&&(a, b)| boundary[&(a.min(b), a.max(b))] == 1)
            .map(|&(a, b)| (mesh.vertices[a].position, mesh.vertices[b].position))
            .collect();
        let hashes: Vec<_> = source_boundary
            .iter()
            .map(|&(a, b)| edge_hash(point_hash(a), point_hash(b)))
            .collect();
        let slots = source_set_slots(&hashes);
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
                // A boundary arrangement also reports the bounded hole cell.
                // Merge only regions covered by an original facet; otherwise
                // triangulated annuli silently acquire a second, filled disc.
                let probe=planar::sample(&outer);
                if !ids.iter().any(|&id|{
                    let ring=mesh.faces[id].vertices.iter().map(|&v|f.project(mesh.vertices[v].position)).collect::<Vec<_>>();
                    planar::inside_ring(probe,&ring)
                }) {continue;}
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
                    rings.push(collapse(source_walk_start(
                        points,
                        &source_boundary,
                        &slots,
                    )));
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
    fn pinned_cpython_numeric_hash_and_resize_iteration_controls() {
        // Values independently queried from CPython 3.13, including true binary
        // half-grid ties and decimal values just to either side of a tie.
        for (x, expected) in [
            (0.03125, 71942301887467248i64),
            (-0.03125, -71942301887467248),
            (0.09375, 216288074264244480),
            (-0.09375, -216288074264244480),
            (0.00005, 936979306793984537),
            (-0.00005, -936979306793984537),
            (0.00015, 936979306793984537),
            (0.00025, 288922129054475852),
        ] {
            assert_eq!(float_hash(x) as i64, expected);
        }
        let a = DVec3::new(1.2345, -6.789, 2.);
        let b = DVec3::new(-3.1416, 0., 8.7654);
        assert_eq!(point_hash(a) as i64, -3642640755035674506);
        assert_eq!(point_hash(b), 1390104798643358996);
        assert_eq!(edge_hash(point_hash(a), point_hash(b)), 1462378670174784654);
        let hashes: Vec<_> = (0..40)
            .map(|i| {
                let x = i as f64 / 10.;
                edge_hash(
                    point_hash(DVec3::new(x, 0., 0.)),
                    point_hash(DVec3::new(x, 1., 0.)),
                )
            })
            .collect();
        let slots = source_set_slots(&hashes);
        let mut order: Vec<_> = (0..40).collect();
        order.sort_by_key(|&i| slots[i]);
        assert_eq!(
            order,
            vec![
                2, 18, 13, 35, 19, 26, 7, 8, 17, 27, 20, 1, 36, 16, 25, 28, 22, 31, 11, 21, 33, 10,
                23, 5, 39, 6, 24, 32, 37, 4, 38, 0, 14, 15, 12, 34, 3, 30, 9, 29
            ]
        );
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

#[cfg(test)]mod annular_coverage_tests{
 use super::*;use glam::DVec3;
 #[test]fn eight_annular_triangles_merge_without_filling_the_void(){
  let mut m=Mesh::new();let outer=[(0.,0.),(4.,0.),(4.,4.),(0.,4.)];let inner=[(1.,1.),(3.,1.),(3.,3.),(1.,3.)];let o=outer.map(|(x,y)|m.add_vertex(DVec3::new(x,y,0.)));let h=inner.map(|(x,y)|m.add_vertex(DVec3::new(x,y,0.)));
  for i in 0..4{let j=(i+1)%4;m.add_face(vec![o[i],o[j],h[j]],vec![]);m.add_face(vec![o[i],h[j],h[i]],vec![]);}
  let (out,_)=simplify_mesh(&m,0.,false).unwrap();assert_eq!(out.faces.len(),1);assert_eq!(out.faces[0].holes.len(),1);assert!((out.face_area(0)-12.).abs()<1e-9);
 }
}
