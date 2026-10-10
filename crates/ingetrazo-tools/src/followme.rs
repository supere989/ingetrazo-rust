// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
//! Follow Me: joint-plane miter sweeps and circular closed-path revolutions.
//! Ported from the original IngeTrazo core/sweep.py construction.
use crate::base::{pick_edge, pick_face, Tool, ToolContext, ToolType};
use glam::{DQuat, DVec3};
use ingetrazo_core::{
    math::newell_vector,
    mesh::{Mesh, MeshFace},
    scene::{Scene, SelectionItem},
};
use std::collections::{BTreeMap, HashSet};

/// Order one simple connected edge chain, rejecting branches/disconnected loops.
pub fn order_path_edges(mesh: &Mesh, edges: &[usize]) -> Result<(Vec<DVec3>, bool), String> {
    if edges.is_empty() {
        return Err("Select connected path edges".into());
    }
    let mut adjacent = BTreeMap::<usize, Vec<usize>>::new();
    let mut unique = HashSet::new();
    for &id in edges {
        let edge = mesh.edges.get(id).ok_or("Invalid path edge")?;
        if !unique.insert(id)
            || edge.v0 == edge.v1
            || edge.v0 >= mesh.vertices.len()
            || edge.v1 >= mesh.vertices.len()
        {
            return Err("Invalid or repeated path edge".into());
        }
        adjacent.entry(edge.v0).or_default().push(edge.v1);
        adjacent.entry(edge.v1).or_default().push(edge.v0);
    }
    if adjacent.values().any(|v| v.len() > 2) {
        return Err("Path cannot branch".into());
    }
    let ends: Vec<_> = adjacent
        .iter()
        .filter(|(_, v)| v.len() == 1)
        .map(|(&v, _)| v)
        .collect();
    if ends.len() != 0 && ends.len() != 2 {
        return Err("Path is not a simple chain".into());
    }
    let closed = ends.is_empty();
    let start = if closed {
        *adjacent.keys().next().unwrap()
    } else {
        ends[0]
    };
    let mut current = start;
    let mut previous = None;
    let mut points = vec![mesh.vertices[start].position];
    let mut visited = HashSet::from([start]);
    for step in 0..edges.len() {
        let next = adjacent[&current]
            .iter()
            .copied()
            .find(|v| Some(*v) != previous)
            .ok_or("Disconnected path")?;
        if next == start && closed && step + 1 == edges.len() {
            return Ok((points, true));
        }
        if !visited.insert(next) {
            return Err("Disconnected or repeated path vertices".into());
        }
        points.push(mesh.vertices[next].position);
        previous = Some(current);
        current = next;
    }
    if closed || points.len() != adjacent.len() {
        return Err("Disconnected path".into());
    }
    Ok((points, false))
}

fn project(
    ring: &[DVec3],
    direction: DVec3,
    anchor: DVec3,
    normal: DVec3,
) -> Result<Vec<DVec3>, String> {
    let denominator = direction.dot(normal);
    if denominator.abs() < 1e-9 {
        return Err("Profile is parallel to a sweep joint".into());
    }
    Ok(ring
        .iter()
        .map(|&p| p + direction * (anchor - p).dot(normal) / denominator)
        .collect())
}

/// Circular path matching the original lathe construction. Profiles crossing
/// the revolution axis are refused until clipped-pole topology is implemented.
fn revolution_rings(
    loops: &[Vec<DVec3>],
    normal: DVec3,
    path: &[DVec3],
) -> Option<Result<Vec<Vec<Vec<DVec3>>>, String>> {
    let center = path.iter().sum::<DVec3>() / path.len() as f64;
    let axis = newell_vector(path).normalize_or_zero();
    if axis.length_squared() < 0.5 {
        return None;
    }
    let radii: Vec<_> = path.iter().map(|p| (*p - center).length()).collect();
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    if mean < 1e-9 {
        return None;
    }
    let tolerance = (mean * 1e-3).max(1e-4);
    if path
        .iter()
        .any(|p| (*p - center).dot(axis).abs() > tolerance)
        || radii
            .iter()
            .any(|r| (*r - mean).abs() > tolerance + mean * 1e-2)
        || normal.dot(axis).abs() > 1e-3
        || (loops[0][0] - center).dot(normal).abs() > tolerance
    {
        return None;
    }
    let mut radial = axis.cross(normal).normalize();
    let profile_center = loops[0].iter().sum::<DVec3>() / loops[0].len() as f64;
    if (profile_center - center).dot(radial) < 0.0 {
        radial = -radial;
    }
    if loops
        .iter()
        .flatten()
        .any(|p| (*p - center).dot(radial) <= 1e-6)
    {
        return Some(Err(
            "Profile touching or crossing the revolution axis is unsupported".into(),
        ));
    }
    let side = axis.cross(radial);
    Some(Ok(path
        .iter()
        .map(|p| {
            let relative = *p - center;
            let angle = relative.dot(side).atan2(relative.dot(radial));
            let rotation = DQuat::from_axis_angle(axis, angle);
            loops
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|p| center + rotation * (*p - center))
                        .collect()
                })
                .collect()
        })
        .collect()))
}

