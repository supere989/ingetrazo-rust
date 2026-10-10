// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Fillet tool: round edges and corners.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_edge, Tool, ToolContext, ToolType};

use ingetrazo_core::{
    math::{newell_vector, PointKey},
    mesh::{Mesh, MeshFace},
};
use std::collections::HashMap;

fn inward(mesh: &Mesh, face: usize, a: usize, b: usize) -> Result<DVec3, String> {
    let f = &mesh.faces[face];
    for ring in std::iter::once(&f.vertices).chain(&f.holes) {
        for i in 0..ring.len() {
            let (v, w) = (ring[i], ring[(i + 1) % ring.len()]);
            if (v == a && w == b) || (v == b && w == a) {
                let direction = mesh.vertices[w].position - mesh.vertices[v].position;
                let into = mesh.face_normal(face).cross(direction).normalize_or_zero();
                if into.length_squared() > 0.5 {
                    return Ok(into);
                }
            }
        }
    }
    Err("Cannot read the faces around this edge".into())
}

fn inside_face(mesh: &Mesh, face: usize, p: DVec3) -> bool {
    let normal = mesh.face_normal(face).abs();
    let axis = if normal.x >= normal.y && normal.x >= normal.z {
        0
    } else if normal.y >= normal.z {
        1
    } else {
        2
    };
    let xy = |p: DVec3| match axis {
        0 => (p.y, p.z),
        1 => (p.x, p.z),
        _ => (p.x, p.y),
    };
    let (x, y) = xy(p);
    let contains = |ring: &Vec<usize>| {
        let mut inside = false;
        for i in 0..ring.len() {
            let (x0, y0) = xy(mesh.vertices[ring[i]].position);
            let (x1, y1) = xy(mesh.vertices[ring[(i + 1) % ring.len()]].position);
            if (y0 > y) != (y1 > y) && x < (x1 - x0) * (y - y0) / (y1 - y0) + x0 {
                inside = !inside;
            }
        }
        inside
    };
    contains(&mesh.faces[face].vertices) && !mesh.faces[face].holes.iter().any(contains)
}

fn add_polygon(
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
    if ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != ids.len()
    {
        return Err("Fillet is below document welding tolerance".into());
    }
    let id = mesh
        .add_face(ids, holes)
        .ok_or("Fillet would collapse a face")?;
    mesh.faces[id].attrs = source.attrs.clone();
    mesh.faces[id].layer = source.layer.clone();
    Ok(())
}

#[derive(Clone)]
struct Round {
    edge: usize,
    f1: usize,
    f2: usize,
    direction: DVec3,
    w1: DVec3,
    w2: DVec3,
    tangent: f64,
    center_offset: DVec3,
    offsets: Vec<DVec3>,
    sign: f64,
    ends: HashMap<usize, Vec<DVec3>>,
    joined: HashMap<usize, bool>,
}

fn set_replacement(
    mesh: &Mesh,
    replacements: &mut HashMap<(usize, usize), Vec<DVec3>>,
    face: usize,
    vertex: usize,
    points: Vec<DVec3>,
    round: Option<&Round>,
) -> Result<(), String> {
    if let Some(round) = round {
        if points.len() == 1 && (face == round.f1 || face == round.f2) {
            let w = if face == round.f1 { round.w1 } else { round.w2 };
            let edge = &mesh.edges[round.edge];
            let mid = (mesh.vertices[edge.v0].position + mesh.vertices[edge.v1].position) * 0.5;
            let point = points[0];
            if !inside_face(
                mesh,
                face,
                point + ((mid - point) - w * round.tangent) * 1e-3,
            ) {
                return Err("Radius is too large for an adjacent face".into());
            }
        }
    }
    if let Some(old) = replacements.get_mut(&(face, vertex)) {
        let same = |a: DVec3, b: DVec3| (a - b).length() < 1e-5;
        if points.len() == 1 && old.iter().any(|&p| same(p, points[0])) {
            return Ok(());
        }
        if old.len() == 1 && points.iter().any(|&p| same(p, old[0])) {
            *old = points;
            return Ok(());
        }
        if old.len() == points.len() && old.iter().zip(&points).all(|(&a, &b)| same(a, b)) {
            return Ok(());
        }
        return Err("Conflicting fillet corner cuts".into());
    }
    replacements.insert((face, vertex), points);
    Ok(())
}

fn cut_section(
    round: &Round,
    point: DVec3,
    normal: DVec3,
    anchor: DVec3,
) -> Result<Vec<DVec3>, String> {
    let denominator = round.direction.dot(normal);
    if denominator.abs() < 0.05 {
        return Err("Fillet end or joint plane is too oblique".into());
    }
    Ok(round
        .offsets
        .iter()
        .map(|&offset| {
            let base = point + offset;
            base + round.direction * (anchor - base).dot(normal) / denominator
        })
        .collect())
}

