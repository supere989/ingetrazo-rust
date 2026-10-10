// SPDX-License-Identifier: GPL-3.0-or-later
//! Plane arrangements: crossing segments are split before bounded cells walk.
use crate::{
    math::newell_vector,
    mesh::{Mesh, MeshFace},
};
use glam::{DVec2, DVec3};
use std::collections::{BTreeMap, BTreeSet};
const TOL: f64 = 1e-7;
#[derive(Clone, Debug)]
pub struct PlanarCell {
    pub outer: Vec<DVec3>,
    pub holes: Vec<Vec<DVec3>>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub origin: DVec3,
    pub normal: DVec3,
    pub u: DVec3,
    pub v: DVec3,
}
impl Frame {
    pub(crate) fn new(ring: &[DVec3]) -> Result<Self, String> {
        let normal = newell_vector(ring).normalize_or_zero();
        if ring.len() < 3 || normal.length_squared() < 0.5 {
            return Err("Degenerate face plane".into());
        }
        let origin = ring[0];
        let u = normal
            .cross(if normal.z.abs() < 0.8 {
                DVec3::Z
            } else {
                DVec3::Y
            })
            .normalize();
        let v = normal.cross(u);
        Ok(Self {
            origin,
            normal,
            u,
            v,
        })
    }
    pub(crate) fn project(self, p: DVec3) -> DVec2 {
        let p = p - self.origin;
        DVec2::new(p.dot(self.u), p.dot(self.v))
    }
    pub(crate) fn unproject(self, p: DVec2) -> DVec3 {
        self.origin + self.u * p.x + self.v * p.y
    }
    pub(crate) fn on_plane(self, p: DVec3) -> bool {
        (p - self.origin).dot(self.normal).abs() < 1e-6
    }
}
fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}
fn area(ring: &[DVec2]) -> f64 {
    (0..ring.len())
        .map(|i| cross(ring[i], ring[(i + 1) % ring.len()]))
        .sum::<f64>()
        * 0.5
}
fn key(p: DVec2) -> (i64, i64) {
    ((p.x / TOL).round() as i64, (p.y / TOL).round() as i64)
}
fn on_segment(p: DVec2, a: DVec2, b: DVec2) -> bool {
    let d = b - a;
    let t = (p - a).dot(d) / d.length_squared();
    t >= -TOL && t <= 1. + TOL && (p - a - d * t).length() < TOL
}
pub(crate) fn inside_ring(p: DVec2, ring: &[DVec2]) -> bool {
    let mut inside = false;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
pub(crate) fn inside_rings(p: DVec2, rings: &[Vec<DVec2>]) -> bool {
    rings
        .iter()
        .fold(false, |inside, r| inside ^ inside_ring(p, r))
}
pub(crate) fn sample(ring: &[DVec2]) -> DVec2 {
    // Left of a boundary edge, including concave cells.
    let i = (0..ring.len())
        .max_by(|&a, &b| {
            ring[a]
                .distance_squared(ring[(a + 1) % ring.len()])
                .total_cmp(&ring[b].distance_squared(ring[(b + 1) % ring.len()]))
        })
        .unwrap();
    let a = ring[i];
    let b = ring[(i + 1) % ring.len()];
    let d = (b - a).normalize();
    (a + b) * 0.5 + DVec2::new(-d.y, d.x) * ((b - a).length() * 1e-6).max(TOL * 2.)
}
pub(crate) fn arrange_2d(
    segments: &[(DVec2, DVec2)],
) -> Result<Vec<(Vec<DVec2>, Vec<Vec<DVec2>>)>, String> {
    if segments.len() > 20000 {
        return Err("Plane arrangement exceeds the interactive segment limit".into());
    }
    let lines: Vec<_> = segments
        .iter()
        .copied()
        .filter(|(a, b)| a.distance(*b) > TOL)
        .collect();
    let mut cuts: Vec<Vec<f64>> = vec![vec![0., 1.]; lines.len()];
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (a, b) = lines[i];
            let (c, d) = lines[j];
            let ab = b - a;
            let cd = d - c;
            let det = cross(ab, cd);
            if det.abs() > 1e-12 {
                let t = cross(c - a, cd) / det;
                let u = cross(c - a, ab) / det;
                if t >= -TOL && t <= 1. + TOL && u >= -TOL && u <= 1. + TOL {
                    cuts[i].push(t.clamp(0., 1.));
                    cuts[j].push(u.clamp(0., 1.));
                }
            } else {
                for p in [c, d] {
                    if on_segment(p, a, b) {
                        cuts[i].push(((p - a).dot(ab) / ab.length_squared()).clamp(0., 1.));
                    }
                }
                for p in [a, b] {
                    if on_segment(p, c, d) {
                        cuts[j].push(((p - c).dot(cd) / cd.length_squared()).clamp(0., 1.));
                    }
                }
            }
        }
    }
    let mut positions = BTreeMap::new();
    let mut edges = BTreeSet::new();
    for (i, &(a, b)) in lines.iter().enumerate() {
        cuts[i].sort_by(f64::total_cmp);
        cuts[i].dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        for pair in cuts[i].windows(2) {
            let p = a + (b - a) * pair[0];
            let q = a + (b - a) * pair[1];
            let (k, l) = (key(p), key(q));
            if k == l {
                continue;
            }
            positions.entry(k).or_insert(p);
            positions.entry(l).or_insert(q);
            edges.insert(if k < l { (k, l) } else { (l, k) });
        }
    }
    let mut adjacent = BTreeMap::<_, Vec<_>>::new();
    for &(a, b) in &edges {
        adjacent.entry(a).or_default().push(b);
        adjacent.entry(b).or_default().push(a);
    }
    for (&a, neighbors) in &mut adjacent {
        neighbors.sort_by(|b, c| {
            let b = positions[b] - positions[&a];
            let c = positions[c] - positions[&a];
            b.y.atan2(b.x).total_cmp(&c.y.atan2(c.x))
        });
    }
    let mut visited = BTreeSet::new();
    let mut cycles = vec![];
    for (&a, neighbors) in &adjacent {
        for &b in neighbors {
            if visited.contains(&(a, b)) {
                continue;
            }
            let mut chain = vec![];
            let mut edge = (a, b);
            for _ in 0..edges.len() * 2 + 1 {
                if visited.contains(&edge) {
                    break;
                }
                visited.insert(edge);
                chain.push(positions[&edge.0]);
                let ns = &adjacent[&edge.1];
                let i = ns.iter().position(|&v| v == edge.0).unwrap();
                edge = (edge.1, ns[(i + ns.len() - 1) % ns.len()]);
                if edge == (a, b) {
                    break;
                }
            }
            if edge == (a, b) {
                // Bridge edges are traversed twice by the same face walk.
                // Extract simple loops; a dangling cut remains a loose edge,
                // and a bridge to a hole does not turn its face into a slit.
                let mut pending = vec![chain];
                while let Some(chain) = pending.pop() {
                    if chain.len() < 3 {
                        continue;
                    }
                    let mut first = BTreeMap::new();
                    let mut repeat = None;
                    for (i, &p) in chain.iter().enumerate() {
                        if let Some(&j) = first.get(&key(p)) {
                            repeat = Some((j, i));
                            break;
                        }
                        first.insert(key(p), i);
                    }
                    if let Some((a, b)) = repeat {
                        pending.push(chain[a..b].to_vec());
                        let mut rest = chain[..a].to_vec();
                        rest.extend_from_slice(&chain[b..]);
                        pending.push(rest);
                    } else if area(&chain).abs() > TOL * TOL {
                        cycles.push(chain);
                    }
                }
            }
        }
    }
    let mut cells: Vec<_> = cycles
        .iter()
        .filter(|r| area(r) > 0.)
        .cloned()
        .map(|r| (r, Vec::new()))
        .collect();
    for hole in cycles.into_iter().filter(|r| area(r) < 0.) {
        let hole_area = -area(&hole);
        let p = sample(&hole);
        let target = (0..cells.len())
            .filter(|&i| area(&cells[i].0) > hole_area + TOL * TOL && inside_ring(p, &cells[i].0))
            .min_by(|&a, &b| area(&cells[a].0).total_cmp(&area(&cells[b].0)));
        if let Some(i) = target {
            cells[i].1.push(hole);
        }
    }
    Ok(cells)
}
pub fn validate_mesh(mesh: &Mesh) -> Result<(), String> {
    if mesh.vertices.iter().any(|v| !v.position.is_finite())
        || mesh.faces.iter().any(|f| {
            f.vertices.len() < 3
                || f.holes.iter().any(|r| r.len() < 3)
                || f.vertices
                    .iter()
                    .chain(f.holes.iter().flatten())
                    .any(|&v| v >= mesh.vertices.len())
        })
        || mesh.edges.iter().any(|e| {
            e.v0 >= mesh.vertices.len()
                || e.v1 >= mesh.vertices.len()
                || e.faces.iter().any(|&f| f >= mesh.faces.len())
        })
    {
        return Err("Invalid mesh coordinates or references".into());
    }
    Ok(())
}
pub(crate) fn rings(mesh: &Mesh, face: &MeshFace) -> Vec<Vec<DVec3>> {
    std::iter::once(&face.vertices)
        .chain(&face.holes)
        .map(|r| r.iter().map(|&v| mesh.vertices[v].position).collect())
        .collect()
}
pub(crate) fn add_face(
    mesh: &mut Mesh,
    rings: &[Vec<DVec3>],
    source: &MeshFace,
) -> Result<usize, String> {
    let vertices = rings[0].iter().map(|&p| mesh.add_vertex(p)).collect();
    let holes = rings[1..]
        .iter()
        .map(|r| r.iter().map(|&p| mesh.add_vertex(p)).collect())
        .collect();
    let id = mesh
        .add_face(vertices, holes)
        .ok_or("Arrangement face collapsed at document precision")?;
    let vertices = mesh.faces[id].vertices.clone();
    let holes = mesh.faces[id].holes.clone();
    mesh.faces[id] = source.clone();
    mesh.faces[id].vertices = vertices;
    mesh.faces[id].holes = holes;
    Ok(id)
}
pub(crate) fn preserve_edges(mesh: &mut Mesh, original: &Mesh) {
    for edge in &original.edges {
        let a = original.vertices[edge.v0].position;
        let b = original.vertices[edge.v1].position;
        let d = b - a;
        let length = d.length_squared();
        if length < TOL * TOL {
            continue;
        }
        if edge.faces.is_empty() {
            let mut stations = vec![(0., a), (1., b)];
            for vertex in &mesh.vertices {
                let p = vertex.position;
                let t = (p - a).dot(d) / length;
                if t > TOL && t < 1. - TOL && (p - a - d * t).length() < 1e-6 {
                    stations.push((t, p));
                }
            }
            stations.sort_by(|a, b| a.0.total_cmp(&b.0));
            stations.dedup_by(|a, b| (a.0 - b.0).abs() < TOL);
            for pair in stations.windows(2) {
                let a = mesh.add_vertex(pair[0].1);
                let b = mesh.add_vertex(pair[1].1);
                mesh.add_edge(a, b);
            }
        }
        for e in &mut mesh.edges {
            let p = mesh.vertices[e.v0].position;
            let q = mesh.vertices[e.v1].position;
            let on = |p: DVec3| {
                let t = (p - a).dot(d) / length;
                t >= -TOL && t <= 1. + TOL && (p - a - d * t).length() < 1e-6
            };
            if on(p) && on(q) {
                e.attrs = edge.attrs.clone();
                e.soft = edge.soft;
                e.curve_id = edge.curve_id;
                e.hidden = edge.hidden;
                e.layer = edge.layer.clone();
            }
        }
    }
    for v in &original.vertices {
        if v.edges.is_empty() {
            mesh.add_vertex(v.position);
        }
    }
    mesh.preserve_vertex_uids_from(original);
}
/// Rebuild all coplanar regions on one selected plane, retaining other planes.
pub fn rebuild_faces(mesh: &Mesh, selected: &[usize]) -> Result<Mesh, String> {
    validate_mesh(mesh)?;
    let frame = if mesh.faces.is_empty() {
        if mesh.vertices.len() < 3 || mesh.edges.len() < 3 {
            return Ok(mesh.clone());
        }
        let origin = mesh.vertices[0].position;
        let mut plane = None;
        'outer: for a in mesh.vertices.iter().skip(1) {
            for b in mesh.vertices.iter().skip(2) {
                if (a.position - origin).cross(b.position - origin).length() > 1e-9 {
                    plane = Some(Frame::new(&[origin, a.position, b.position])?);
                    break 'outer;
                }
            }
        }
        let Some(mut frame) = plane else {
            return Ok(mesh.clone());
        };
        let largest = frame.normal.abs().max_element();
        let sign = if frame.normal.x.abs() == largest {
            frame.normal.x
        } else if frame.normal.y.abs() == largest {
            frame.normal.y
        } else {
            frame.normal.z
        };
        if sign < 0. {
            frame.normal = -frame.normal;
            frame.v = -frame.v;
        }
        frame
    } else {
        let first = selected.first().copied().unwrap_or(0);
        Frame::new(&rings(mesh, mesh.faces.get(first).ok_or("Invalid selected face")?)[0])?
    };
    if selected.iter().any(|&id| {
        mesh.faces
            .get(id)
            .map(|f| rings(mesh, f).iter().flatten().any(|&p| !frame.on_plane(p)))
            .unwrap_or(true)
    }) {
        return Err("Select faces on one plane".into());
    }
    if selected.is_empty() && mesh.vertices.iter().any(|v| !frame.on_plane(v.position)) {
        return Err("Select faces on one plane to rebuild a 3D model".into());
    }
    let affected: Vec<_> = mesh
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| rings(mesh, f).iter().flatten().all(|&p| frame.on_plane(p)))
        .map(|(i, _)| i)
        .collect();
    let mut segments = vec![];
    for &i in &affected {
        for r in rings(mesh, &mesh.faces[i]) {
            for j in 0..r.len() {
                segments.push((frame.project(r[j]), frame.project(r[(j + 1) % r.len()])));
            }
        }
    }
    for e in &mesh.edges {
        let a = mesh.vertices[e.v0].position;
        let b = mesh.vertices[e.v1].position;
        if frame.on_plane(a) && frame.on_plane(b) {
            segments.push((frame.project(a), frame.project(b)));
        }
    }
    let cells = arrange_2d(&segments)?;
    let source_rings: Vec<Vec<Vec<DVec2>>> = affected
        .iter()
        .map(|&i| {
            rings(mesh, &mesh.faces[i])
                .iter()
                .map(|r| r.iter().map(|&p| frame.project(p)).collect())
                .collect()
        })
        .collect();
    let mut result = Mesh::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        if !affected.contains(&i) {
            add_face(&mut result, &rings(mesh, f), f)?;
        }
    }
    for (outer, holes) in cells {
        let p = sample(&outer);
        let source_index = source_rings.iter().position(|r| inside_rings(p, r));
        // Preserve declared voids; otherwise a bounded loose-wire region is
        // precisely the missing face this command is intended to reconstruct.
        if source_index.is_none()
            && source_rings
                .iter()
                .any(|rings| rings.iter().skip(1).any(|hole| inside_ring(p, hole)))
        {
            continue;
        }
        let default_source = MeshFace::new(vec![]);
        let source = source_index
            .map(|i| &mesh.faces[affected[i]])
            .unwrap_or(&default_source);
        let mut loops = vec![outer
            .iter()
            .map(|&p| frame.unproject(p))
            .collect::<Vec<_>>()];
        loops.extend(
            holes
                .iter()
                .map(|r| r.iter().map(|&p| frame.unproject(p)).collect()),
        );
        if source_index
            .map(|i| mesh.face_normal(affected[i]).dot(frame.normal) < 0.)
            .unwrap_or(false)
        {
            for r in &mut loops {
                r.reverse();
            }
        }
        add_face(&mut result, &loops, source)?;
    }
    preserve_edges(&mut result, mesh);
    validate_mesh(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn square(mesh: &mut Mesh, x: f64, y: f64, size: f64, z: f64) -> usize {
        let r = [
            DVec3::new(x, y, z),
            DVec3::new(x + size, y, z),
            DVec3::new(x + size, y + size, z),
            DVec3::new(x, y + size, z),
        ]
        .into_iter()
        .map(|p| mesh.add_vertex(p))
        .collect();
        mesh.add_face(r, vec![]).unwrap()
    }
    #[test]
    fn loose_closed_wire_reconstructs_missing_face_and_preserves_edge_metadata() {
        let mut mesh = Mesh::new();
        let ids: Vec<_> = [
            DVec3::ZERO,
            DVec3::new(2., 0., 0.),
            DVec3::new(2., 2., 0.),
            DVec3::new(0., 2., 0.),
        ]
        .into_iter()
        .map(|p| mesh.add_vertex(p))
        .collect();
        for i in 0..4 {
            let id = mesh.add_edge(ids[i], ids[(i + 1) % 4]).unwrap();
            mesh.edges[id].curve_id = Some(9);
            mesh.edges[id]
                .attrs
                .insert("retain".into(), serde_json::json!(i));
        }
        let rebuilt = rebuild_faces(&mesh, &[]).unwrap();
        assert_eq!(rebuilt.faces.len(), 1);
        assert!((rebuilt.face_area(0) - 4.).abs() < 1e-8);
        assert!(rebuilt.edges.iter().all(|e| e.faces.len() == 1
            && e.curve_id == Some(9)
            && e.attrs.contains_key("retain")));
        for v in &mesh.vertices {
            assert_eq!(
                rebuilt
                    .vertices
                    .iter()
                    .find(|p| p.position == v.position)
                    .unwrap()
                    .uid,
                v.uid
            );
        }
    }

    #[test]
    fn dangling_lines_and_hole_bridges_do_not_enter_face_rings() {
        let ring = [
            DVec2::ZERO,
            DVec2::new(4., 0.),
            DVec2::splat(4.),
            DVec2::new(0., 4.),
        ];
        let mut lines: Vec<_> = (0..4).map(|i| (ring[i], ring[(i + 1) % 4])).collect();
        lines.push((DVec2::new(0., 2.), DVec2::new(0.5, 2.)));
        let cells = arrange_2d(&lines).unwrap();
        assert_eq!(cells.len(), 1);
        assert!((area(&cells[0].0) - 16.).abs() < 1e-8);
        assert!(!cells[0].0.contains(&DVec2::new(0.5, 2.)));
        let hole = [
            DVec2::splat(1.),
            DVec2::new(1., 3.),
            DVec2::splat(3.),
            DVec2::new(3., 1.),
        ];
        lines.extend((0..4).map(|i| (hole[i], hole[(i + 1) % 4])));
        lines.push((DVec2::new(0.5, 2.), DVec2::new(1., 2.)));
        let cells = arrange_2d(&lines).unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(
            cells.iter().filter(|(_, holes)| holes.len() == 1).count(),
            1
        );
        for (outer, holes) in cells {
            let points: BTreeSet<_> = outer.iter().map(|&p| key(p)).collect();
            assert_eq!(points.len(), outer.len());
            for ring in holes {
                let points: BTreeSet<_> = ring.iter().map(|&p| key(p)).collect();
                assert_eq!(points.len(), ring.len());
            }
        }
    }

    #[test]
    fn dimension_anchor_survives_move_rebuild_and_another_move() {
        use crate::{dimensions::Dimension, scene::Scene};
        let mut scene = Scene::new();
        square(&mut scene.mesh, 0., 0., 2., 0.);
        square(&mut scene.mesh, 1., 1., 2., 0.);
        scene.dimensions.push(Dimension::new(
            DVec3::ZERO,
            DVec3::new(2., 2., 0.),
            DVec3::Y,
        ));
        scene.bind_dimensions();
        let held = scene.mesh.vertices[0].uid;
        for vertex in &mut scene.mesh.vertices {
            vertex.position.x += 5.;
        }
        scene.mesh.rebuild_spatial_index();
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].start.x, 5.);
        scene.mesh = rebuild_faces(&scene.mesh, &[]).unwrap();
        let vertex = scene
            .mesh
            .vertices
            .iter_mut()
            .find(|v| v.uid == held)
            .unwrap();
        assert_eq!(vertex.position, DVec3::new(5., 0., 0.));
        vertex.position.x += 1.;
        scene.refresh_dimensions();
        assert_eq!(scene.dimensions[0].start.x, 6.);
    }

    #[test]
    fn overlapping_squares_rebuild_into_three_regions_retaining_attributes() {
        let mut mesh = Mesh::new();
        square(&mut mesh, 0., 0., 2., 0.);
        square(&mut mesh, 1., 1., 2., 0.);
        mesh.faces[0].attrs.insert("mat".into(), "red".into());
        mesh.faces[1].attrs.insert("mat".into(), "blue".into());
        let rebuilt = rebuild_faces(&mesh, &[]).unwrap();
        assert_eq!(rebuilt.faces.len(), 3);
        let area: f64 = (0..rebuilt.faces.len()).map(|i| rebuilt.face_area(i)).sum();
        assert!((area - 7.).abs() < 1e-8);
        assert!(rebuilt.faces.iter().all(|f| f.attrs.contains_key("mat")));
    }
    #[test]
    fn selected_plane_leaves_other_planes_and_nested_holes_intact() {
        let mut mesh = Mesh::new();
        let id = square(&mut mesh, 0., 0., 4., 0.);
        let h = [
            DVec3::new(1., 1., 0.),
            DVec3::new(1., 3., 0.),
            DVec3::new(3., 3., 0.),
            DVec3::new(3., 1., 0.),
        ]
        .into_iter()
        .map(|p| mesh.add_vertex(p))
        .collect();
        mesh.faces[id].holes.push(h);
        square(&mut mesh, 8., 8., 1., 2.);
        let rebuilt = rebuild_faces(&mesh, &[0]).unwrap();
        assert_eq!(rebuilt.faces.len(), 2);
        assert_eq!(
            rebuilt.faces.iter().filter(|f| !f.holes.is_empty()).count(),
            1
        );
        assert!(rebuild_faces(&mesh, &[]).is_err());
        assert!(rebuild_faces(&mesh, &[0, 1]).is_err());
    }
}