fn add_face(
    mesh: &mut Mesh,
    ring: &[DVec3],
    holes: &[Vec<DVec3>],
    source: &MeshFace,
) -> Result<(), String> {
    let ids: Vec<_> = ring.iter().map(|&p| mesh.add_vertex(p)).collect();
    let holes: Vec<Vec<_>> = holes
        .iter()
        .map(|h| h.iter().map(|&p| mesh.add_vertex(p)).collect())
        .collect();
    if ids.iter().copied().collect::<HashSet<_>>().len() != ids.len()
        || holes
            .iter()
            .any(|h| h.iter().copied().collect::<HashSet<_>>().len() != h.len())
    {
        return Err("Sweep is below document welding tolerance".into());
    }
    let id = mesh.add_face(ids, holes).ok_or("Degenerate sweep face")?;
    mesh.faces[id].attrs = source.attrs.clone();
    mesh.faces[id].layer = source.layer.clone();
    Ok(())
}

/// Sweep a free-standing planar profile (holes included), atomically. Incoming
/// rings slide onto joint planes, producing exact miters rather than pinching.
/// Geometry validation precedes replacement; unrelated faces and wires survive.
pub fn sweep_profile(
    scene: &mut Scene,
    face_id: usize,
    path: &[DVec3],
    closed: bool,
) -> Result<(), String> {
    if scene.mesh.vertices.iter().any(|v| !v.position.is_finite())
        || scene.mesh.faces.iter().any(|f| {
            f.vertices
                .iter()
                .chain(f.holes.iter().flatten())
                .any(|&v| v >= scene.mesh.vertices.len())
        })
        || scene.mesh.edges.iter().any(|e| {
            e.v0 >= scene.mesh.vertices.len()
                || e.v1 >= scene.mesh.vertices.len()
                || e.faces.iter().any(|&f| f >= scene.mesh.faces.len())
        })
    {
        return Err("Document mesh has invalid coordinates or references".into());
    }
    let source = scene
        .mesh
        .faces
        .get(face_id)
        .ok_or("Invalid profile face")?
        .clone();
    if source.vertices.len() < 3
        || source.holes.iter().any(|h| h.len() < 3)
        || source
            .vertices
            .iter()
            .chain(source.holes.iter().flatten())
            .any(|&v| v >= scene.mesh.vertices.len())
    {
        return Err("Invalid profile boundary".into());
    }
    if scene
        .mesh
        .edges
        .iter()
        .any(|e| e.faces.contains(&face_id) && e.faces.len() > 1)
    {
        return Err("Follow Me currently requires a free-standing profile".into());
    }
    if path.len() < if closed { 3 } else { 2 } || path.iter().any(|p| !p.is_finite()) {
        return Err("Sweep needs a finite connected path".into());
    }
    let normal = scene.mesh.face_normal(face_id);
    let origin = scene.mesh.vertices[source.vertices[0]].position;
    let mut loops: Vec<Vec<_>> = std::iter::once(&source.vertices)
        .chain(&source.holes)
        .map(|ring| {
            ring.iter()
                .map(|&v| scene.mesh.vertices[v].position)
                .collect()
        })
        .collect();
    if loops
        .iter()
        .flatten()
        .any(|p| !p.is_finite() || (*p - origin).dot(normal).abs() > 1e-7)
        || newell_vector(&loops[0]).length_squared() < 1e-16
    {
        return Err("Profile must be nondegenerate and planar".into());
    }
    for hole in &mut loops[1..] {
        if newell_vector(hole).dot(normal) > 0.0 {
            hole.reverse();
        }
    }
    let spans = if closed { path.len() } else { path.len() - 1 };
    let mut directions = Vec::new();
    for i in 0..spans {
        let delta = path[(i + 1) % path.len()] - path[i];
        if delta.length() < 1e-6 {
            return Err("Path contains a zero-length segment".into());
        }
        directions.push(delta.normalize());
    }
    // Face orientation follows the first span so every wall and cap is outward.
    if normal.dot(directions[0]) < 0.0 {
        for ring in &mut loops {
            ring.reverse();
        }
    }
    let mut rings = if closed {
        revolution_rings(&loops, normal, path).transpose()?
    } else {
        None
    };
    if rings.is_none() {
        let mut stations: Vec<Vec<Vec<DVec3>>> = Vec::new();
        for i in 0..path.len() {
            let joint = if !closed && i == 0 {
                directions[0]
            } else if !closed && i == spans {
                directions[spans - 1]
            } else {
                let sum = directions[(i + spans - 1) % spans] + directions[i % spans];
                if sum.length() < 1e-6 {
                    return Err("Path reverses by 180 degrees".into());
                }
                sum.normalize()
            };
            let incoming = if i == 0 {
                directions[0]
            } else {
                directions[i - 1]
            };
            let previous = if i == 0 { &loops } else { &stations[i - 1] };
            stations.push(
                previous
                    .iter()
                    .map(|ring| project(ring, incoming, path[i], joint))
                    .collect::<Result<_, _>>()?,
            );
        }
        rings = Some(stations);
    }
    let rings = rings.unwrap();
    let mut sweep = Mesh::new();
    for i in 0..spans {
        let next = if closed { (i + 1) % rings.len() } else { i + 1 };
        for (a, b) in rings[i].iter().zip(&rings[next]) {
            for j in 0..a.len() {
                let k = (j + 1) % a.len();
                add_face(&mut sweep, &[a[j], a[k], b[k], b[j]], &[], &source)?;
            }
        }
    }
    if !closed {
        let mut bottom = rings[0].clone();
        for ring in &mut bottom {
            ring.reverse();
        }
        add_face(&mut sweep, &bottom[0], &bottom[1..], &source)?;
        let top = rings.last().unwrap();
        add_face(&mut sweep, &top[0], &top[1..], &source)?;
    }
    // A negative closed-path orientation can arise from the initial profile
    // normal. Normalize all rings together, never one arbitrary face.
    let signed: f64 = (0..sweep.faces.len())
        .flat_map(|id| sweep.triangulate_face(id))
        .map(|(a, b, c)| a.dot(b.cross(c)) / 6.0)
        .sum();
    if signed < 0.0 {
        for face in &mut sweep.faces {
            face.vertices.reverse();
            for hole in &mut face.holes {
                hole.reverse();
            }
        }
    }
    ingetrazo_core::solids::validate_solid_mesh(&sweep)?;
    for id in 0..sweep.edges.len() {
        let edge = &sweep.edges[id];
        if edge.faces.len() == 2 {
            let dot = sweep
                .face_normal(edge.faces[0])
                .dot(sweep.face_normal(edge.faces[1]));
            if dot > 0.85 && dot < 0.99995 {
                sweep.edges[id].soft = true;
            }
        }
    }
    let mut result = sweep;
    for (id, face) in scene.mesh.faces.iter().enumerate() {
        if id == face_id {
            continue;
        }
        let ring: Vec<_> = face
            .vertices
            .iter()
            .map(|&v| scene.mesh.vertices[v].position)
            .collect();
        let holes: Vec<Vec<_>> = face
            .holes
            .iter()
            .map(|h| h.iter().map(|&v| scene.mesh.vertices[v].position).collect())
            .collect();
        add_face(&mut result, &ring, &holes, face)?;
    }
    let coordinates: std::collections::HashMap<_, _> = result
        .vertices
        .iter()
        .enumerate()
        .map(|(id, v)| (ingetrazo_core::math::PointKey::from_vec3(v.position), id))
        .collect();
    for edge in &scene.mesh.edges {
        if !edge.faces.is_empty()
            && edge.attrs.is_empty()
            && !edge.soft
            && edge.curve_id.is_none()
            && edge.layer.is_none()
            && !edge.hidden
        {
            continue;
        }
        let a = scene.mesh.vertices[edge.v0].position;
        let b = scene.mesh.vertices[edge.v1].position;
        let id = if edge.faces.is_empty() {
            let a = result.add_vertex(a);
            let b = result.add_vertex(b);
            result.add_edge(a, b)
        } else {
            coordinates
                .get(&ingetrazo_core::math::PointKey::from_vec3(a))
                .zip(coordinates.get(&ingetrazo_core::math::PointKey::from_vec3(b)))
                .and_then(|(&a, &b)| result.find_edge(a, b))
        };
        if let Some(id) = id {
            result.edges[id].attrs = edge.attrs.clone();
            result.edges[id].curve_id = edge.curve_id;
            result.edges[id].soft |= edge.soft;
            result.edges[id].hidden = edge.hidden;
            result.edges[id].layer = edge.layer.clone();
        }
    }
    for vertex in &scene.mesh.vertices {
        if vertex.edges.is_empty() {
            result.add_vertex(vertex.position);
        }
    }
    scene.mesh = result;
    scene.clear_selection();
    scene.bump_version();
    Ok(())
}