fn shared_source(mesh: &Mesh, round: &Round) -> MeshFace {
    let mut source = MeshFace::new(vec![]);
    let keys = ["color", "texture", "mat", "opacity"];
    if keys
        .iter()
        .all(|key| mesh.faces[round.f1].attrs.get(*key) == mesh.faces[round.f2].attrs.get(*key))
    {
        for key in keys {
            if let Some(value) = mesh.faces[round.f1].attrs.get(key) {
                source.attrs.insert(key.into(), value.clone());
            }
        }
    }
    if mesh.faces[round.f1].layer == mesh.faces[round.f2].layer {
        source.layer = mesh.faces[round.f1].layer.clone();
    }
    source
}

fn oriented(mut ring: Vec<DVec3>, center: DVec3, sign: f64) -> Vec<DVec3> {
    let centroid = ring.iter().sum::<DVec3>() / ring.len() as f64;
    if newell_vector(&ring).dot(centroid - center) * sign < 0.0 {
        ring.reverse();
    }
    ring
}

/// Spherical triangle with exact strip boundary rows, matching the original
/// sine-weighted corner grid. Returns n² triangles and all seam edges.
fn corner_patch(
    rounds: &[Round],
    group: &[usize],
    sections: &[Vec<DVec3>],
    radius: f64,
    segments: usize,
) -> Result<(Vec<Vec<DVec3>>, Vec<(DVec3, DVec3)>), String> {
    let meet = |a: &Vec<DVec3>, b: &Vec<DVec3>| {
        [a[0], a[segments]].into_iter().find(|&p| {
            [b[0], b[segments]]
                .into_iter()
                .any(|q| (p - q).length() < 1e-5)
        })
    };
    let c12 = meet(&sections[0], &sections[1]).ok_or("Corner strips do not meet")?;
    let c23 = meet(&sections[1], &sections[2]).ok_or("Corner strips do not meet")?;
    let c31 = meet(&sections[2], &sections[0]).ok_or("Corner strips do not meet")?;
    let center = group
        .iter()
        .zip(sections)
        .map(|(&id, s)| s[0] - rounds[id].w1 * rounds[id].tangent + rounds[id].center_offset)
        .sum::<DVec3>()
        / 3.;
    let directions = [c12, c23, c31].map(|p| (p - center).normalize_or_zero());
    if directions.iter().any(|d| d.length_squared() < 0.5) {
        return Err("Degenerate spherical fillet corner".into());
    }
    let endpoints = [(c12, c23), (c23, c31), (c12, c31)];
    let mut arcs: [Option<Vec<DVec3>>; 3] = [None, None, None];
    for section in sections {
        for (i, (a, b)) in endpoints.iter().enumerate() {
            if arcs[i].is_none() {
                if (section[0] - *a).length() < 1e-5 && (section[segments] - *b).length() < 1e-5 {
                    arcs[i] = Some(section.clone());
                } else if (section[0] - *b).length() < 1e-5
                    && (section[segments] - *a).length() < 1e-5
                {
                    arcs[i] = Some(section.iter().rev().copied().collect());
                }
            }
        }
    }
    let arcs: Vec<_> = arcs
        .into_iter()
        .collect::<Option<_>>()
        .ok_or("Cannot resolve spherical corner arcs")?;
    let mut grid = HashMap::new();
    for i in 0..=segments {
        for j in 0..=segments - i {
            let k = segments - i - j;
            let point = if k == 0 {
                arcs[0][j]
            } else if i == 0 {
                arcs[1][k]
            } else if j == 0 {
                arcs[2][k]
            } else {
                let weight =
                    |n: usize| (n as f64 * std::f64::consts::PI / (2. * segments as f64)).sin();
                let direction = (directions[0] * weight(i)
                    + directions[1] * weight(j)
                    + directions[2] * weight(k))
                .normalize_or_zero();
                if direction.length_squared() < 0.5 {
                    return Err("Degenerate spherical corner interpolation".into());
                }
                center + direction * radius
            };
            grid.insert((i as isize, j as isize), point);
        }
    }
    let mut triangles = Vec::new();
    let mut soft = Vec::new();
    let sign = rounds[group[0]].sign;
    for i in 0..segments as isize {
        for j in 0..segments as isize - i {
            triangles.push(oriented(
                vec![grid[&(i, j)], grid[&(i + 1, j)], grid[&(i, j + 1)]],
                center,
                sign,
            ));
            if i + j + 1 < segments as isize {
                triangles.push(oriented(
                    vec![grid[&(i + 1, j)], grid[&(i + 1, j + 1)], grid[&(i, j + 1)]],
                    center,
                    sign,
                ));
            }
        }
    }
    for (&(i, j), &p) in &grid {
        for (di, dj) in [(1, 0), (0, 1), (-1, 1)] {
            if let Some(&q) = grid.get(&(i + di, j + dj)) {
                soft.push((p, q));
            }
        }
    }
    Ok((triangles, soft))
}

/// Round selected solid edges in one atomic plan. One edge receives arc caps;
/// two selected edges meeting at a vertex share an exact miter section; three
/// share a spherical corner patch. Unsupported/skewed joints leave the document
/// unchanged, with a reason, rather than accepting a half-cut mesh.
pub fn fillet_edges(
    scene: &mut Scene,
    edges: &[usize],
    radius: f64,
    segments: usize,
) -> Result<(), String> {
    prepare_fillet(scene, edges, radius, segments, true)
}

