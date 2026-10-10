// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
//! Follow Me: joint-plane miter sweeps and circular closed-path revolutions.
//! Ported from the original IngeTrazo core/sweep.py construction.
use crate::base::{pick_edge, pick_face, Tool, ToolContext, ToolType};
use glam::DVec3;
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

/// Clip a revolution profile to its own side of the axis. Axis crossings
/// become exact pole points, shared identically by every longitude station.
fn clip_half(loop_points: &[DVec3], tolerance: f64) -> Result<Vec<DVec3>, String> {
    // Local coordinates are (height, radial, transverse).
    let mut result = Vec::new();
    for i in 0..loop_points.len() {
        let current = loop_points[i];
        let next = loop_points[(i + 1) % loop_points.len()];
        let current_in = current.y >= -tolerance;
        let next_in = next.y >= -tolerance;
        if current_in {
            result.push(if current.y > tolerance {
                DVec3::new(current.x, current.y.max(0.), current.z)
            } else {
                DVec3::new(current.x, 0., 0.)
            });
        }
        if current_in != next_in {
            let t = current.y / (current.y - next.y);
            result.push(DVec3::new(current.x + (next.x - current.x) * t, 0., 0.));
        }
    }
    result.dedup_by(|a, b| (*a - *b).length() < 1e-12);
    if result.len() > 1 && (result[0] - *result.last().unwrap()).length() < 1e-12 {
        result.pop();
    }
    if result.len() < 3 {
        return Err("Revolution profile collapses on the axis".into());
    }
    Ok(result)
}