#[derive(Default)]
pub struct FollowMeTool {
    profile_face: Option<usize>,
    hover_point: Option<DVec3>,
    pub last_error: Option<String>,
}

impl FollowMeTool {
    fn selected_path(scene: &Scene) -> Vec<usize> {
        let mut ids: Vec<_> = scene
            .selection
            .iter()
            .filter_map(|item| {
                if let SelectionItem::Edge(id) = item {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        ids.sort_unstable();
        ids
    }
    fn apply(&mut self, scene: &mut Scene, face: usize, edges: &[usize]) -> Result<(), String> {
        let (mut points, closed) = order_path_edges(&scene.mesh, edges)?;
        let center = scene.mesh.face_centroid(face);
        if closed {
            let start = (0..points.len())
                .min_by(|&a, &b| {
                    (points[a] - center)
                        .length_squared()
                        .total_cmp(&(points[b] - center).length_squared())
                })
                .unwrap();
            points.rotate_left(start);
            let normal = scene.mesh.face_normal(face);
            if (points[points.len() - 1] - points[0])
                .normalize()
                .dot(normal)
                .abs()
                > (points[1] - points[0]).normalize().dot(normal).abs() + 1e-9
            {
                points[1..].reverse();
            }
        } else if (points[0] - center).length_squared()
            > (points[points.len() - 1] - center).length_squared()
        {
            points.reverse();
        }
        sweep_profile(scene, face, &points, closed)
    }
}

impl Tool for FollowMeTool {
    fn error_message(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    fn tool_type(&self) -> ToolType {
        ToolType::FollowMe
    }
    fn name(&self) -> &'static str {
        "Follow Me"
    }
    fn shortcut(&self) -> Option<&'static str> {
        Some("W")
    }
    fn description(&self) -> &'static str {
        "Sweep a profile along selected connected path edges."
    }
    fn vcb_label(&self) -> &'static str {
        "Select connected path edges, then profile"
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.last_error = None;
        let edges = Self::selected_path(scene);
        if !edges.is_empty() {
            if let Some((face, _, _)) = pick_face(&scene.mesh, &ctx.ray) {
                self.last_error = self.apply(scene, face, &edges).err();
                self.profile_face = None;
            }
            return;
        }
        if self.profile_face.is_none() {
            self.profile_face = pick_face(&scene.mesh, &ctx.ray).map(|(face, _, _)| face);
        } else if let Some(edge) = pick_edge(&scene.mesh, &ctx.ray, 0.2) {
            // Manual path: all connected wire edges of the clicked path.
            let mut connected = HashSet::from([edge]);
            loop {
                let vertices: HashSet<_> = connected
                    .iter()
                    .flat_map(|&id| [scene.mesh.edges[id].v0, scene.mesh.edges[id].v1])
                    .collect();
                let old = connected.len();
                for (id, e) in scene.mesh.edges.iter().enumerate() {
                    if e.faces.is_empty() && (vertices.contains(&e.v0) || vertices.contains(&e.v1))
                    {
                        connected.insert(id);
                    }
                }
                if old == connected.len() {
                    break;
                }
            }
            let mut ids: Vec<_> = connected.into_iter().collect();
            ids.sort_unstable();
            self.last_error = self.apply(scene, self.profile_face.unwrap(), &ids).err();
            self.profile_face = None;
        }
    }
    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.profile_face = None;
        self.hover_point = None;
        self.last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile(size: f64, hole: bool) -> Scene {
        let mut scene = Scene::new();
        let ids = [
            DVec3::ZERO,
            DVec3::new(size, 0., 0.),
            DVec3::new(size, 0., size),
            DVec3::new(0., 0., size),
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(p))
        .collect();
        let holes = if hole {
            vec![[
                DVec3::new(0.5, 0., 0.5),
                DVec3::new(1.5, 0., 0.5),
                DVec3::new(1.5, 0., 1.5),
                DVec3::new(0.5, 0., 1.5),
            ]
            .into_iter()
            .map(|p| scene.mesh.add_vertex(p))
            .collect()]
        } else {
            vec![]
        };
        scene.mesh.add_face(ids, holes);
        scene.mesh.faces[0]
            .attrs
            .insert("mat".into(), "stone".into());
        scene
    }