fn prepare_fillet(
    scene: &mut Scene,
    edges: &[usize],
    radius: f64,
    segments: usize,
    validate: bool,
) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("Radius must be finite and positive".into());
    }
    if !(1..=256).contains(&segments) {
        return Err("Fillet segments must be between 1 and 256".into());
    }
    if edges.is_empty() {
        return Err("Select at least one solid edge to round".into());
    }
    let mesh = &scene.mesh;
    if validate {
        ingetrazo_core::solids::validate_solid_mesh(mesh)?;
    } else if mesh.vertices.iter().any(|v| !v.position.is_finite())
        || mesh.faces.iter().any(|f| {
            f.vertices
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
        return Err("Invalid mesh references for fillet preview".into());
    }
    let mut unique = std::collections::HashSet::new();
    let mut rounds = Vec::new();
    let mut at_vertex = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for &id in edges {
        if !unique.insert(id) {
            continue;
        }
        let edge = mesh.edges.get(id).ok_or("Invalid fillet edge")?;
        if edge.faces.len() != 2 {
            return Err("Fillet needs exactly two adjacent faces".into());
        }
        let (f1, f2) = (edge.faces[0], edge.faces[1]);
        let direction =
            (mesh.vertices[edge.v1].position - mesh.vertices[edge.v0].position).normalize_or_zero();
        if direction.length_squared() < 0.5 {
            return Err("Cannot round a zero-length edge".into());
        }
        let w1 = inward(mesh, f1, edge.v0, edge.v1)?;
        let w2 = inward(mesh, f2, edge.v0, edge.v1)?;
        let alpha = w1.dot(w2).clamp(-1., 1.).acos();
        if alpha < 2.0_f64.to_radians() || alpha > 178.0_f64.to_radians() {
            return Err("Faces meet too flat or too sharply to round".into());
        }
        let tangent = radius / (alpha * 0.5).tan();
        let center_offset = (w1 + w2) * (radius / alpha.sin());
        let a = (w1 * tangent - center_offset).normalize();
        let rb = w2 * tangent - center_offset;
        let b = (rb - a * rb.dot(a)).normalize();
        let phi = a.dot(rb.normalize()).clamp(-1., 1.).acos();
        let offsets = (0..=segments)
            .map(|k| {
                let angle = phi * k as f64 / segments as f64;
                center_offset + radius * (a * angle.cos() + b * angle.sin())
            })
            .collect();
        let sign = if mesh.face_normal(f1).dot(w1 * tangent - center_offset) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let round = Round {
            edge: id,
            f1,
            f2,
            direction,
            w1,
            w2,
            tangent,
            center_offset,
            offsets,
            sign,
            ends: HashMap::new(),
            joined: HashMap::new(),
        };
        for vertex in [edge.v0, edge.v1] {
            at_vertex.entry(vertex).or_default().push(rounds.len());
        }
        rounds.push(round);
    }
    let mut replacements = HashMap::<(usize, usize), Vec<DVec3>>::new();
    let mut patches = Vec::new();
    let mut soft = Vec::new();
    let mut curves = Vec::new();
    for (&vertex, group) in &at_vertex {
        let faces_here: Vec<_> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(_, face)| {
                face.vertices
                    .iter()
                    .chain(face.holes.iter().flatten())
                    .any(|&v| v == vertex)
            })
            .map(|(id, _)| id)
            .collect();
        let point = mesh.vertices[vertex].position;
        match group.len() {
            1 => {
                let id = group[0];
                let round = &rounds[id];
                let caps: Vec<_> = faces_here
                    .iter()
                    .copied()
                    .filter(|&f| f != round.f1 && f != round.f2)
                    .collect();
                if caps.len() != 1 {
                    return Err("Fillet needs one simple end cap at an unjoined endpoint".into());
                }
                let cap = caps[0];
                let anchor = mesh.vertices[mesh.faces[cap].vertices[0]].position;
                let section = cut_section(round, point, mesh.face_normal(cap), anchor)?;
                set_replacement(
                    mesh,
                    &mut replacements,
                    round.f1,
                    vertex,
                    vec![section[0]],
                    Some(round),
                )?;
                set_replacement(
                    mesh,
                    &mut replacements,
                    round.f2,
                    vertex,
                    vec![section[segments]],
                    Some(round),
                )?;
                set_replacement(
                    mesh,
                    &mut replacements,
                    cap,
                    vertex,
                    section.clone(),
                    Some(round),
                )?;
                rounds[id].ends.insert(vertex, section);
                rounds[id].joined.insert(vertex, false);
            }
            2 => {
                let (ia, ib) = (group[0], group[1]);
                let (ra, rb) = (&rounds[ia], &rounds[ib]);
                let allowed = std::collections::HashSet::from([ra.f1, ra.f2, rb.f1, rb.f2]);
                if faces_here.iter().any(|f| !allowed.contains(f)) {
                    return Err("Fillet joint has extra faces".into());
                }
                let other = |r: &Round| {
                    let e = &mesh.edges[r.edge];
                    mesh.vertices[if e.v0 == vertex { e.v1 } else { e.v0 }].position
                };
                let normal = ((other(ra) - point).normalize() - (other(rb) - point).normalize())
                    .normalize_or_zero();
                if normal.length_squared() < 0.5 {
                    return Err("Fillet edges fold back at their joint".into());
                }
                let section = cut_section(ra, point, normal, point)?;
                let section_b = if rb.f1 == ra.f1 || rb.f2 == ra.f2 {
                    section.clone()
                } else if rb.f1 == ra.f2 || rb.f2 == ra.f1 {
                    section.iter().rev().copied().collect()
                } else {
                    return Err("Joined fillet edges share no face".into());
                };
                for (r, section) in [(ra, &section), (rb, &section_b)] {
                    set_replacement(
                        mesh,
                        &mut replacements,
                        r.f1,
                        vertex,
                        vec![section[0]],
                        Some(r),
                    )?;
                    set_replacement(
                        mesh,
                        &mut replacements,
                        r.f2,
                        vertex,
                        vec![section[segments]],
                        Some(r),
                    )?;
                }
                rounds[ia].ends.insert(vertex, section);
                rounds[ia].joined.insert(vertex, true);
                rounds[ib].ends.insert(vertex, section_b);
                rounds[ib].joined.insert(vertex, true);
            }
            3 => {
                let allowed: std::collections::HashSet<_> = group
                    .iter()
                    .flat_map(|&id| [rounds[id].f1, rounds[id].f2])
                    .collect();
                if faces_here.iter().any(|f| !allowed.contains(f)) {
                    return Err("Fillet corner has extra faces".into());
                }
                let mut sections = Vec::new();
                for &id in group {
                    let round = &rounds[id];
                    let mut hits = Vec::new();
                    for (face, w) in [(round.f1, round.w1), (round.f2, round.w2)] {
                        let neighbors: Vec<_> = group
                            .iter()
                            .copied()
                            .filter(|&q| q != id && (rounds[q].f1 == face || rounds[q].f2 == face))
                            .collect();
                        if neighbors.len() != 1 {
                            return Err("Fillet corner is not a simple three-edge corner".into());
                        }
                        let neighbor = &rounds[neighbors[0]];
                        let wn = if neighbor.f1 == face {
                            neighbor.w1
                        } else {
                            neighbor.w2
                        };
                        let p = point + w * round.tangent;
                        let q = point + wn * neighbor.tangent;
                        let normal = mesh.face_normal(face);
                        let denominator = round.direction.cross(neighbor.direction).dot(normal);
                        if denominator.abs() < 1e-12 {
                            return Err("Cannot intersect corner tangent lines".into());
                        }
                        hits.push(
                            p + round.direction * (q - p).cross(neighbor.direction).dot(normal)
                                / denominator,
                        );
                    }
                    let s1 = (hits[0] - point).dot(round.direction);
                    let s2 = (hits[1] - point).dot(round.direction);
                    if (s1 - s2).abs() > 1e-3 * round.tangent.max(1.) {
                        return Err("Fillet corner is too skewed".into());
                    }
                    let e = &mesh.edges[round.edge];
                    if s1.abs()
                        > (mesh.vertices[e.v1].position - mesh.vertices[e.v0].position).length()
                            * 0.5
                            + 1e-9
                    {
                        return Err("Radius is too large for the corner edge".into());
                    }
                    let origin = point + round.direction * s1;
                    let section: Vec<_> = round
                        .offsets
                        .iter()
                        .map(|&offset| origin + offset)
                        .collect();
                    set_replacement(
                        mesh,
                        &mut replacements,
                        round.f1,
                        vertex,
                        vec![section[0]],
                        Some(round),
                    )?;
                    set_replacement(
                        mesh,
                        &mut replacements,
                        round.f2,
                        vertex,
                        vec![section[segments]],
                        Some(round),
                    )?;
                    sections.push(section);
                }
                for (&id, section) in group.iter().zip(&sections) {
                    rounds[id].ends.insert(vertex, section.clone());
                    rounds[id].joined.insert(vertex, true);
                }
                let (triangles, seams) = corner_patch(&rounds, group, &sections, radius, segments)?;
                let source = shared_source(mesh, &rounds[group[0]]);
                patches.push((triangles, source));
                soft.extend(seams);
            }
            _ => return Err("More than three selected fillet edges meet at a vertex".into()),
        }
    }
    let mut result = Mesh::new();
    for (fid, face) in mesh.faces.iter().enumerate() {
        let rebuild = |ring: &Vec<usize>| -> Result<Vec<DVec3>, String> {
            let mut points = Vec::new();
            for i in 0..ring.len() {
                let vertex = ring[i];
                if at_vertex.contains_key(&vertex) {
                    let mut replacement = replacements
                        .get(&(fid, vertex))
                        .ok_or("Unresolved fillet corner")?
                        .clone();
                    if replacement.len() > 1 {
                        let prev = mesh.vertices[ring[(i + ring.len() - 1) % ring.len()]].position;
                        let next = mesh.vertices[ring[(i + 1) % ring.len()]].position;
                        let forward = (prev - replacement[0]).length()
                            + (next - *replacement.last().unwrap()).length();
                        let backward = (prev - *replacement.last().unwrap()).length()
                            + (next - replacement[0]).length();
                        if backward < forward {
                            replacement.reverse();
                        }
                    }
                    points.extend(replacement);
                } else {
                    points.push(mesh.vertices[vertex].position);
                }
            }
            points.dedup_by(|a, b| (*a - *b).length() < 1e-8);
            if points.len() > 1 && (points[0] - *points.last().unwrap()).length() < 1e-8 {
                points.pop();
            }
            if points.len() < 3 {
                return Err("Radius would collapse an adjacent face".into());
            }
            let origin = mesh.vertices[face.vertices[0]].position;
            let normal = mesh.face_normal(fid);
            if points
                .iter()
                .any(|p| (*p - origin).dot(normal).abs() > 1e-7 * (*p - origin).length().max(1.))
            {
                return Err("Fillet would bend a planar face".into());
            }
            Ok(points)
        };
        let ring = rebuild(&face.vertices)?;
        let holes = face
            .holes
            .iter()
            .map(rebuild)
            .collect::<Result<Vec<_>, _>>()?;
        add_polygon(&mut result, &ring, &holes, face)?;
    }
    for round in &rounds {
        let edge = &mesh.edges[round.edge];
        let (a, b) = (&round.ends[&edge.v0], &round.ends[&edge.v1]);
        if (b[0] - a[0]).dot(round.direction) <= 1e-6 {
            return Err("Fillet endpoint cuts cross".into());
        }
        let center = (a[0] + b[0]) * 0.5 - round.w1 * round.tangent + round.center_offset;
        let source = shared_source(mesh, round);
        for i in 0..segments {
            add_polygon(
                &mut result,
                &oriented(vec![a[i], a[i + 1], b[i + 1], b[i]], center, round.sign),
                &[],
                &source,
            )?;
        }
        for i in 0..=segments {
            soft.push((a[i], b[i]));
        }
        for vertex in [edge.v0, edge.v1] {
            let section = &round.ends[&vertex];
            if round.joined[&vertex] {
                for i in 0..segments {
                    soft.push((section[i], section[i + 1]));
                }
            } else {
                curves.push(section.clone());
            }
        }
    }
    for (triangles, source) in patches {
        for triangle in triangles {
            add_polygon(&mut result, &triangle, &[], &source)?;
        }
    }
    if !validate {
        scene.mesh = result;
        return Ok(());
    }
    ingetrazo_core::solids::validate_solid_mesh(&result)?;
    let positions: HashMap<_, _> = result
        .vertices
        .iter()
        .enumerate()
        .map(|(id, v)| (PointKey::from_vec3(v.position), id))
        .collect();
    for original in &mesh.edges {
        if original.attrs.is_empty()
            && !original.soft
            && original.curve_id.is_none()
            && original.layer.is_none()
            && !original.hidden
        {
            continue;
        }
        let a = mesh.vertices[original.v0].position;
        let b = mesh.vertices[original.v1].position;
        let d = b - a;
        let length = d.length_squared();
        if length < 1e-20 {
            continue;
        }
        for edge in &mut result.edges {
            let on = |p: DVec3| {
                let t = (p - a).dot(d) / length;
                (-1e-9..=1. + 1e-9).contains(&t) && (p - (a + t * d)).length() < 1e-8
            };
            if on(result.vertices[edge.v0].position) && on(result.vertices[edge.v1].position) {
                edge.attrs = original.attrs.clone();
                edge.soft = original.soft;
                edge.curve_id = original.curve_id;
                edge.layer = original.layer.clone();
                edge.hidden = original.hidden;
            }
        }
    }
    for (a, b) in soft {
        let a = positions[&PointKey::from_vec3(a)];
        let b = positions[&PointKey::from_vec3(b)];
        if let Some(id) = result.find_edge(a, b) {
            result.edges[id].soft = true;
        }
    }
    let mut curve = mesh
        .edges
        .iter()
        .filter_map(|e| e.curve_id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Curve identifiers exhausted")?;
    for section in curves {
        for i in 0..segments {
            let a = positions[&PointKey::from_vec3(section[i])];
            let b = positions[&PointKey::from_vec3(section[i + 1])];
            if let Some(id) = result.find_edge(a, b) {
                result.edges[id].curve_id = Some(curve);
                result.edges[id].soft = false;
            }
        }
        curve = curve.checked_add(1).ok_or("Curve identifiers exhausted")?;
    }
    scene.mesh = result;
    scene.clear_selection();
    scene.bump_version();
    Ok(())
}

