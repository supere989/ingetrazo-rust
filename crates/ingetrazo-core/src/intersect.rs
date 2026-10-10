// SPDX-License-Identifier: GPL-3.0-or-later
//! Face crossings clipped against actual polygon rings (including holes).
use crate::{
    groups::Group,
    mesh::Mesh,
    planar::{self, Frame},
    scene::{Scene, SelectionItem},
};
use glam::{DMat4, DVec3};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug)]
pub enum IntersectMode {
    WithModel,
    WithSelection,
    WithContext,
}
#[derive(Clone, Debug)]
pub struct WorldPoly {
    pub rings: Vec<Vec<DVec3>>,
    pub normal: DVec3,
    plane: f64,
}
pub fn world_polys(mesh: &Mesh, face: usize, transform: DMat4) -> Result<WorldPoly, String> {
    planar::validate_mesh(mesh)?;
    if !transform.is_finite() || transform.determinant().abs() < 1e-12 {
        return Err("Invalid group transform".into());
    }
    let face = mesh.faces.get(face).ok_or("Invalid intersection face")?;
    let rings: Vec<Vec<_>> = planar::rings(mesh, face)
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|p| transform.transform_point3(p))
                .collect()
        })
        .collect();
    let frame = Frame::new(&rings[0])?;
    if rings
        .iter()
        .flatten()
        .any(|&p| !p.is_finite() || !frame.on_plane(p))
    {
        return Err("Intersect Faces requires planar finite faces".into());
    }
    let plane = rings[0][0].dot(frame.normal);
    Ok(WorldPoly {
        rings,
        normal: frame.normal,
        plane,
    })
}
fn intervals(poly: &WorldPoly, origin: DVec3, direction: DVec3) -> Vec<(f64, f64)> {
    let u = poly
        .normal
        .cross(if poly.normal.z.abs() < 0.8 {
            DVec3::Z
        } else {
            DVec3::Y
        })
        .normalize();
    let v = poly.normal.cross(u);
    let ox = origin.dot(u);
    let oy = origin.dot(v);
    let dx = direction.dot(u);
    let dy = direction.dot(v);
    let den = dx * dx + dy * dy;
    let mut ts = vec![];
    for ring in &poly.rings {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let (ax, ay) = (a.dot(u), a.dot(v));
            let (bx, by) = (b.dot(u), b.dot(v));
            let sa = (ax - ox) * dy - (ay - oy) * dx;
            let sb = (bx - ox) * dy - (by - oy) * dx;
            if (sa >= 0.) == (sb >= 0.) {
                continue;
            }
            let k = sa / (sa - sb);
            ts.push(((ax + (bx - ax) * k - ox) * dx + (ay + (by - ay) * k - oy) * dy) / den);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.chunks_exact(2).map(|p| (p[0], p[1])).collect()
}
pub fn cross(a: &WorldPoly, b: &WorldPoly) -> Vec<(DVec3, DVec3)> {
    let direction = a.normal.cross(b.normal);
    if direction.length() < 1e-9 {
        return vec![];
    }
    let direction = direction.normalize();
    let c = a.normal.dot(b.normal);
    let den = 1. - c * c;
    if den < 1e-15 {
        return vec![];
    }
    let origin =
        a.normal * ((a.plane - b.plane * c) / den) + b.normal * ((b.plane - a.plane * c) / den);
    let mut segments = vec![];
    for (x0, x1) in intervals(a, origin, direction) {
        for (y0, y1) in intervals(b, origin, direction) {
            let lo = x0.max(y0);
            let hi = x1.min(y1);
            if hi - lo > 1e-5 {
                segments.push((origin + direction * lo, origin + direction * hi));
            }
        }
    }
    segments
}
fn group_polys(group: &Group, parent: DMat4, out: &mut Vec<WorldPoly>) -> Result<(), String> {
    let transform = parent * group.transform;
    for i in 0..group.mesh.faces.len() {
        out.push(world_polys(&group.mesh, i, transform)?);
    }
    for child in &group.children {
        group_polys(child, transform, out)?;
    }
    Ok(())
}
pub fn segments_for(scene: &Scene, mode: IntersectMode) -> Result<Vec<(DVec3, DVec3)>, String> {
    planar::validate_mesh(&scene.mesh)?;
    let mut mine = vec![];
    let mut rest = vec![];
    for i in 0..scene.mesh.faces.len() {
        let p = world_polys(&scene.mesh, i, DMat4::IDENTITY)?;
        if scene.selection.contains(&SelectionItem::Face(i)) {
            mine.push(p);
        } else {
            rest.push(p);
        }
    }
    for (id, group) in scene.groups.iter().enumerate() {
        if scene.selection.contains(&SelectionItem::Group(id)) {
            group_polys(group, DMat4::IDENTITY, &mut mine)?;
        } else if matches!(mode, IntersectMode::WithModel) {
            group_polys(group, DMat4::IDENTITY, &mut rest)?;
        }
    }
    let mut all = vec![];
    if matches!(mode, IntersectMode::WithSelection) {
        for i in 0..mine.len() {
            for j in i + 1..mine.len() {
                all.extend(cross(&mine[i], &mine[j]));
            }
        }
    } else {
        for a in &mine {
            for b in &rest {
                all.extend(cross(a, b));
            }
        }
    }
    let mut seen = BTreeSet::new();
    all.retain(|&(p, q)| {
        let key = |p: DVec3| {
            (
                (p.x * 1e6).round() as i64,
                (p.y * 1e6).round() as i64,
                (p.z * 1e6).round() as i64,
            )
        };
        let (a, b) = (key(p), key(q));
        seen.insert(if a < b { (a, b) } else { (b, a) })
    });
    Ok(all)
}
/// Prepare all loose-face subdivisions and crossing edges before committing.
pub fn intersect_faces(scene: &mut Scene, mode: IntersectMode) -> Result<usize, String> {
    let segments = segments_for(scene, mode)?;
    if segments.is_empty() {
        return Ok(0);
    }
    let original = &scene.mesh;
    let mut result = Mesh::new();
    for face in &original.faces {
        let rings = planar::rings(original, face);
        let frame = Frame::new(&rings[0])?;
        let projected: Vec<Vec<_>> = rings
            .iter()
            .map(|r| r.iter().map(|&p| frame.project(p)).collect())
            .collect();
        let cuts: Vec<_> = segments
            .iter()
            .filter(|(a, b)| frame.on_plane(*a) && frame.on_plane(*b))
            .map(|&(a, b)| (frame.project(a), frame.project(b)))
            .collect();
        if cuts.is_empty() {
            planar::add_face(&mut result, &rings, face)?;
            continue;
        }
        let mut lines = cuts;
        for r in &projected {
            for i in 0..r.len() {
                lines.push((r[i], r[(i + 1) % r.len()]));
            }
        }
        let cells = planar::arrange_2d(&lines)?;
        let mut area = 0.;
        for (outer, holes) in cells {
            if !planar::inside_rings(planar::sample(&outer), &projected) {
                continue;
            }
            let mut output = vec![outer.iter().map(|&p| frame.unproject(p)).collect()];
            output.extend(
                holes
                    .iter()
                    .map(|r| r.iter().map(|&p| frame.unproject(p)).collect()),
            );
            let id = planar::add_face(&mut result, &output, face)?;
            area += result.face_area(id);
        }
        let expected = original.face_area(
            original
                .faces
                .iter()
                .position(|f| std::ptr::eq(f, face))
                .unwrap(),
        );
        if (area - expected).abs() > expected.max(1.) * 1e-6 {
            return Err(format!(
                "Intersection subdivision changed face area: expected {expected}, result {area}"
            ));
        }
    }
    for &(a, b) in &segments {
        let a = result.add_vertex(a);
        let b = result.add_vertex(b);
        result.add_edge(a, b);
    }
    planar::preserve_edges(&mut result, original);
    let result = planar::stitch(&result)?;
    planar::validate_mesh(&result)?;
    if result == *original {
        return Ok(0);
    }
    scene.mesh = result;
    scene.clear_selection();
    scene.bump_version();
    Ok(segments.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn face(mesh: &mut Mesh, points: &[DVec3], holes: Vec<Vec<DVec3>>) -> usize {
        let ring = points.iter().map(|&p| mesh.add_vertex(p)).collect();
        let holes = holes
            .iter()
            .map(|h| h.iter().map(|&p| mesh.add_vertex(p)).collect())
            .collect();
        mesh.add_face(ring, holes).unwrap()
    }
    #[test]
    fn crossing_holes_coplanar_and_split_are_real() {
        let mut scene = Scene::new();
        face(
            &mut scene.mesh,
            &[
                DVec3::ZERO,
                DVec3::new(4., 0., 0.),
                DVec3::new(4., 4., 0.),
                DVec3::new(0., 4., 0.),
            ],
            vec![vec![
                DVec3::new(1., 1., 0.),
                DVec3::new(1., 3., 0.),
                DVec3::new(3., 3., 0.),
                DVec3::new(3., 1., 0.),
            ]],
        );
        face(
            &mut scene.mesh,
            &[
                DVec3::new(2., -1., -1.),
                DVec3::new(2., 5., -1.),
                DVec3::new(2., 5., 1.),
                DVec3::new(2., -1., 1.),
            ],
            vec![],
        );
        let a = world_polys(&scene.mesh, 0, DMat4::IDENTITY).unwrap();
        let b = world_polys(&scene.mesh, 1, DMat4::IDENTITY).unwrap();
        let cross = cross(&a, &b);
        assert_eq!(cross.len(), 2);
        assert!((cross.iter().map(|(a, b)| a.distance(*b)).sum::<f64>() - 2.).abs() < 1e-8);
        scene.selection.insert(SelectionItem::Face(0));
        scene.selection.insert(SelectionItem::Face(1));
        assert_eq!(
            intersect_faces(&mut scene, IntersectMode::WithSelection).unwrap(),
            2
        );
        assert!(scene.mesh.faces.len() > 2);
        assert!(
            (scene
                .mesh
                .faces
                .iter()
                .enumerate()
                .map(|(i, _)| scene.mesh.face_area(i))
                .sum::<f64>()
                - 24.)
                .abs()
                < 1e-8
        );
    }
}