    #[test]
    fn straight_and_bent_paths_form_mitered_solids() {
        let mut straight = profile(1., false);
        sweep_profile(
            &mut straight,
            0,
            &[DVec3::ZERO, DVec3::new(0., 3., 0.)],
            false,
        )
        .unwrap();
        assert_eq!(straight.mesh.faces.len(), 6);
        assert!((straight.mesh.volume().unwrap() - 3.).abs() < 1e-8);
        let mut bent = profile(1., false);
        sweep_profile(
            &mut bent,
            0,
            &[DVec3::ZERO, DVec3::new(0., 4., 0.), DVec3::new(4., 4., 0.)],
            false,
        )
        .unwrap();
        assert!((bent.mesh.volume().unwrap() - 7.).abs() < 1e-8);
        assert!(bent
            .mesh
            .vertices
            .iter()
            .any(|v| (v.position.x + v.position.y - 4.).abs() < 1e-8));
        assert!(bent
            .mesh
            .faces
            .iter()
            .all(|f| f.attrs.get("mat") == Some(&"stone".into())));
    }

    #[test]
    fn holed_profile_and_closed_circular_path() {
        let mut tube = profile(2., true);
        sweep_profile(&mut tube, 0, &[DVec3::ZERO, DVec3::new(0., 5., 0.)], false).unwrap();
        assert!((tube.mesh.volume().unwrap() - 15.).abs() < 1e-8);
        assert_eq!(
            tube.mesh
                .faces
                .iter()
                .filter(|f| !f.holes.is_empty())
                .count(),
            2
        );
        let mut torus = profile(1., false);
        for vertex in &mut torus.mesh.vertices {
            vertex.position.x += 4.;
        }
        torus.mesh.rebuild_spatial_index();
        let n = 12;
        let path: Vec<_> = (0..n)
            .map(|i| {
                let angle = 2. * std::f64::consts::PI * i as f64 / n as f64;
                DVec3::new(4.5 * angle.cos(), 4.5 * angle.sin(), 0.5)
            })
            .collect();
        sweep_profile(&mut torus, 0, &path, true).unwrap();
        assert_eq!(torus.mesh.faces.len(), 4 * n);
        // Polygon annulus area: n/2 sin(2π/n) (R²-r²), height1.
        assert!((torus.mesh.volume().unwrap() - 27.).abs() < 1e-8);
        assert!(torus.mesh.edges.iter().any(|e| e.soft));
    }