/// Convenience wrapper for a single selected edge.
pub fn fillet_edge(
    scene: &mut Scene,
    edge: usize,
    radius: f64,
    segments: usize,
) -> Result<(), String> {
    fillet_edges(scene, &[edge], radius, segments)
}

pub struct FilletTool {
    pub radius: f64,
    pub segments: usize,
    pub sizing: bool,
    edges: Vec<usize>,
    hover_edge: Option<usize>,
    hover_point: Option<DVec3>,
    preview: Option<Mesh>,
    preview_key: Option<(Vec<usize>, u64, usize, u64)>,
    label_point: Option<DVec3>,
    pub last_error: Option<String>,
}
impl Default for FilletTool {
    fn default() -> Self {
        Self {
            radius: 0.1,
            segments: 8,
            sizing: false,
            edges: vec![],
            hover_edge: None,
            hover_point: None,
            preview: None,
            preview_key: None,
            label_point: None,
            last_error: None,
        }
    }
}
impl FilletTool {
    fn selection(scene: &Scene) -> Vec<usize> {
        let mut ids: Vec<_> = scene
            .selection
            .iter()
            .filter_map(|i| {
                if let ingetrazo_core::scene::SelectionItem::Edge(id) = i {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        ids.sort_unstable();
        ids
    }
    fn reset(&mut self) {
        self.edges.clear();
        self.sizing = false;
        self.hover_edge = None;
        self.hover_point = None;
        self.preview = None;
        self.preview_key = None;
        self.label_point = None;
        self.last_error = None;
    }
    fn replan(&mut self, scene: &Scene) {
        let edges = if self.edges.is_empty() {
            self.hover_edge.into_iter().collect()
        } else {
            self.edges.clone()
        };
        let key = (
            edges.clone(),
            (self.radius * 1e6).round().to_bits(),
            self.segments,
            scene.version,
        );
        if self.preview_key.as_ref() == Some(&key) {
            return;
        }
        self.preview_key = Some(key);
        self.preview = None;
        self.label_point = edges
            .first()
            .and_then(|&id| scene.mesh.edges.get(id))
            .map(|e| {
                (scene.mesh.vertices[e.v0].position + scene.mesh.vertices[e.v1].position) * 0.5
            });
        if edges.is_empty() {
            return;
        }
        let mut temporary = Scene::new();
        temporary.mesh = scene.mesh.clone();
        match prepare_fillet(&mut temporary, &edges, self.radius, self.segments, false) {
            Ok(()) => {
                let mut preview = Mesh::new();
                for face in temporary.mesh.faces.iter().skip(scene.mesh.faces.len()) {
                    let points: Vec<_> = face
                        .vertices
                        .iter()
                        .map(|&v| temporary.mesh.vertices[v].position)
                        .collect();
                    let holes: Vec<Vec<_>> = face
                        .holes
                        .iter()
                        .map(|h| {
                            h.iter()
                                .map(|&v| temporary.mesh.vertices[v].position)
                                .collect()
                        })
                        .collect();
                    if add_polygon(&mut preview, &points, &holes, face).is_err() {
                        return;
                    }
                }
                self.preview = Some(preview);
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(e),
        }
    }
    fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.edges.is_empty() {
            return Ok(());
        }
        fillet_edges(scene, &self.edges, self.radius, self.segments)?;
        self.reset();
        Ok(())
    }
    pub fn set_segments(&mut self, scene: &Scene, segments: usize) -> Result<(), String> {
        if !(1..=256).contains(&segments) {
            return Err("Fillet segments must be between 1 and 256".into());
        }
        self.segments = segments;
        self.replan(scene);
        Ok(())
    }
}
impl Tool for FilletTool {
    fn error_message(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Fillet
    }
    fn name(&self) -> &'static str {
        "Fillet"
    }
    fn description(&self) -> &'static str {
        "Click an edge, size the radius with the cursor, then click to round."
    }
    fn vcb_label(&self) -> &'static str {
        "Radius; Ns changes segments"
    }
    fn on_activate(&mut self, scene: &mut Scene) {
        self.reset();
        self.edges = Self::selection(scene);
        self.replan(scene);
    }
    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.last_error = None;
        if self.sizing {
            self.last_error = self.commit(scene).err();
            return;
        }
        if self.edges.is_empty() {
            let Some(edge) = pick_edge(&scene.mesh, &ctx.ray, 0.3) else {
                return;
            };
            let selected = Self::selection(scene);
            self.edges = if selected.contains(&edge) {
                selected
            } else {
                vec![edge]
            };
        }
        self.sizing = true;
        self.hover_point = Some(ctx.world);
        self.replan(scene);
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
        if self.sizing {
            let best = self
                .edges
                .iter()
                .filter_map(|&id| scene.mesh.edges.get(id))
                .filter_map(|e| {
                    let a = scene.mesh.vertices[e.v0].position;
                    let d = (scene.mesh.vertices[e.v1].position - a).normalize_or_zero();
                    if d.length_squared() < 0.5 {
                        return None;
                    }
                    let offset = ctx.world - a;
                    Some((offset - d * offset.dot(d)).length())
                })
                .min_by(f64::total_cmp);
            if let Some(r) = best {
                if r >= 1e-4 && r.is_finite() {
                    self.radius = r;
                }
            }
        } else if self.edges.is_empty() {
            self.hover_edge = pick_edge(&scene.mesh, &ctx.ray, 0.3);
        }
        self.replan(scene);
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
    }
    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let text = text.trim();
        if let Some(count) = text.strip_suffix('s') {
            let count = count
                .trim()
                .parse::<usize>()
                .map_err(|_| "Enter a segment count followed by s")?;
            return self.set_segments(scene, count);
        }
        let radius = text
            .parse::<f64>()
            .map_err(|_| "Enter a valid positive radius")?;
        if !radius.is_finite() || radius <= 0. {
            return Err("Enter a finite radius >0".into());
        }
        self.radius = radius;
        if self.edges.is_empty() {
            self.edges = Self::selection(scene);
        }
        self.commit(scene)
    }
    fn preview_mesh(&self) -> Option<&Mesh> {
        self.preview.as_ref()
    }
    fn value_label(&self) -> Option<(String, DVec3)> {
        self.label_point.map(|p| {
            (
                self.last_error.clone().unwrap_or_else(|| {
                    format!("R {:.3} m ({} segments)", self.radius, self.segments)
                }),
                p,
            )
        })
    }
    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.hover_point.map(|h| (h, h + DVec3::X * self.radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_scene() -> Scene {
        let mut scene = Scene::new();
        for p in [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [2., 0., 1.],
            [2., 1., 1.],
            [0., 1., 1.],
        ] {
            scene.mesh.add_vertex(DVec3::from_array(p));
        }
        for ring in [
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ] {
            let id = scene.mesh.add_face(ring, vec![]).unwrap();
            scene.mesh.faces[id]
                .attrs
                .insert("mat".into(), "stone".into());
        }
        scene
    }
    #[test]
    fn cursor_sizing_preview_and_second_click_commit_match_original_workflow() {
        use ingetrazo_core::math::Ray;
        let mut scene = box_scene();
        let before = scene.mesh.clone();
        let mut tool = FilletTool::default();
        let edge = scene.mesh.find_edge(4, 5).unwrap();
        let pick = ToolContext::new(
            DVec3::new(1., 0., 1.),
            Ray::new(DVec3::new(1., -2., 1.), DVec3::Y),
        );
        tool.on_move(&mut scene, &pick);
        assert_eq!(tool.hover_edge, Some(edge));
        assert_eq!(tool.preview_mesh().unwrap().faces.len(), 8);
        assert_eq!(scene.mesh, before);
        tool.on_click(&mut scene, &pick);
        assert!(tool.sizing);
        assert_eq!(scene.mesh, before);
        let size = ToolContext::new(
            DVec3::new(1., 0.25, 1.),
            Ray::new(DVec3::new(1., 0.25, 3.), -DVec3::Z),
        );
        tool.on_move(&mut scene, &size);
        assert!((tool.radius - 0.25).abs() < 1e-9);
        assert_eq!(scene.mesh, before);
        tool.on_click(&mut scene, &size);
        assert!(!tool.sizing);
        assert!(tool.last_error.is_none(), "{:?}", tool.last_error);
        assert_eq!(scene.mesh.faces.len(), 14);
        assert!(scene.mesh.is_manifold_solid());
        assert!(tool.preview_mesh().is_none());
    }

    #[test]
    fn selected_rim_accepts_segments_and_typed_radius_without_preview_mutation() {
        use ingetrazo_core::scene::SelectionItem;
        let mut scene = box_scene();
        for (a, b) in [(4, 5), (5, 6), (6, 7), (7, 4)] {
            let e = scene.mesh.find_edge(a, b).unwrap();
            scene.selection.insert(SelectionItem::Edge(e));
        }
        let before = scene.mesh.clone();
        let mut tool = FilletTool::default();
        tool.on_activate(&mut scene);
        assert_eq!(tool.edges.len(), 4);
        tool.on_vcb_input(&mut scene, "6s").unwrap();
        assert_eq!(tool.segments, 6);
        assert_eq!(scene.mesh, before);
        assert_eq!(tool.preview_mesh().unwrap().faces.len(), 24);
        assert!(tool.on_vcb_input(&mut scene, "-1").is_err());
        assert_eq!(scene.mesh, before);
        assert!(tool.on_vcb_input(&mut scene, "5").is_err());
        assert_eq!(scene.mesh, before);
        tool.on_vcb_input(&mut scene, "0.15").unwrap();
        assert_eq!(scene.mesh.faces.len(), 30);
        assert!(scene.selection.is_empty());
        assert!(!tool.sizing);
        assert!(tool.edges.is_empty());
    }

    #[test]
    fn one_edge_creates_tangent_strip_caps_curves_and_analytic_volume() {
        let mut scene = box_scene();
        let edge = scene.mesh.find_edge(4, 5).unwrap();
        fillet_edge(&mut scene, edge, 0.2, 8).unwrap();
        assert_eq!(scene.mesh.faces.len(), 14);
        assert!(scene.mesh.is_manifold_solid());
        let removed = (0.2_f64.powi(2) - std::f64::consts::PI * 0.2_f64.powi(2) / 4.) * 2.;
        assert!((scene.mesh.volume().unwrap() - (2. - removed)).abs() < 0.001);
        for face in &scene.mesh.faces[6..] {
            for &vertex in &face.vertices {
                let p = scene.mesh.vertices[vertex].position;
                assert!(((p.y - 0.2).hypot(p.z - 0.8) - 0.2).abs() < 1e-8);
            }
        }
        let top = scene
            .mesh
            .faces
            .iter()
            .find(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 1.).abs() < 1e-8)
            })
            .unwrap();
        assert!(
            (top.vertices
                .iter()
                .map(|&v| scene.mesh.vertices[v].position.y)
                .fold(f64::INFINITY, f64::min)
                - 0.2)
                .abs()
                < 1e-8
        );
        assert_eq!(scene.mesh.edges.iter().filter(|e| e.soft).count(), 9);
        assert_eq!(
            scene
                .mesh
                .edges
                .iter()
                .filter_map(|e| e.curve_id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2
        );
        assert!(scene
            .mesh
            .faces
            .iter()
            .all(|f| f.attrs.get("mat") == Some(&"stone".into())));
    }

    #[test]
    fn concave_edge_adds_material_and_invalid_radius_is_atomic() {
        let mut scene = Scene::new();
        let ids = [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [0., 2., 0.],
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(DVec3::from_array(p)))
        .collect();
        scene.mesh.add_face(ids, vec![]);
        crate::pushpull::PushPullTool::extrude_face(&mut scene, 0, 1.).unwrap();
        let v0 = scene
            .mesh
            .vertices
            .iter()
            .position(|v| v.position == DVec3::new(1., 1., 0.))
            .unwrap();
        let v1 = scene
            .mesh
            .vertices
            .iter()
            .position(|v| v.position == DVec3::new(1., 1., 1.))
            .unwrap();
        let edge = scene.mesh.find_edge(v0, v1).unwrap();
        let before = scene.clone();
        for radius in [f64::NAN, f64::INFINITY, -1., 0., 3.] {
            assert!(fillet_edge(&mut scene, edge, radius, 8).is_err());
            assert_eq!(scene, before);
        }
        fillet_edge(&mut scene, edge, 0.2, 8).unwrap();
        let added = 0.2_f64.powi(2) * (1. - std::f64::consts::PI / 4.);
        assert!((scene.mesh.volume().unwrap() - (3. + added)).abs() < 0.001);
    }
    #[test]
    fn transformed_solid_rounding_uses_world_radius() {
        let mut scene = box_scene();
        let transform = glam::DMat4::from_translation(DVec3::new(3., -5., 7.))
            * glam::DMat4::from_rotation_y(0.37)
            * glam::DMat4::from_scale(DVec3::new(2., 3., 0.5));
        for vertex in &mut scene.mesh.vertices {
            vertex.position = transform.transform_point3(vertex.position);
        }
        scene.mesh.rebuild_spatial_index();
        let edge = scene.mesh.find_edge(4, 5).unwrap();
        fillet_edge(&mut scene, edge, 0.1, 16).unwrap();
        let removed = (0.1_f64.powi(2) - std::f64::consts::PI * 0.1_f64.powi(2) / 4.) * 4.;
        assert!((scene.mesh.volume().unwrap() - (6. - removed)).abs() < 0.0001);
    }
    #[test]
    fn rim_and_all_edges_build_mitered_strips_and_spherical_corners() {
        let mut rim = box_scene();
        let edges: Vec<_> = [(4, 5), (5, 6), (6, 7), (7, 4)]
            .into_iter()
            .map(|(a, b)| rim.mesh.find_edge(a, b).unwrap())
            .collect();
        fillet_edges(&mut rim, &edges, 0.2, 8).unwrap();
        assert_eq!(rim.mesh.faces.len(), 6 + 4 * 8);
        assert!(rim.mesh.is_manifold_solid());
        assert!(rim.mesh.edges.iter().all(|e| e.curve_id.is_none()));
        let top = rim
            .mesh
            .faces
            .iter()
            .find(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (rim.mesh.vertices[v].position.z - 1.).abs() < 1e-8)
            })
            .unwrap();
        let min_x = top
            .vertices
            .iter()
            .map(|&v| rim.mesh.vertices[v].position.x)
            .fold(f64::INFINITY, f64::min);
        assert!((min_x - 0.2).abs() < 1e-8);
        let mut all = box_scene();
        let edges: Vec<_> = (0..all.mesh.edges.len()).collect();
        fillet_edges(&mut all, &edges, 0.2, 8).unwrap();
        assert_eq!(all.mesh.faces.len(), 6 + 12 * 8 + 8 * 64);
        assert!(all.mesh.is_manifold_solid());
        assert!(all.mesh.edges.iter().all(|e| e.soft));
        let center = DVec3::splat(0.2);
        let corner: Vec<_> = all
            .mesh
            .faces
            .iter()
            .filter(|f| {
                f.vertices.len() == 3
                    && f.vertices.iter().all(|&v| {
                        let p = all.mesh.vertices[v].position;
                        p.x < 0.21 && p.y < 0.21 && p.z < 0.21
                    })
            })
            .collect();
        assert_eq!(corner.len(), 64);
        for face in corner {
            for &v in &face.vertices {
                assert!(((all.mesh.vertices[v].position - center).length() - 0.2).abs() < 1e-8);
            }
        }
        assert!((1.8..1.9).contains(&all.mesh.volume().unwrap()));
    }
}