/// Insert shared boundary stations in every incident face; no T-junctions.
pub(crate) fn stitch(mesh: &Mesh) -> Result<Mesh, String> {
    let mut result = Mesh::new();
    for face in &mesh.faces {
        let mut output = vec![];
        for ring in rings(mesh, face) {
            let mut points = vec![];
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                let d = b - a;
                let len = d.length_squared();
                if len < TOL * TOL {
                    return Err("Collapsed face edge".into());
                }
                let mut stations = vec![(0., a)];
                for vertex in &mesh.vertices {
                    let p = vertex.position;
                    let t = (p - a).dot(d) / len;
                    if t > TOL && t < 1. - TOL && (p - a - d * t).length() < 1e-6 {
                        stations.push((t, p));
                    }
                }
                stations.sort_by(|a, b| a.0.total_cmp(&b.0));
                stations.dedup_by(|a, b| (a.0 - b.0).abs() < TOL);
                points.extend(stations.into_iter().map(|(_, p)| p));
            }
            output.push(points);
        }
        add_face(&mut result, &output, face)?;
    }
    preserve_edges(&mut result, mesh);
    Ok(result)
}

/// Compare topology and metadata independently of indexing/cyclic ring starts.
pub fn equivalent(a: &Mesh, b: &Mesh) -> bool {
    if a.faces.len() != b.faces.len()
        || a.edges.len() != b.edges.len()
        || a.vertices.len() != b.vertices.len()
    {
        return false;
    }
    let cycle = |mesh: &Mesh, ring: &[usize]| {
        let mut points: Vec<_> = ring
            .iter()
            .map(|&v| crate::math::PointKey::from_vec3(mesh.vertices[v].position))
            .collect();
        if let Some((i, _)) = points.iter().enumerate().min_by_key(|(_, p)| **p) {
            points.rotate_left(i);
        }
        points
    };
    let mut used = vec![false; b.faces.len()];
    for f in &a.faces {
        let outer = cycle(a, &f.vertices);
        let mut holes: Vec<_> = f.holes.iter().map(|r| cycle(a, r)).collect();
        holes.sort();
        let Some(id) = b.faces.iter().enumerate().position(|(i, g)| {
            !used[i]
                && f.attrs == g.attrs
                && f.layer == g.layer
                && outer == cycle(b, &g.vertices)
                && {
                    let mut h: Vec<_> = g.holes.iter().map(|r| cycle(b, r)).collect();
                    h.sort();
                    h == holes
                }
        }) else {
            return false;
        };
        used[id] = true;
    }
    let mut used = vec![false; b.edges.len()];
    for e in &a.edges {
        let mut vertices = [
            crate::math::PointKey::from_vec3(a.vertices[e.v0].position),
            crate::math::PointKey::from_vec3(a.vertices[e.v1].position),
        ];
        vertices.sort();
        let Some(id) = b.edges.iter().enumerate().position(|(i, f)| {
            !used[i]
                && e.attrs == f.attrs
                && e.layer == f.layer
                && e.soft == f.soft
                && e.hidden == f.hidden
                && e.curve_id == f.curve_id
                && {
                    let mut p = [
                        crate::math::PointKey::from_vec3(b.vertices[f.v0].position),
                        crate::math::PointKey::from_vec3(b.vertices[f.v1].position),
                    ];
                    p.sort();
                    p == vertices
                }
        }) else {
            return false;
        };
        used[id] = true;
    }
    true
}

pub(crate) fn inside_or_boundary(p: DVec2, ring: &[DVec2]) -> bool {
    inside_ring(p, ring)
        || (0..ring.len()).any(|i| on_segment(p, ring[i], ring[(i + 1) % ring.len()]))
}