    #[test]
    fn path_ordering_rejects_branches_and_disconnected_chains() {
        let mut mesh = Mesh::new();
        let points = [
            DVec3::ZERO,
            DVec3::X,
            DVec3::new(2., 0., 0.),
            DVec3::new(2., 1., 0.),
        ];
        let ids: Vec<_> = points.into_iter().map(|p| mesh.add_vertex(p)).collect();
        let mut edges = vec![
            mesh.add_edge(ids[0], ids[1]).unwrap(),
            mesh.add_edge(ids[2], ids[1]).unwrap(),
            mesh.add_edge(ids[2], ids[3]).unwrap(),
        ];
        let (path, closed) = order_path_edges(&mesh, &edges).unwrap();
        assert!(!closed);
        assert_eq!(path, points);
        let extra = mesh.add_vertex(DVec3::Y);
        edges.push(mesh.add_edge(ids[1], extra).unwrap());
        assert!(order_path_edges(&mesh, &edges).is_err());
        let a = mesh.add_vertex(DVec3::splat(4.));
        let b = mesh.add_vertex(DVec3::splat(5.));
        let disjoint = mesh.add_edge(a, b).unwrap();
        assert!(order_path_edges(&mesh, &[edges[0], disjoint]).is_err());
    }

    #[test]
    fn invalid_path_and_axis_crossing_are_atomic() {
        let mut scene = profile(1., false);
        let before = scene.clone();
        assert!(sweep_profile(
            &mut scene,
            0,
            &[DVec3::ZERO, DVec3::Y * 3., DVec3::ZERO],
            false
        )
        .is_err());
        assert_eq!(scene, before);
        assert!(
            sweep_profile(&mut scene, 0, &[DVec3::ZERO, DVec3::splat(f64::NAN)], false).is_err()
        );
        assert_eq!(scene, before);
    }
}
