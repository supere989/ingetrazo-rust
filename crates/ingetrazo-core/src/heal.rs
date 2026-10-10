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
fn orient_closed_components(mesh: &mut Mesh) -> Result<(), String> {
    let mut orientation = vec![None; mesh.faces.len()];
    let mut components = vec![];
    for start in 0..mesh.faces.len() {
        if orientation[start].is_some() {
            continue;
        }
        orientation[start] = Some(false);
        let mut queue = std::collections::VecDeque::from([start]);
        let mut component = vec![];
        let mut closed = true;
        while let Some(f) = queue.pop_front() {
            component.push(f);
            for ring in std::iter::once(&mesh.faces[f].vertices).chain(&mesh.faces[f].holes) {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    let Some(edge) = mesh.find_edge(a, b) else {
                        closed = false;
                        continue;
                    };
                    let e = &mesh.edges[edge];
                    if e.faces.len() != 2 {
                        closed = false;
                        continue;
                    }
                    let g = *e.faces.iter().find(|&&g| g != f).unwrap();
                    let same = std::iter::once(&mesh.faces[g].vertices)
                        .chain(&mesh.faces[g].holes)
                        .any(|r| (0..r.len()).any(|i| r[i] == a && r[(i + 1) % r.len()] == b));
                    let wanted = orientation[f].unwrap() ^ same;
                    if let Some(old) = orientation[g] {
                        if old != wanted {
                            return Err("Non-orientable face network".into());
                        }
                    } else {
                        orientation[g] = Some(wanted);
                        queue.push_back(g);
                    }
                }
            }
        }
        if closed {
            components.push(component);
        } else {
            for f in component {
                orientation[f] = Some(false);
            }
        }
    }
    for (i, flip) in orientation.into_iter().enumerate() {
        if flip == Some(true) {
            mesh.faces[i].vertices.reverse();
            for h in &mut mesh.faces[i].holes {
                h.reverse();
            }
        }
    }
    for component in components {
        let signed: f64 = component
            .iter()
            .flat_map(|&i| mesh.triangulate_face(i))
            .map(|(a, b, c)| a.dot(b.cross(c)) / 6.)
            .sum();
        if signed < 0. {
            for i in component {
                mesh.faces[i].vertices.reverse();
                for h in &mut mesh.faces[i].holes {
                    h.reverse();
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
