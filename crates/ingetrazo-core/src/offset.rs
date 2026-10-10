// SPDX-License-Identifier: GPL-3.0-or-later
//! Parallel offsets using the original slid-edge, limited-mitre arrangement.
use crate::{
    mesh::Mesh,
    planar::{self, Frame, PlanarCell},
};
use glam::{DVec2, DVec3};
const MITRE_LIMIT: f64 = 2.;
fn frame(points: &[DVec3], normal: DVec3, distance: f64) -> Result<Frame, String> {
    if points.is_empty()
        || !distance.is_finite()
        || !normal.is_finite()
        || points.iter().any(|p| !p.is_finite())
    {
        return Err("Offset requires finite coordinates and distance".into());
    }
    let normal = normal.normalize_or_zero();
    if normal.length_squared() < 0.5 {
        return Err("Offset requires a valid plane normal".into());
    }
    let u = normal
        .cross(if normal.z.abs() < 0.8 {
            DVec3::Z
        } else {
            DVec3::Y
        })
        .normalize();
    let f = Frame {
        origin: points[0],
        normal,
        u,
        v: normal.cross(u),
    };
    if points.iter().any(|&p| !f.on_plane(p)) {
        return Err("Offset run must be planar".into());
    }
    Ok(f)
}
fn intersection(a: DVec2, da: DVec2, b: DVec2, db: DVec2) -> Option<DVec2> {
    let denom = da.perp_dot(db);
    if denom.abs() < 1e-9 {
        None
    } else {
        Some(a + da * ((b - a).perp_dot(db) / denom))
    }
}
fn lines(points: &[DVec2], closed: bool, d: f64) -> Vec<(DVec2, DVec2, DVec2)> {
    let count = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    (0..count)
        .filter_map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            let edge = b - a;
            if edge.length() < 1e-9 {
                return None;
            }
            let dir = edge.normalize();
            let push = DVec2::new(-dir.y, dir.x) * d;
            Some((a + push, b + push, dir))
        })
        .collect()
}
/// Signed side offset of an open planar chain, including limited mitre/bevel joints.
pub fn offset_chain(points: &[DVec3], normal: DVec3, d: f64) -> Result<Vec<DVec3>, String> {
    let f = frame(points, normal, d)?;
    let points: Vec<_> = points.iter().map(|&p| f.project(p)).collect();
    let ls = lines(&points, false, d);
    if ls.is_empty() {
        return Err("Offset chain needs two distinct points".into());
    }
    let mut out = vec![ls[0].0];
    for i in 0..ls.len() - 1 {
        let (_, end, dir) = ls[i];
        let (start, _, next) = ls[i + 1];
        match intersection(end, dir, start, next)
            .filter(|&p| (p - end).length() <= MITRE_LIMIT * d.abs() + 1e-9)
        {
            Some(p) => out.push(p),
            None => {
                out.push(end);
                out.push(start);
            }
        }
    }
    out.push(ls.last().unwrap().1);
    Ok(out.into_iter().map(|p| f.unproject(p)).collect())
}
/// Surviving regions after a signed loop offset (positive along normal-cross-edge).
pub fn offset_regions(points: &[DVec3], normal: DVec3, d: f64) -> Result<Vec<PlanarCell>, String> {
    let f = frame(points, normal, d)?;
    if points.len() < 3 || d.abs() < 1e-9 {
        return Ok(vec![]);
    }
    let original: Vec<_> = points.iter().map(|&p| f.project(p)).collect();
    let ls = lines(&original, true, d);
    if ls.len() < 3 {
        return Ok(vec![]);
    }
    let corners: Vec<_> = (0..ls.len())
        .map(|i| {
            let (_, end, dir) = ls[i];
            let (start, _, next) = ls[(i + 1) % ls.len()];
            intersection(end, dir, start, next)
                .filter(|&p| (p - end).length() <= MITRE_LIMIT * d.abs() + 1e-9)
        })
        .collect();
    let mut segments = vec![];
    for i in 0..ls.len() {
        let (start, end, _) = ls[i];
        segments.push((
            corners[(i + ls.len() - 1) % ls.len()].unwrap_or(start),
            corners[i].unwrap_or(end),
        ));
        if corners[i].is_none() {
            segments.push((end, ls[(i + 1) % ls.len()].0));
        }
    }
    let mut kept = vec![];
    for (outer, holes) in planar::arrange_2d(&segments)? {
        // Match the original largest-triangle centroid classification, not the
        // vertex average (which can lie outside a concave polygon).
        let mut temp = Mesh::new();
        let ids = outer
            .iter()
            .map(|&p| temp.add_vertex(f.unproject(p)))
            .collect();
        let Some(id) = temp.add_face(ids, vec![]) else {
            continue;
        };
        let point = temp
            .triangulate_face(id)
            .into_iter()
            .max_by(|(a, b, c), (x, y, z)| {
                (b - a)
                    .cross(*c - *a)
                    .length_squared()
                    .total_cmp(&(y - x).cross(*z - *x).length_squared())
            })
            .map(|(a, b, c)| f.project((a + b + c) / 3.));
        let Some(point) = point else {
            continue;
        };
        let mut depth = (0..original.len())
            .map(|i| {
                let a = original[i];
                let ab = original[(i + 1) % original.len()] - a;
                let t = if ab.length_squared() < 1e-18 {
                    0.
                } else {
                    ((point - a).dot(ab) / ab.length_squared()).clamp(0., 1.)
                };
                (point - a - ab * t).length()
            })
            .fold(f64::INFINITY, f64::min);
        if !planar::inside_ring(point, &original) {
            depth = -depth;
        }
        if depth < if d > 0. { d * 0.75 } else { d } {
            continue;
        }
        kept.push(PlanarCell {
            outer: outer.into_iter().map(|p| f.unproject(p)).collect(),
            holes: holes
                .into_iter()
                .map(|r| r.into_iter().map(|p| f.unproject(p)).collect())
                .collect(),
        });
    }
    Ok(kept)
}
/// Prepare a face offset without mutating its source. Existing holes stay void,
/// while every resulting ring/tile inherits an independent copy of its paint.
pub fn offset_face(mesh: &Mesh, face_id: usize, d: f64) -> Result<Mesh, String> {
    planar::validate_mesh(mesh)?;
    let source = mesh.faces.get(face_id).ok_or("Invalid Offset face")?;
    let source_rings = planar::rings(mesh, source);
    let f = Frame::new(&source_rings[0])?;
    let regions = offset_regions(&source_rings[0], f.normal, d)?;
    if regions.is_empty() {
        return Err("This distance consumes the face; use a smaller offset".into());
    }
    let mut boundaries = source_rings.clone();
    for r in &regions {
        boundaries.push(r.outer.clone());
        boundaries.extend(r.holes.clone());
    }
    let segs: Vec<_> = boundaries
        .iter()
        .flat_map(|r| (0..r.len()).map(move |i| (f.project(r[i]), f.project(r[(i + 1) % r.len()]))))
        .collect();
    let domain: Vec<Vec<_>> = if d > 0. {
        source_rings
            .iter()
            .map(|r| r.iter().map(|&p| f.project(p)).collect())
            .collect()
    } else {
        let biggest = regions
            .iter()
            .max_by(|a, b| {
                let area = |r: &Vec<DVec3>| crate::math::newell_vector(r).length();
                area(&a.outer).total_cmp(&area(&b.outer))
            })
            .unwrap();
        std::iter::once(&biggest.outer)
            .chain(source_rings[1..].iter())
            .map(|r| r.iter().map(|&p| f.project(p)).collect())
            .collect()
    };
    let mut out = Mesh::new();
    for (i, face) in mesh.faces.iter().enumerate() {
        if i != face_id {
            planar::add_face(&mut out, &planar::rings(mesh, face), face)?;
        }
    }
    for (outer, holes) in planar::arrange_2d(&segs)? {
        if !planar::inside_rings(planar::sample(&outer), &domain) {
            continue;
        }
        let rings: Vec<_> = std::iter::once(outer)
            .chain(holes)
            .map(|r| r.into_iter().map(|p| f.unproject(p)).collect())
            .collect();
        planar::add_face(&mut out, &rings, source)?;
    }
    planar::preserve_edges(&mut out, mesh);
    let mut out = planar::stitch(&out)?;
    // One-to-one curve identity is mirrored only while every original corner
    // survives, exactly the original tool's curve tagging boundary.
    if regions.len() == 1 && regions[0].outer.len() == source_rings[0].len() {
        let mut next = mesh
            .edges
            .iter()
            .filter_map(|e| e.curve_id)
            .max()
            .map_or(0, |x| x + 1);
        let mut curves = std::collections::BTreeMap::new();
        for edge in &mut out.edges {
            let a = out.vertices[edge.v0].position;
            let b = out.vertices[edge.v1].position;
            if !regions[0].outer.iter().any(|p| p.distance(a) < 1e-6)
                || !regions[0].outer.iter().any(|p| p.distance(b) < 1e-6)
            {
                continue;
            }
            let dir = (b - a).normalize_or_zero();
            for i in 0..source.vertices.len() {
                let v0 = source.vertices[i];
                let v1 = source.vertices[(i + 1) % source.vertices.len()];
                let x = mesh.vertices[v0].position;
                let y = mesh.vertices[v1].position;
                let sd = (y - x).normalize_or_zero();
                if dir.dot(sd).abs() < 0.999999 {
                    continue;
                }
                let shift = f.normal.cross(sd) * d;
                if (a - x - shift).cross(sd).length() > 1e-6
                    || (b - x - shift).cross(sd).length() > 1e-6
                {
                    continue;
                }
                if let Some(old) = mesh.find_edge(v0, v1).map(|id| &mesh.edges[id]) {
                    edge.soft = old.soft;
                    edge.layer = old.layer.clone();
                    edge.hidden = old.hidden;
                    edge.attrs = old.attrs.clone();
                    edge.curve_id = old.curve_id.map(|id| {
                        *curves.entry(id).or_insert_with(|| {
                            let n = next;
                            next += 1;
                            n
                        })
                    });
                }
                break;
            }
        }
    }
    out.preserve_vertex_uids_from(mesh);
    planar::validate_mesh(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn points(xy: &[(f64, f64)]) -> Vec<DVec3> {
        xy.iter().map(|&(x, y)| DVec3::new(x, y, 0.)).collect()
    }
    fn area(r: &[DVec3]) -> f64 {
        crate::math::newell_vector(r).length() * 0.5
    }
    #[test]
    fn original_region_collapse_split_and_reported_downward_plaza() {
        let dumb = points(&[
            (0., 0.),
            (4., 0.),
            (4., 1.7),
            (6., 1.7),
            (6., 0.),
            (10., 0.),
            (10., 4.),
            (6., 4.),
            (6., 2.3),
            (4., 2.3),
            (4., 4.),
            (0., 4.),
        ]);
        assert_eq!(offset_regions(&dumb, DVec3::Z, 0.25).unwrap().len(), 1);
        let halves = offset_regions(&dumb, DVec3::Z, 0.4).unwrap();
        assert_eq!(halves.len(), 2);
        for r in halves {
            assert!((area(&r.outer) - 10.24).abs() < 0.05);
        }
        let kerb = points(&[(0., 0.), (2.4, 0.), (2.4, 0.2), (0., 0.2)]);
        for d in [0.1, 0.5] {
            assert!(offset_regions(&kerb, DVec3::Z, d).unwrap().is_empty());
        }
        let plaza = points(&[
            (0., 0.),
            (0., 2.4),
            (3.8167, 2.4),
            (3.6874, 2.1498),
            (3.5576, 1.7338),
            (3.5021, 1.3015),
            (3.5224, 0.8662),
            (3.6180, 0.4411),
            (3.7860, 0.0390),
            (3.8109, 0.),
        ]);
        for d in [0.05, 0.1, 0.2, 0.3] {
            let rs = offset_regions(&plaza, -DVec3::Z, d).unwrap();
            assert_eq!(rs.len(), 1);
            assert!(area(&rs[0].outer) > area(&plaza) * 0.5);
            assert!(area(&rs[0].outer) < area(&plaza));
        }
    }
    #[test]
    fn painted_holes_tile_without_overlap_and_invalid_inputs_are_atomic() {
        let mut m = Mesh::new();
        let outer = points(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]);
        let hole = points(&[(4., 4.), (6., 4.), (6., 6.), (4., 6.)]);
        let ids = outer.iter().map(|&p| m.add_vertex(p)).collect();
        let hs = hole.iter().map(|&p| m.add_vertex(p)).collect();
        let id = m.add_face(ids, vec![hs]).unwrap();
        m.faces[id]
            .attrs
            .insert("mat".into(), serde_json::json!("Stone"));
        for d in [1., -1.] {
            let out = offset_face(&m, id, d).unwrap();
            let total: f64 = (0..out.faces.len()).map(|i| out.face_area(i)).sum();
            let expected = if d > 0. { 96. } else { 140. };
            assert!((total - expected).abs() < 1e-7, "{total}");
            assert!(out.faces.iter().all(|f| f.attrs == m.faces[0].attrs));
        }
        let before = m.clone();
        for d in [f64::NAN, f64::INFINITY, 5.] {
            assert!(offset_face(&m, 0, d).is_err());
            assert_eq!(m, before);
        }
    }
    #[test]
    fn open_chain_mitre_bevel_and_plane_guard() {
        let p = points(&[(0., 0.), (10., 0.), (10., 10.)]);
        assert_eq!(
            offset_chain(&p, DVec3::Z, 1.).unwrap(),
            points(&[(0., 1.), (9., 1.), (9., 10.)])
        );
        let mut off = p.clone();
        off[2].z = 1.;
        assert!(offset_chain(&off, DVec3::Z, 1.).is_err());
    }
    #[test]
    fn surviving_dimension_anchors_and_curve_tags_follow_the_offset() {
        use crate::{dimensions::Dimension, scene::Scene};
        let mut scene = Scene::new();
        let ps = points(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]);
        let ids = ps.iter().map(|&p| scene.mesh.add_vertex(p)).collect();
        scene.mesh.add_face(ids, vec![]);
        for e in &mut scene.mesh.edges {
            e.curve_id = Some(7);
            e.soft = true;
            e.attrs.insert("arc".into(), serde_json::json!("source"));
        }
        scene
            .dimensions
            .push(Dimension::new(DVec3::ZERO, DVec3::X * 10., DVec3::Y));
        scene.bind_dimensions();
        let held = scene.mesh.vertices[0].uid;
        scene.mesh = offset_face(&scene.mesh, 0, 1.).unwrap();
        let v = scene
            .mesh
            .vertices
            .iter_mut()
            .find(|v| v.uid == held)
            .unwrap();
        v.position.x += 0.5;
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].start.x, 0.5);
        let curve: Vec<_> = scene
            .mesh
            .edges
            .iter()
            .filter(|e| e.curve_id == Some(8))
            .collect();
        assert_eq!(curve.len(), 4);
        assert!(curve.iter().all(|e| e.soft && e.attrs["arc"] == "source"));
    }
}
