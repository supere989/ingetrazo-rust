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
    let paint_only=explicit.is_some_and(|ring|mesh.faces.iter().any(|face|{
        let loops=planar::rings(mesh,face);let Ok(frame)=planar::Frame::new(&loops[0])else{return false;};
        ring.iter().all(|&p|frame.on_plane(p)&&(planar::inside_rings(frame.project(p),&loops.iter().map(|r|r.iter().map(|&p|frame.project(p)).collect()).collect::<Vec<_>>())||loops.iter().any(|r|(0..r.len()).any(|i|on_segment(p,r[i],r[(i+1)%r.len()])))))
    }));
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
            if e.curve_id.is_some() {
                continue;
            }
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
    let mut remember = |point: DVec3, normal: DVec3| {
        if normal.length_squared() > 0.5
            && !planes
                .iter()
                .any(|&(p, n)| n.dot(normal).abs() > 1. - 1e-7 && (point - p).dot(n).abs() < 1e-6)
        {
            planes.push((point, normal));
        }
    };
    // A cycle through a new subedge must leave it through an incident edge.
    // Those neighbors enumerate its possible planes without scanning arbitrary
    // vertices throughout unrelated 3D content.
    for &(a, b) in segments {
        if a.distance(b) < 1e-9 {
            continue;
        }
        for edge in &out.edges {
            if !on_segment(out.vertices[edge.v0].position, a, b)
                || !on_segment(out.vertices[edge.v1].position, a, b)
            {
                continue;
            }
            for endpoint in [edge.v0, edge.v1] {
                for &neighbor in &out.vertices[endpoint].edges {
                    let e = &out.edges[neighbor];
                    let other = if e.v0 == endpoint { e.v1 } else { e.v0 };
                    remember(
                        a,
                        (b - a)
                            .cross(out.vertices[other].position - a)
                            .normalize_or_zero(),
                    );
                }
            }
        }
        // An interior chord can remain disconnected from the mother's boundary.
        for (id, face) in out.faces.iter().enumerate() {
            let p = out.vertices[face.vertices[0]].position;
            let n = out.face_normal(id);
            if (a - p).dot(n).abs() < 1e-6 && (b - p).dot(n).abs() < 1e-6 {
                remember(p, n);
            }
        }
    }
    // Source curve drafting performs one global arrangement on whole-flat
    // drawings, including disconnected old curves and deleted bounded cells.
    if let Some(first) = out.vertices.first() {
        let origin = first.position;
        if let Some(axis) = out
            .vertices
            .iter()
            .map(|v| v.position - origin)
            .find(|d| d.length_squared() > 1e-12)
        {
            if let Some(normal) = out
                .vertices
                .iter()
                .map(|v| axis.cross(v.position - origin).normalize_or_zero())
                .find(|n| n.length_squared() > 0.5)
            {
                if out
                    .vertices
                    .iter()
                    .all(|v| (v.position - origin).dot(normal).abs() < 1e-6)
                {
                    remember(origin, normal);
                }
            }
        }
    }
    drop(remember);
    if planes.len() > 4096 {
        return Err("Drawing meets too many distinct planes for an interactive edit".into());
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
        if plane.edges.len() < 3 {
            continue;
        }
        let rebuilt = planar::rebuild_faces(&plane, &[])?;
        if affected.is_empty() && rebuilt.faces.is_empty() {
            continue;
        }
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
                let mut source=f.clone();
                if paint_only&&explicit.is_some_and(|ring|ring.iter().all(|p|(*p-point).dot(normal).abs()<1e-6)&&planar::inside_ring(frame.project(sample),&ring.iter().map(|&p|frame.project(p)).collect::<Vec<_>>())){
                    // AddFaceCommand's newly authored rectangle inherits only
                    // PAINT_KEYS. Opaque metadata belongs to the mother and
                    // remains on her remainder pieces, unlike a chord split.
                    source.attrs.retain(|key,_|matches!(key.as_str(),"color"|"mat"|"texture"|"opacity"));
                }
                planar::add_face(&mut result, &loops, &source)?;
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
    #[test]fn authored_rectangle_band_inherits_paint_but_not_mother_metadata(){
        let mut m=square();m.faces[0].attrs.insert("color".into(),serde_json::json!([1.,0.,0.]));m.faces[0].attrs.insert("vendor".into(),serde_json::json!(71));let ring=[DVec3::new(0.25,0.,0.),DVec3::new(0.5,0.,0.),DVec3::new(0.5,1.,0.),DVec3::new(0.25,1.,0.)];let segments=(0..4).map(|i|(ring[i],ring[(i+1)%4])).collect::<Vec<_>>();let out=add_segments(&m,&segments,Some(&ring),None).unwrap();assert_eq!(out.faces.len(),3);assert_eq!(out.faces.iter().filter(|f|f.attrs.contains_key("vendor")).count(),2);assert!(out.faces.iter().all(|f|f.attrs.get("color")==m.faces[0].attrs.get("color")));
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
    #[test]fn original_old_disconnected_curve_rebuilds_deleted_flat_region(){
        let mut m=square();m.faces.clear();for e in &mut m.edges{e.faces.clear();}for v in &mut m.vertices{v.position.x+=10.;}
        let a=m.add_vertex(DVec3::ZERO);let b=m.add_vertex(DVec3::X);let c=m.add_vertex(DVec3::X+DVec3::Y);for (a,b) in [(a,b),(b,c)]{let id=m.add_edge(a,b).unwrap();m.edges[id].curve_id=Some(71);}
        let out=add_segments(&m,&[(DVec3::new(20.,0.,0.),DVec3::new(21.,0.,0.))],None,None).unwrap();assert_eq!(out.faces.len(),1);assert!((out.face_area(0)-1.).abs()<1e-9);
    }
    #[test]fn unrelated_spatial_vertices_do_not_supply_drafting_planes(){
        let mut m=Mesh::new();for i in 0..5000{m.add_vertex(DVec3::new(i as f64+10.,(i*i) as f64+10.,(i%17) as f64+1.));}let uid=m.vertices[4999].uid;
        let out=add_segments(&m,&[(DVec3::ZERO,DVec3::X)],None,None).unwrap();assert_eq!(out.edges.len(),1);assert!(out.faces.is_empty());assert!(out.vertices.iter().any(|v|v.uid==uid));
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
