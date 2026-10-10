//! Atomic segment drafting shared by line, rectangle and curve tools.
use crate::{mesh::Mesh, planar};
use glam::DVec3;
fn on_segment(p: DVec3, a: DVec3, b: DVec3) -> bool {
    let d = b - a;
    let t = (p - a).dot(d) / d.length_squared();
    t >= -1e-7 && t <= 1. + 1e-7 && (p - a - d * t).length() < 1e-6
}
/// Add ordered drawing segments, optionally owning an explicit face or curve.
/// Only spatial crossings split; generated faces retain mother attributes.
pub fn add_segments(
    mesh: &Mesh,
    segments: &[(DVec3, DVec3)],
    explicit: Option<&[DVec3]>,
    curve: Option<usize>,
) -> Result<Mesh, String> {
    planar::validate_mesh(mesh)?;
    if segments
        .iter()
        .any(|(a, b)| !a.is_finite() || !b.is_finite())
        || explicit.is_some_and(|r| r.iter().any(|p| !p.is_finite()))
    {
        return Err("Drawing coordinates must be finite".into());
    }
    let mut out = mesh.clone();
    let mut targets = vec![];
    for &(a, b) in segments {
        if a.distance(b) < 1e-9 {
            continue;
        }
        let va = out.add_vertex(a);
        let vb = out.add_vertex(b);
        let id = out.add_edge(va, vb).ok_or("Drawing edge collapsed")?;
        targets.push(id);
    }
    out = planar::split_edge_intersections(&out, &targets)?;
    if let Some(ring) = explicit {
        let ids = ring.iter().map(|&p| out.add_vertex(p)).collect();
        out.add_face(ids, vec![]).ok_or("Invalid drawing face")?;
        out = planar::stitch_boundaries(&out)?;
    }
    if let Some(id) = curve {
        for e in &mut out.edges {
            if e.curve_id.is_some(){continue;}
            let a = out.vertices[e.v0].position;
            let b = out.vertices[e.v1].position;
            if segments
                .iter()
                .any(|&(s, t)| on_segment(a, s, t) && on_segment(b, s, t))
            {
                e.curve_id = Some(id);
            }
        }
    }
    let mut planes: Vec<(DVec3, DVec3)> = vec![];
    for &(a, b) in segments {
        for vertex in &out.vertices {
            let n = (b - a).cross(vertex.position - a).normalize_or_zero();
            if n.length_squared() < 0.5 {
                continue;
            }
            if !planes
                .iter()
                .any(|&(p, m)| m.dot(n).abs() > 1. - 1e-7 && (a - p).dot(m).abs() < 1e-6)
            {
                planes.push((a, n));
            }
        }
    }
    for (point, normal) in planes {
        // The explicit footprint supplies coverage; it does not auto-face
        // unrelated cycles. Contained bands split their mother even in 3D.
        let flat = out
            .vertices
            .iter()
            .all(|v| (v.position - point).dot(normal).abs() < 1e-6);
        let affected: Vec<_> = out
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                planar::rings(&out, f)
                    .iter()
                    .flatten()
                    .all(|p| (*p - point).dot(normal).abs() < 1e-6)
            })
            .map(|(i, _)| i)
            .collect();
        let mut plane = Mesh::new();
        for &i in &affected {
            planar::add_face(
                &mut plane,
                &planar::rings(&out, &out.faces[i]),
                &out.faces[i],
            )?;
        }
        for e in &out.edges {
            let a = out.vertices[e.v0].position;
            let b = out.vertices[e.v1].position;
            if (a - point).dot(normal).abs() < 1e-6 && (b - point).dot(normal).abs() < 1e-6 {
                let a = plane.add_vertex(a);
                let b = plane.add_vertex(b);
                plane.add_edge(a, b);
            }
        }
        if plane.edges.len()<3 {continue;}
        let rebuilt = planar::rebuild_faces(&plane, &[])?;
        if affected.is_empty()&&rebuilt.faces.is_empty(){continue;}
        let mut result = Mesh::new();
        for (i, f) in out.faces.iter().enumerate() {
            if !affected.contains(&i) {
                planar::add_face(&mut result, &planar::rings(&out, f), f)?;
            }
        }
        for f in &rebuilt.faces {
            let loops = planar::rings(&rebuilt, f);
            let frame = planar::Frame::new(&loops[0])?;
            let sample = frame.unproject(planar::sample(
                &loops[0]
                    .iter()
                    .map(|&p| frame.project(p))
                    .collect::<Vec<_>>(),
            ));
            let covered = affected.iter().any(|&i| {
                let rings = planar::rings(&out, &out.faces[i]);
                let rings = rings
                    .iter()
                    .map(|r| r.iter().map(|&p| frame.project(p)).collect())
                    .collect::<Vec<_>>();
                planar::inside_rings(frame.project(sample), &rings)
            });
            let touched = loops.iter().any(|ring| {
                (0..ring.len()).any(|i| {
                    segments.iter().any(|&(a, b)| {
                        on_segment(ring[i], a, b) && on_segment(ring[(i + 1) % ring.len()], a, b)
                    })
                })
            });
            let full_curve = flat && out.edges.iter().any(|e| e.curve_id.is_some());
            if covered || touched && explicit.is_none() || full_curve {
                planar::add_face(&mut result, &loops, f)?;
            }
        }
        planar::preserve_edges(&mut result, &out);
        out = result;
    }
    out.preserve_vertex_uids_from(mesh);
    planar::validate_mesh(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square() -> Mesh {
        let mut m = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| m.add_vertex(p));
        let f = m.add_face(ids.to_vec(), vec![]).unwrap();
        m.faces[f]
            .attrs
            .insert("paint".into(), serde_json::json!("brick"));
        m
    }
    #[test]
    fn diagonal_splits_painted_mother_into_two_halves() {
        let m = square();
        let out = add_segments(&m, &[(DVec3::ZERO, DVec3::X + DVec3::Y)], None, None).unwrap();
        assert_eq!(out.faces.len(), 2);
        assert!(out.faces.iter().all(|f| f.attrs == m.faces[0].attrs));
        assert!(
            (out.faces
                .iter()
                .enumerate()
                .map(|(i, _)| out.face_area(i))
                .sum::<f64>()
                - 1.)
                .abs()
                < 1e-9
        );
    }
    #[test]
    fn faceless_diagonal_closes_both_sides() {
        let mut m = square();
        m.faces.clear();
        for e in &mut m.edges {
            e.faces.clear();
        }
        let out = add_segments(&m, &[(DVec3::ZERO, DVec3::X + DVec3::Y)], None, None).unwrap();
        assert_eq!(out.faces.len(), 2);
    }
    #[test]
    fn drawing_elsewhere_does_not_resurrect_deleted_region() {
        let mut m = square();
        m.faces.clear();
        for e in &mut m.edges {
            e.faces.clear();
        }
        let out = add_segments(
            &m,
            &[(DVec3::new(3., 0., 0.), DVec3::new(4., 0., 0.))],
            None,
            None,
        )
        .unwrap();
        assert!(out.faces.is_empty());
    }
    #[test]
    fn nonfinite_batch_is_atomic() {
        let m = square();
        assert!(add_segments(
            &m,
            &[(DVec3::ZERO, DVec3::new(f64::NAN, 1., 0.))],
            None,
            None
        )
        .is_err());
        assert_eq!(m.faces.len(), 1);
    }
}