/// Match the original circular lathe construction, including full profiles
/// crossing the axis: their half profile sweeps once, not twice over itself.
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
    if (profile_center - center).dot(radial) < 0. {
        radial = -radial;
    }
    let side = axis.cross(radial);
    let local = |p: DVec3| {
        let relative = p - center;
        DVec3::new(relative.dot(axis), relative.dot(radial), relative.dot(side))
    };
    let size = loops[0]
        .iter()
        .map(|&p| local(p).abs().max_element())
        .fold(0., f64::max)
        .max(1e-12);
    let keep_tolerance = (size * 1e-6).max(1e-7);
    let mut outer: Vec<_> = loops[0].iter().copied().map(local).collect();
    if outer.iter().any(|p| p.y < -keep_tolerance) {
        outer = match clip_half(&outer, keep_tolerance) {
            Ok(points) => points,
            Err(error) => return Some(Err(error)),
        };
    } else {
        // A planar profile through the axis has zero transverse coordinate;
        // clamp pole positions to exact bit-identical points at all stations.
        for point in &mut outer {
            *point = if point.y.abs() > keep_tolerance {
                DVec3::new(point.x, point.y, 0.)
            } else {
                DVec3::new(point.x, 0., 0.)
            };
        }
    }
    let mut local_loops = vec![outer];
    for hole in &loops[1..] {
        let points: Vec<_> = hole.iter().copied().map(local).collect();
        if points.iter().any(|p| p.y < -keep_tolerance) {
            if points.iter().any(|p| p.y > keep_tolerance) {
                return Some(Err("A profile hole crosses the revolution axis".into()));
            }
            // A hole entirely on the discarded half does not affect this side.
        } else {
            local_loops.push(points);
        }
    }
    Some(Ok(path
        .iter()
        .map(|p| {
            let relative = *p - center;
            let angle = relative.dot(side).atan2(relative.dot(radial));
            let (sin, cos) = angle.sin_cos();
            local_loops
                .iter()
                .map(|ring| {
                    ring.iter()
                        .map(|p| {
                            center
                                + axis * p.x
                                + (radial * cos + side * sin) * p.y
                                + (side * cos - radial * sin) * p.z
                        })
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
    prepare_sweep(scene, face_id, path, closed, true)
}

fn prepare_sweep(
    scene: &mut Scene,
    face_id: usize,
    path: &[DVec3],
    closed: bool,
    validate: bool,
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
    // An attached profile sweeps material onto its own closed solid. Derive
    // adjacency from face rings, not potentially stale edge-face caches.
    let mut boundary_faces = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (id, face) in scene.mesh.faces.iter().enumerate() {
        for ring in std::iter::once(&face.vertices).chain(&face.holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                boundary_faces
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .push(id);
            }
        }
    }
    let attached = boundary_faces
        .values()
        .any(|faces| faces.contains(&face_id) && faces.len() > 1);
    let mut consumed = HashSet::from([face_id]);
    let body = if attached && validate {
        let mut queue = std::collections::VecDeque::from([face_id]);
        while let Some(id) = queue.pop_front() {
            let face = &scene.mesh.faces[id];
            for ring in std::iter::once(&face.vertices).chain(&face.holes) {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    for &other in &boundary_faces[&(a.min(b), a.max(b))] {
                        if consumed.insert(other) {
                            queue.push_back(other);
                        }
                    }
                }
            }
        }
        let mut faces: Vec<_> = consumed.iter().copied().collect();
        faces.sort_unstable();
        let edges = scene
            .mesh
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                !edge.faces.is_empty() && edge.faces.iter().any(|id| consumed.contains(id))
            })
            .map(|(id, _)| id)
            .collect();
        let body = ingetrazo_core::pieces::copy_part(
            &scene.mesh,
            &ingetrazo_core::pieces::Piece { faces, edges },
        )?;
        let closed_body = consumed.iter().all(|&id| {
            let face = &scene.mesh.faces[id];
            std::iter::once(&face.vertices)
                .chain(&face.holes)
                .all(|ring| {
                    (0..ring.len()).all(|i| {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        boundary_faces[&(a.min(b), a.max(b))].len() == 2
                    })
                })
        });
        if closed_body {
            ingetrazo_core::solids::validate_solid_mesh(&body)
                .map_err(|e| format!("Invalid attached solid: {e}"))?;
            Some(body)
        } else {
            // Original sweeps also accept a profile adjoining an open sheet.
            // Such sheets have no material volume to union: consume only the
            // profile and keep the other faces as document geometry.
            consumed.clear();
            consumed.insert(face_id);
            None
        }
    } else {
        None
    };
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
                let mut wall = vec![a[j], a[k], b[k], b[j]];
                wall.dedup_by(|p, q| (*p - *q).length() < 1e-9);
                if wall.len() > 1 && (wall[0] - *wall.last().unwrap()).length() < 1e-9 {
                    wall.pop();
                }
                if wall.len() >= 3 {
                    add_face(&mut sweep, &wall, &[], &source)?;
                }
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
    if !validate {
        scene.mesh = sweep;
        return Ok(());
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
    if let Some(body) = body {
        sweep = ingetrazo_core::solids::perform_solid_op(
            ingetrazo_core::solids::SolidOp::Union,
            &body,
            &sweep,
        )?;
    }
    let mut result = sweep;
    for (id, face) in scene.mesh.faces.iter().enumerate() {
        if consumed.contains(&id) {
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
    result = ingetrazo_core::planar::stitch_boundaries(&result)?;
    result.preserve_vertex_uids_from(&scene.mesh);
    ingetrazo_core::heal::refresh_interior(&mut result)?;
    scene.mesh = result;
    scene.clear_selection();
    scene.bump_version();
    Ok(())
}

/// Initial stations for a manually dragged path, including mid-edge profiles.
pub fn manual_path_start(
    mesh: &Mesh,
    face: usize,
    edge: usize,
    toward: DVec3,
) -> Result<(Option<DVec3>, Vec<usize>), String> {
    let e = mesh.edges.get(edge).ok_or("Invalid path edge")?;
    if e.v0 >= mesh.vertices.len() || e.v1 >= mesh.vertices.len() || face >= mesh.faces.len() {
        return Err("Invalid profile/path references".into());
    }
    let c = mesh.face_centroid(face);
    let n = mesh.face_normal(face);
    let a = mesh.vertices[e.v0].position;
    let b = mesh.vertices[e.v1].position;
    let da = (a - c).dot(n);
    let db = (b - c).dot(n);
    if da * db < -1e-12 {
        let far = if a.distance_squared(toward) < b.distance_squared(toward) {
            e.v0
        } else {
            e.v1
        };
        Ok((Some(a + (b - a) * (da / (da - db))), vec![far]))
    } else if da.abs() <= db.abs() {
        Ok((None, vec![e.v0, e.v1]))
    } else {
        Ok((None, vec![e.v1, e.v0]))
    }
}

/// Backtrack or bridge skipped stations by a bounded shortest-path search.
pub fn manual_path_extend(
    mesh: &Mesh,
    chain: &mut Vec<usize>,
    edge: usize,
    skip: &HashSet<usize>,
) -> bool {
    use std::collections::VecDeque;
    let Some(e) = mesh.edges.get(edge) else {
        return false;
    };
    let Some(&last) = chain.last() else {
        return false;
    };
    if skip.contains(&edge) {
        return false;
    }
    let ids: BTreeMap<_, _> = chain.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    if last == e.v0 || last == e.v1 {
        let other = if last == e.v0 { e.v1 } else { e.v0 };
        if chain.len() >= 2 && other == chain[chain.len() - 2] {
            chain.pop();
            return true;
        }
        if ids.contains_key(&other) {
            if other == chain[0] && chain.len() >= 3 {
                chain.push(other);
                return true;
            }
            return false;
        }
        chain.push(other);
        return true;
    }
    if let (Some(&a), Some(&b)) = (ids.get(&e.v0), ids.get(&e.v1)) {
        let len = chain.len();
        chain.truncate(a.max(b) + 1);
        return chain.len() != len;
    }
    let mut prev = BTreeMap::from([(last, None)]);
    let mut queue = VecDeque::from([last]);
    let mut hit = None;
    for _ in 0..400 {
        let Some(v) = queue.pop_front() else { break };
        for (id, ed) in mesh.edges.iter().enumerate() {
            if skip.contains(&id) || (ed.v0 != v && ed.v1 != v) {
                continue;
            }
            let w = if ed.v0 == v { ed.v1 } else { ed.v0 };
            if prev.contains_key(&w) || (ids.contains_key(&w) && w != last) {
                continue;
            }
            prev.insert(w, Some(v));
            if w == e.v0 || w == e.v1 {
                hit = Some(w);
                break;
            }
            queue.push_back(w);
        }
        if hit.is_some() {
            break;
        }
    }
    let Some(reached) = hit else { return false };
    let mut route = vec![];
    let mut v = reached;
    while v != last {
        route.push(v);
        v = prev[&v].unwrap();
    }
    route.reverse();
    chain.extend(route);
    let other = if reached == e.v0 { e.v1 } else { e.v0 };
    if !ids.contains_key(&other) && other != reached {
        chain.push(other);
    }
    true
}

fn orient_path(mesh: &Mesh, face: usize, points: &mut Vec<DVec3>, closed: bool) {
    let center = mesh.face_centroid(face);
    if closed && points.len() >= 3 {
        let start = (0..points.len())
            .min_by(|&a, &b| {
                points[a]
                    .distance_squared(center)
                    .total_cmp(&points[b].distance_squared(center))
            })
            .unwrap();
        points.rotate_left(start);
        let n = mesh.face_normal(face);
        if (points[points.len() - 1] - points[0])
            .normalize()
            .dot(n)
            .abs()
            > (points[1] - points[0]).normalize().dot(n).abs() + 1e-9
        {
            points[1..].reverse();
        }
    } else if points.len() >= 2
        && points[0].distance_squared(center) > points[points.len() - 1].distance_squared(center)
    {
        points.reverse();
    }
}

#[derive(Default)]
pub struct FollowMeTool {
    profile_face: Option<usize>,
    selected_path: Option<(Vec<DVec3>, bool)>,
    chain: Vec<usize>,
    start_point: Option<DVec3>,
    closed_path: Option<Vec<DVec3>>,
    skip: HashSet<usize>,
    last_edge: Option<usize>,
    preview: Option<Mesh>,
    path_lines: Vec<(DVec3, DVec3)>,
    pub last_error: Option<String>,
}
impl FollowMeTool {
    fn points(&self, scene: &Scene) -> (Vec<DVec3>, bool) {
        if let Some(p) = &self.closed_path {
            return (p.clone(), true);
        }
        let mut p = Vec::new();
        if let Some(s) = self.start_point {
            p.push(s);
        }
        p.extend(self.chain.iter().map(|&v| scene.mesh.vertices[v].position));
        let closed = self.start_point.is_none()
            && self.chain.len() >= 4
            && self.chain.first() == self.chain.last();
        if closed {
            p.pop();
        }
        (p, closed)
    }
    fn update_preview(&mut self, scene: &Scene) {
        let Some(face) = self.profile_face else {
            return;
        };
        let (mut points, closed) = self.points(scene);
        if closed {
            orient_path(&scene.mesh, face, &mut points, true);
        }
        self.path_lines = points.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && points.len() >= 3 {
            self.path_lines.push((*points.last().unwrap(), points[0]));
        }
        self.preview = None;
        if points.len() < 2 {
            return;
        }
        let mut temporary = Scene::new();
        let source = &scene.mesh.faces[face];
        let vertices = source
            .vertices
            .iter()
            .map(|&v| temporary.mesh.add_vertex(scene.mesh.vertices[v].position))
            .collect();
        let holes = source
            .holes
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|&v| temporary.mesh.add_vertex(scene.mesh.vertices[v].position))
                    .collect()
            })
            .collect();
        temporary.mesh.add_face(vertices, holes);
        temporary.mesh.faces[0].attrs = source.attrs.clone();
        // Ring construction is cached per changed station; expensive kernel
        // validation runs only on commit. The preview is never a live model.
        if prepare_sweep(&mut temporary, 0, &points, closed, false).is_ok() {
            self.preview = Some(temporary.mesh);
        }
    }
    fn finish(&mut self, scene: &mut Scene) {
        let Some(face) = self.profile_face else {
            return;
        };
        let (mut points, closed) = self.points(scene);
        if points.len() < 2 {
            let selected = self.selected_path.take();
            self.on_cancel(scene);
            self.selected_path = selected;
            self.last_error = Some("Touch the path edges before finishing the sweep".into());
            return;
        }
        if closed {
            orient_path(&scene.mesh, face, &mut points, true);
        }
        self.last_error = sweep_profile(scene, face, &points, closed).err();
        self.profile_face = None;
        self.chain.clear();
        self.start_point = None;
        self.closed_path = None;
        self.preview = None;
        self.path_lines.clear();
        self.last_edge = None;
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
        "Sweep a profile along selected or dragged path edges."
    }
    fn vcb_label(&self) -> &'static str {
        "Pick profile; drag path; Alt picks a face perimeter"
    }
    fn on_activate(&mut self, scene: &mut Scene) {
        *self = Self::default();
        let edges: Vec<_> = scene
            .selection
            .iter()
            .filter_map(|i| {
                if let SelectionItem::Edge(id) = i {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if !edges.is_empty() {
            match order_path_edges(&scene.mesh, &edges) {
                Ok(p) => self.selected_path = Some(p),
                Err(e) => self.last_error = Some(e),
            }
        } else {
            let faces: Vec<_> = scene
                .selection
                .iter()
                .filter_map(|i| {
                    if let SelectionItem::Face(id) = i {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            if faces.len() == 1 {
                self.selected_path = Some((
                    scene.mesh.faces[faces[0]]
                        .vertices
                        .iter()
                        .map(|&v| scene.mesh.vertices[v].position)
                        .collect(),
                    true,
                ));
            }
        }
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.last_error = None;
        if self.profile_face.is_some() {
            self.finish(scene);
            return;
        }
        let Some((face, _, _)) = pick_face(&scene.mesh, &ctx.ray) else {
            return;
        };
        // Also support tools constructed before selection without activation.
        let mut path = self.selected_path.clone();
        if path.is_none() {
            let edges: Vec<_> = scene
                .selection
                .iter()
                .filter_map(|i| {
                    if let SelectionItem::Edge(id) = i {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            if !edges.is_empty() {
                match order_path_edges(&scene.mesh, &edges) {
                    Ok(p) => path = Some(p),
                    Err(e) => {
                        self.last_error = Some(e);
                        return;
                    }
                }
            }
        }
        if let Some((mut points, closed)) = path {
            orient_path(&scene.mesh, face, &mut points, closed);
            self.last_error = sweep_profile(scene, face, &points, closed).err();
            return;
        }
        self.profile_face = Some(face);
        let center = scene.mesh.face_centroid(face);
        let normal = scene.mesh.face_normal(face);
        self.skip = scene
            .mesh
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                [e.v0, e.v1]
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position - center).dot(normal).abs() < 1e-6)
            })
            .map(|(i, _)| i)
            .collect();
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let Some(profile) = self.profile_face else {
            return;
        };
        if ctx.alt_held {
            if let Some((face, _, _)) = pick_face(&scene.mesh, &ctx.ray) {
                if face != profile {
                    let p: Vec<_> = scene.mesh.faces[face]
                        .vertices
                        .iter()
                        .map(|&v| scene.mesh.vertices[v].position)
                        .collect();
                    if self.closed_path.as_ref() != Some(&p) {
                        self.closed_path = Some(p);
                        self.update_preview(scene);
                    }
                }
            }
            return;
        }
        if self.closed_path.take().is_some() {
            self.update_preview(scene);
        }
        let Some(edge) = pick_edge(&scene.mesh, &ctx.ray, 0.2) else {
            return;
        };
        if self.skip.contains(&edge) || self.last_edge == Some(edge) {
            return;
        }
        self.last_edge = Some(edge);
        let changed = if self.chain.is_empty() {
            match manual_path_start(&scene.mesh, profile, edge, ctx.world) {
                Ok((start, chain)) => {
                    self.start_point = start;
                    self.chain = chain;
                    true
                }
                Err(e) => {
                    self.last_error = Some(e);
                    false
                }
            }
        } else {
            manual_path_extend(&scene.mesh, &mut self.chain, edge, &self.skip)
        };
        if changed {
            self.update_preview(scene);
        }
    }
    fn on_release(&mut self, scene: &mut Scene, _ctx: &ToolContext, drag_distance: f64) {
        if drag_distance >= 6. {
            self.finish(scene);
        }
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        let selected = self.selected_path.take();
        *self = Self::default();
        self.selected_path = selected;
    }
    fn on_deactivate(&mut self, scene: &mut Scene) {
        self.on_cancel(scene);
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        self.path_lines.clone()
    }
    fn preview_mesh(&self) -> Option<&Mesh> {
        self.preview.as_ref()
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
    fn attached_cap_sweep_unions_closed_body_and_preserves_open_attachment() {
        let mut scene = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.), DVec3::Y]
            .into_iter()
            .map(|p| scene.mesh.add_vertex(p))
            .collect();
        scene.mesh.add_face(ids, vec![]);
        crate::pushpull::PushPullTool::extrude_face(&mut scene, 0, 1.).unwrap();
        let face = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 1.).abs() < 1e-8)
            })
            .unwrap();
        let mut bent = scene.clone();
        sweep_profile(
            &mut scene,
            face,
            &[DVec3::new(0.5, 0.5, 1.), DVec3::new(0.5, 0.5, 2.)],
            false,
        )
        .unwrap();
        assert!(scene.mesh.is_manifold_solid());
        assert!((scene.mesh.volume().unwrap() - 2.).abs() < 1e-8);
        sweep_profile(
            &mut bent,
            face,
            &[
                DVec3::new(0.5, 0.5, 1.),
                DVec3::new(0.5, 0.5, 2.),
                DVec3::new(1.5, 0.5, 2.),
            ],
            false,
        )
        .unwrap();
        assert!(bent.mesh.is_manifold_solid());
        assert!((bent.mesh.volume().unwrap() - 3.).abs() < 1e-8);
        let mut open = profile(1., false);
        let ids = [
            DVec3::ZERO,
            DVec3::X,
            DVec3::new(1., -1., 0.),
            DVec3::new(0., -1., 0.),
        ]
        .into_iter()
        .map(|p| open.mesh.add_vertex(p))
        .collect();
        open.mesh.add_face(ids, vec![]);
        open.mesh.faces[1]
            .attrs
            .insert("sheet".into(), serde_json::json!("retained"));
        sweep_profile(&mut open, 0, &[DVec3::ZERO, DVec3::new(0., 2., 0.)], false).unwrap();
        assert_eq!(open.mesh.faces.len(), 7);
        let sheet = open
            .mesh
            .faces
            .iter()
            .position(|f| f.attrs.get("sheet") == Some(&serde_json::json!("retained")))
            .unwrap();
        assert!((open.mesh.face_area(sheet) - 1.).abs() < 1e-9);
        let ids: Vec<_> = (0..open.mesh.faces.len()).filter(|&i| i != sheet).collect();
        let solid = ingetrazo_core::pieces::copy_part(
            &open.mesh,
            &ingetrazo_core::pieces::Piece {
                faces: ids,
                edges: vec![],
            },
        )
        .unwrap();
        ingetrazo_core::solids::validate_solid_mesh(&solid).unwrap();
        assert!((solid.volume().unwrap() - 2.).abs() < 1e-8);
    }

    #[test]
    fn retained_equator_wires_soften_with_sphere_and_offset_profile_forms_torus() {
        let n = 24;
        let path: Vec<_> = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                DVec3::new(a.cos(), a.sin(), 0.)
            })
            .collect();
        for phi in [0_f64, 7.5] {
            let mut scene = Scene::new();
            let ids: Vec<_> = path.iter().map(|&p| scene.mesh.add_vertex(p)).collect();
            for i in 0..n {
                scene.mesh.add_edge(ids[i], ids[(i + 1) % n]);
            }
            let ring = (0..n)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / n as f64;
                    scene.mesh.add_vertex(DVec3::new(
                        a.cos() * phi.to_radians().cos(),
                        a.cos() * phi.to_radians().sin(),
                        a.sin(),
                    ))
                })
                .collect();
            scene.mesh.add_face(ring, vec![]);
            sweep_profile(&mut scene, 0, &path, true).unwrap();
            assert!(scene.mesh.edges.iter().all(|e| e.soft));
            assert!(scene.mesh.is_manifold_solid());
        }
        let mut scene = Scene::new();
        let ring = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                scene
                    .mesh
                    .add_vertex(DVec3::new(2. + 0.3 * a.cos(), 0., 0.3 * a.sin()))
            })
            .collect();
        scene.mesh.add_face(ring, vec![]);
        let path: Vec<_> = path.iter().map(|p| *p * 2.).collect();
        sweep_profile(&mut scene, 0, &path, true).unwrap();
        assert!(scene.mesh.is_manifold_solid());
        for v in &scene.mesh.vertices {
            let p = v.position;
            assert!((((p.x.hypot(p.y) - 2.).hypot(p.z)) - 0.3).abs() < 1e-7);
        }
    }

    #[test]
    fn selected_face_perimeter_and_alt_hover_supply_closed_paths_without_mutation() {
        use ingetrazo_core::math::Ray;
        let mut scene = profile(1., false);
        let ring = [
            DVec3::ZERO,
            DVec3::new(0., 3., 0.),
            DVec3::new(3., 3., 0.),
            DVec3::new(3., 0., 0.),
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(p))
        .collect();
        let face = scene.mesh.add_face(ring, vec![]).unwrap();
        let before = scene.mesh.clone();
        let mut tool = FollowMeTool::default();
        scene.selection.insert(SelectionItem::Face(face));
        tool.on_activate(&mut scene);
        assert!(tool.selected_path.as_ref().unwrap().1);
        tool.on_cancel(&mut scene);
        assert!(tool.selected_path.is_some());
        scene.clear_selection();
        tool.on_activate(&mut scene);
        let profile_ctx = ToolContext::new(
            DVec3::new(0.5, 0., 0.5),
            Ray::new(DVec3::new(0.5, -2., 0.5), DVec3::Y),
        );
        tool.on_click(&mut scene, &profile_ctx);
        let alt = ToolContext::with_modifiers(
            DVec3::new(1.5, 1.5, 0.),
            Ray::new(DVec3::new(1.5, 1.5, 4.), -DVec3::Z),
            false,
            false,
            true,
        );
        tool.on_move(&mut scene, &alt);
        assert_eq!(scene.mesh, before);
        assert_eq!(tool.rubber_band_lines().len(), 4);
        let preview = tool.preview_mesh().unwrap();
        assert!(preview.is_manifold_solid());
        assert!((preview.volume().unwrap() - 8.).abs() < 1e-8);
        tool.on_cancel(&mut scene);
        assert_eq!(scene.mesh, before);
        assert!(tool.preview_mesh().is_none());
    }

    #[test]
    fn manual_paths_bridge_backtrack_and_start_at_plane_crossings() {
        let mut scene = profile(1., false);
        let mut vertices = vec![];
        for y in 0..=5 {
            vertices.push(scene.mesh.add_vertex(DVec3::new(0., y as f64, 0.)));
        }
        let edges: Vec<_> = vertices
            .windows(2)
            .map(|w| scene.mesh.add_edge(w[0], w[1]).unwrap())
            .collect();
        let mut chain = vertices[..2].to_vec();
        let skip = HashSet::new();
        assert!(manual_path_extend(&scene.mesh, &mut chain, edges[4], &skip));
        assert_eq!(chain, vertices);
        assert!(manual_path_extend(&scene.mesh, &mut chain, edges[4], &skip));
        assert_eq!(chain, vertices[..5]);
        assert!(manual_path_extend(&scene.mesh, &mut chain, edges[1], &skip));
        assert_eq!(chain, vertices[..3]);
        let before = chain.clone();
        assert!(!manual_path_extend(
            &scene.mesh,
            &mut chain,
            edges[3],
            &HashSet::from([edges[3]])
        ));
        assert_eq!(chain, before);
        let a = scene.mesh.add_vertex(DVec3::new(0.5, -2., 0.5));
        let b = scene.mesh.add_vertex(DVec3::new(0.5, 2., 0.5));
        let e = scene.mesh.add_edge(a, b).unwrap();
        let (start, chain) =
            manual_path_start(&scene.mesh, 0, e, DVec3::new(0.5, -1., 0.5)).unwrap();
        assert_eq!(start, Some(DVec3::new(0.5, 0., 0.5)));
        assert_eq!(chain, vec![a]);
    }

    #[test]
    fn manual_hover_is_immutable_release_commits_and_escape_discards_preview() {
        use ingetrazo_core::math::Ray;
        let mut scene = profile(1., false);
        let a = scene.mesh.add_vertex(DVec3::ZERO);
        let b = scene.mesh.add_vertex(DVec3::new(0., 2., 0.));
        scene.mesh.add_edge(a, b);
        let before = scene.mesh.clone();
        let mut tool = FollowMeTool::default();
        let pick = ToolContext::new(
            DVec3::new(0.5, 0., 0.5),
            Ray::new(DVec3::new(0.5, -2., 0.5), DVec3::Y),
        );
        let hover = ToolContext::new(
            DVec3::new(0., 1., 0.),
            Ray::new(DVec3::new(0., 1., 2.), -DVec3::Z),
        );
        tool.on_click(&mut scene, &pick);
        tool.on_move(&mut scene, &hover);
        assert_eq!(scene.mesh, before);
        assert!(tool.preview_mesh().is_some());
        assert_eq!(tool.rubber_band_lines().len(), 1);
        tool.on_release(&mut scene, &hover, 2.);
        assert_eq!(scene.mesh, before);
        tool.on_cancel(&mut scene);
        assert!(tool.preview_mesh().is_none());
        assert_eq!(scene.mesh, before);
        tool.on_click(&mut scene, &pick);
        tool.on_move(&mut scene, &hover);
        tool.on_release(&mut scene, &hover, 10.);
        assert!(tool.last_error.is_none(), "{:?}", tool.last_error);
        assert!((scene.mesh.volume().unwrap() - 2.).abs() < 1e-8);
        assert!(tool.preview_mesh().is_none());
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
    #[test]
    fn full_circles_revolve_once_into_closed_spheres_with_shared_poles() {
        let n = 24;
        for degrees in [0.0_f64, 7.5, 33.] {
            for (radius, path_radius) in [(1., 1.), (0.5, 1.), (1., 0.5)] {
                let angle = degrees.to_radians();
                let radial = DVec3::new(angle.cos(), angle.sin(), 0.);
                let mut scene = Scene::new();
                let ids = (0..n)
                    .map(|i| {
                        let angle = 2. * std::f64::consts::PI * i as f64 / n as f64;
                        scene.mesh.add_vertex(
                            radial * (radius * angle.cos()) + DVec3::Z * (radius * angle.sin()),
                        )
                    })
                    .collect();
                scene.mesh.add_face(ids, vec![]);
                let path: Vec<_> = (0..n)
                    .map(|i| {
                        let angle = 2. * std::f64::consts::PI * i as f64 / n as f64;
                        DVec3::new(path_radius * angle.cos(), path_radius * angle.sin(), 0.)
                    })
                    .collect();
                sweep_profile(&mut scene, 0, &path, true).unwrap();
                assert_eq!(scene.mesh.faces.len(), n * (n / 2));
                assert!(scene.mesh.is_manifold_solid());
                assert!(scene.mesh.edges.iter().all(|e| e.soft));
                for vertex in &scene.mesh.vertices {
                    assert!((vertex.position.length() - radius).abs() < 1e-8);
                }
                let area: f64 = (0..scene.mesh.faces.len())
                    .map(|id| scene.mesh.face_area(id))
                    .sum();
                let sphere_area = 4. * std::f64::consts::PI * radius * radius;
                assert!((0.98 * sphere_area..sphere_area).contains(&area));
            }
        }
    }
}
