// SPDX-License-Identifier: GPL-3.0-or-later
//! Separate welded physical solids without pulling in their neighbours' edges.
use crate::{groups::Group, mesh::Mesh, planar};
use glam::{DMat4, DVec3};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub faces: Vec<usize>,
    pub edges: Vec<usize>,
}
struct Dsu(Vec<usize>);
impl Dsu {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    fn find(&mut self, x: usize) -> usize {
        if self.0[x] != x {
            self.0[x] = self.find(self.0[x]);
        }
        self.0[x]
    }
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.0[b] = a;
    }
}
fn face_vertices(mesh: &Mesh, f: usize) -> impl Iterator<Item = usize> + '_ {
    mesh.faces[f]
        .vertices
        .iter()
        .chain(mesh.faces[f].holes.iter().flatten())
        .copied()
}
pub fn connected_parts(mesh: &Mesh) -> Result<Vec<Piece>, String> {
    planar::validate_mesh(mesh)?;
    let mut dsu = Dsu::new(mesh.vertices.len());
    for i in 0..mesh.faces.len() {
        let vs: Vec<_> = face_vertices(mesh, i).collect();
        for &v in &vs[1..] {
            dsu.join(vs[0], v);
        }
    }
    for e in &mesh.edges {
        dsu.join(e.v0, e.v1);
    }
    let mut parts = BTreeMap::new();
    for i in 0..mesh.faces.len() {
        parts
            .entry(dsu.find(mesh.faces[i].vertices[0]))
            .or_insert_with(|| Piece {
                faces: vec![],
                edges: vec![],
            })
            .faces
            .push(i);
    }
    for (i, e) in mesh.edges.iter().enumerate() {
        parts
            .entry(dsu.find(e.v0))
            .or_insert_with(|| Piece {
                faces: vec![],
                edges: vec![],
            })
            .edges
            .push(i);
    }
    let mut parts: Vec<_> = parts.into_values().collect();
    parts.sort_by_key(|p| {
        (
            std::cmp::Reverse(p.faces.len()),
            std::cmp::Reverse(p.edges.len()),
        )
    });
    Ok(parts)
}
fn inward(mesh: &Mesh, f: usize, a: usize, b: usize, n: DVec3) -> Option<DVec3> {
    for (k, ring) in std::iter::once(&mesh.faces[f].vertices)
        .chain(&mesh.faces[f].holes)
        .enumerate()
    {
        for i in 0..ring.len() {
            let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
            if (p == a && q == b) || (p == b && q == a) {
                let mut w = n.cross(mesh.vertices[q].position - mesh.vertices[p].position);
                if k > 0 {
                    let points: Vec<_> = ring.iter().map(|&v| mesh.vertices[v].position).collect();
                    if crate::math::newell_vector(&points).dot(n) > 0. {
                        w = -w;
                    }
                }
                return (w.length_squared() > 1e-12).then(|| w.normalize());
            }
        }
    }
    None
}
fn partners(mesh: &Mesh, e: usize) -> Vec<(usize, usize)> {
    let edge = &mesh.edges[e];
    if edge.faces.len() < 3 {
        return if edge.faces.len() == 2 {
            vec![(edge.faces[0], edge.faces[1])]
        } else {
            vec![]
        };
    }
    let axis =
        (mesh.vertices[edge.v1].position - mesh.vertices[edge.v0].position).normalize_or_zero();
    let mut info = vec![];
    let mut basis = None;
    for &f in &edge.faces {
        let n = mesh.face_normal(f);
        let Some(w) = inward(mesh, f, edge.v0, edge.v1, n) else {
            continue;
        };
        let (u, v) = *basis.get_or_insert((w, axis.cross(w)));
        let angle = w.dot(v).atan2(w.dot(u));
        let spin = if axis.cross(w).dot(n) > 0. { -1. } else { 1. };
        info.push((f, angle, spin, w, n));
    }
    let mut chosen = BTreeMap::new();
    for &(f, angle, spin, w, _) in &info {
        let mut best = None;
        for &(g, phi, _, _, n) in &info {
            if f == g {
                continue;
            }
            let mut delta = ((phi - angle) * spin).rem_euclid(std::f64::consts::TAU);
            if delta < 1e-3 || std::f64::consts::TAU - delta < 1e-3 {
                delta = std::f64::consts::TAU;
            }
            let key = (
                (delta / 1e-3).round() as i64,
                if n.dot(w) < 0. { 0 } else { 1 },
            );
            if best.as_ref().map(|&(old, _)| key < old).unwrap_or(true) {
                best = Some((key, g));
            }
        }
        if let Some((_, g)) = best {
            chosen.insert(f, g);
        }
    }
    chosen
        .iter()
        .filter(|&(f, g)| chosen.get(g) == Some(f))
        .map(|(&f, &g)| (f, g))
        .collect()
}
fn is_solid(mesh: &Mesh, faces: &[usize]) -> bool {
    if faces.len() < 4 {
        return false;
    }
    let mut uses = BTreeMap::new();
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for &f in faces {
        for ring in std::iter::once(&mesh.faces[f].vertices).chain(&mesh.faces[f].holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                *uses.entry(if a < b { (a, b) } else { (b, a) }).or_insert(0) += 1;
                lo = lo.min(mesh.vertices[a].position);
                hi = hi.max(mesh.vertices[a].position);
            }
        }
    }
    uses.values().all(|count| count % 2 == 0) && (hi - lo).min_element() >= 0.001
}
pub fn solid_parts(mesh: &Mesh) -> Result<Vec<Piece>, String> {
    planar::validate_mesh(mesh)?;
    let mut dsu = Dsu::new(mesh.faces.len());
    for e in 0..mesh.edges.len() {
        for (f, g) in partners(mesh, e) {
            dsu.join(f, g);
        }
    }
    let mut groups = BTreeMap::new();
    for i in 0..mesh.faces.len() {
        groups.entry(dsu.find(i)).or_insert_with(Vec::new).push(i);
    }
    let comps: Vec<Vec<usize>> = groups.into_values().collect();
    let solid: Vec<_> = comps.iter().map(|fs| is_solid(mesh, fs)).collect();
    let mut owner = BTreeMap::new();
    for (i, fs) in comps.iter().enumerate() {
        if solid[i] {
            for &f in fs {
                for v in face_vertices(mesh, f) {
                    owner.entry(v).or_insert(i);
                }
            }
        }
    }
    let mut fragments = Dsu::new(comps.len());
    let mut corners = BTreeMap::new();
    for (i, fs) in comps.iter().enumerate() {
        if !solid[i] {
            for &f in fs {
                for v in face_vertices(mesh, f) {
                    if let Some(&other) = corners.get(&v) {
                        fragments.join(i, other);
                    } else {
                        corners.insert(v, i);
                    }
                }
            }
        }
    }
    let mut clusters = BTreeMap::new();
    for i in 0..comps.len() {
        if !solid[i] {
            clusters
                .entry(fragments.find(i))
                .or_insert_with(Vec::new)
                .push(i);
        }
    }
    let mut pieces: BTreeMap<usize, Vec<usize>> = comps
        .iter()
        .enumerate()
        .filter(|(i, _)| solid[*i])
        .map(|(i, fs)| (i, fs.clone()))
        .collect();
    for members in clusters.into_values() {
        let faces: Vec<_> = members
            .iter()
            .flat_map(|&i| comps[i].iter().copied())
            .collect();
        let vertices: BTreeSet<_> = faces.iter().flat_map(|&f| face_vertices(mesh, f)).collect();
        let mut votes = BTreeMap::new();
        for v in &vertices {
            if let Some(&solid) = owner.get(v) {
                *votes.entry(solid).or_insert(0) += 1;
            }
        }
        let best = votes
            .into_iter()
            .max_by_key(|&(i, count)| (count, std::cmp::Reverse(i)));
        let target = best
            .filter(|&(_, count)| count * 2 >= vertices.len())
            .map(|(i, _)| i)
            .unwrap_or(comps.len() + members[0]);
        pieces.entry(target).or_default().extend(faces);
    }
    let mut out: Vec<_> = pieces
        .into_values()
        .map(|faces| Piece {
            faces,
            edges: vec![],
        })
        .collect();
    let mut face_piece = BTreeMap::new();
    let mut vertex_piece = BTreeMap::new();
    for (i, p) in out.iter().enumerate() {
        for &f in &p.faces {
            face_piece.insert(f, i);
            for v in face_vertices(mesh, f) {
                vertex_piece.entry(v).or_insert(i);
            }
        }
    }
    let mut wires = vec![];
    for (i, e) in mesh.edges.iter().enumerate() {
        if !e.faces.is_empty() {
            let owners: BTreeSet<_> = e
                .faces
                .iter()
                .filter_map(|f| face_piece.get(f).copied())
                .collect();
            for n in owners {
                out[n].edges.push(i);
            }
        } else if let Some(&n) = vertex_piece.get(&e.v0).or_else(|| vertex_piece.get(&e.v1)) {
            out[n].edges.push(i);
        } else {
            wires.push(i);
        }
    }
    let mut runs = Dsu::new(mesh.vertices.len());
    for &i in &wires {
        let e = &mesh.edges[i];
        runs.join(e.v0, e.v1);
    }
    let mut groups = BTreeMap::new();
    for i in wires {
        groups
            .entry(runs.find(mesh.edges[i].v0))
            .or_insert_with(Vec::new)
            .push(i);
    }
    out.extend(groups.into_values().map(|edges| Piece {
        faces: vec![],
        edges,
    }));
    out.sort_by_key(|p| {
        (
            std::cmp::Reverse(p.faces.len()),
            std::cmp::Reverse(p.edges.len()),
        )
    });
    Ok(out)
}
pub fn copy_part(mesh: &Mesh, part: &Piece) -> Result<Mesh, String> {
    planar::validate_mesh(mesh)?;
    let mut out = Mesh::new();
    for &f in &part.faces {
        let face = mesh.faces.get(f).ok_or("Invalid piece face")?;
        planar::add_face(&mut out, &planar::rings(mesh, face), face)?;
    }
    for &e in &part.edges {
        let edge = mesh.edges.get(e).ok_or("Invalid piece edge")?;
        let a = out.add_vertex(mesh.vertices[edge.v0].position);
        let b = out.add_vertex(mesh.vertices[edge.v1].position);
        let id = out.add_edge(a, b).ok_or("Invalid piece edge boundary")?;
        let (v0, v1, faces) = (
            out.edges[id].v0,
            out.edges[id].v1,
            out.edges[id].faces.clone(),
        );
        out.edges[id] = edge.clone();
        out.edges[id].v0 = v0;
        out.edges[id].v1 = v1;
        out.edges[id].faces = faces;
    }
    out.preserve_vertex_uids_from(mesh);
    Ok(out)
}
fn flatten(
    group: &Group,
    transform: DMat4,
    mesh: &mut Mesh,
    next_curve: &mut usize,
    inherited_layer: Option<&str>,
    inherited_paint: Option<&serde_json::Value>,
) -> Result<(), String> {
    if !transform.is_finite()
        || transform.determinant().abs() < 1e-12
        || transform.x_axis.w.abs() > 1e-12
        || transform.y_axis.w.abs() > 1e-12
        || transform.z_axis.w.abs() > 1e-12
        || (transform.w_axis.w - 1.).abs() > 1e-12
    {
        return Err("Invalid piece container transform".into());
    }
    planar::validate_mesh(&group.mesh)?;
    let layer = group.layer.as_deref().or(inherited_layer);
    let paint = group
        .native_document
        .get("material")
        .filter(|p| p.is_object())
        .or(inherited_paint);
    let mut curves = BTreeMap::new();
    let mut transformed = group.mesh.clone();
    for v in &mut transformed.vertices {
        v.position = transform.transform_point3(v.position);
    }
    if transform.determinant() < 0. {
        for face in &mut transformed.faces {
            face.vertices.reverse();
            for hole in &mut face.holes {
                hole.reverse();
            }
        }
    }
    let inverse = transform.inverse().transpose();
    for f in &mut transformed.faces {
        if group.hidden {
            f.attrs.insert("hidden".into(), serde_json::json!(true));
        }
        f.layer = f.layer.clone().or_else(|| layer.map(str::to_owned));
        if !f.attrs.contains_key("color") && !f.attrs.contains_key("texture") {
            if let Some(paint) = paint {
                for key in ["color", "texture", "mat", "opacity"] {
                    if let Some(value) = paint.get(key) {
                        f.attrs.insert(key.into(), value.clone());
                    }
                }
            }
        }
        if let Some(texture) = f.attrs.get_mut("texture") {
            if let Some(uvw) = texture
                .get("uvw")
                .and_then(|v| v.as_array())
                .filter(|v| v.len() == 8)
                .and_then(|v| v.iter().map(|v| v.as_f64()).collect::<Option<Vec<_>>>())
            {
                let mut mapped = uvw.clone();
                for base in [0, 4] {
                    let gradient = inverse.transform_vector3(DVec3::new(
                        uvw[base],
                        uvw[base + 1],
                        uvw[base + 2],
                    ));
                    mapped[base] = gradient.x;
                    mapped[base + 1] = gradient.y;
                    mapped[base + 2] = gradient.z;
                    mapped[base + 3] = uvw[base + 3] - gradient.dot(transform.w_axis.truncate());
                }
                texture["uvw"] = serde_json::json!(mapped);
            }
        }
    }
    for e in &mut transformed.edges {
        e.hidden |= group.hidden;
        e.layer = e.layer.clone().or_else(|| layer.map(str::to_owned));
        if let Some(old) = e.curve_id {
            let id = if let Some(&id) = curves.get(&old) {
                id
            } else {
                let id = *next_curve;
                *next_curve = next_curve
                    .checked_add(1)
                    .ok_or("Curve identity exhausted")?;
                curves.insert(old, id);
                id
            };
            e.curve_id = Some(id);
            if e.attrs.contains_key("curve") {
                e.attrs.insert("curve".into(), serde_json::json!(id));
            }
        }
    }
    let copy = copy_part(
        &transformed,
        &Piece {
            faces: (0..transformed.faces.len()).collect(),
            edges: (0..transformed.edges.len()).collect(),
        },
    )?;
    for face in &copy.faces {
        planar::add_face(mesh, &planar::rings(&copy, face), face)?;
    }
    for edge in &copy.edges {
        let a = mesh.add_vertex(copy.vertices[edge.v0].position);
        let b = mesh.add_vertex(copy.vertices[edge.v1].position);
        let id = mesh.add_edge(a, b).ok_or("Collapsed piece edge")?;
        let (v0, v1, faces) = (
            mesh.edges[id].v0,
            mesh.edges[id].v1,
            mesh.edges[id].faces.clone(),
        );
        mesh.edges[id] = edge.clone();
        mesh.edges[id].v0 = v0;
        mesh.edges[id].v1 = v1;
        mesh.edges[id].faces = faces;
    }
    mesh.preserve_vertex_uids_from(&transformed);
    for child in &group.children {
        let mut child = child.clone();
        child.hidden |= group.hidden;
        flatten(
            &child,
            transform * child.transform,
            mesh,
            next_curve,
            layer,
            paint,
        )?;
    }
    Ok(())
}
fn generic(name: &str) -> bool {
    let mut tokens = name.split_whitespace();
    let a = tokens.next().unwrap_or("").to_lowercase();
    let b = tokens.next();
    matches!(a.as_str(), "part" | "parte" | "piece" | "pieza")
        && b.and_then(|b| b.parse::<usize>().ok()).is_some()
        && tokens.next().is_none()
}
/// New child placements in the selected container's own coordinates.
pub fn split_into_pieces(group: &Group) -> Result<Vec<Group>, String> {
    if !group.transform.is_finite() || group.transform.determinant().abs() < 1e-12 {
        return Err("Invalid selected container transform".into());
    }
    let mut local = Mesh::new();
    let mut root = group.clone();
    root.transform = DMat4::IDENTITY;
    flatten(&root, DMat4::IDENTITY, &mut local, &mut 0, None, None)?;
    let parts = solid_parts(&local)?;
    if parts.len() <= 1 && group.children.is_empty() {
        return Ok(vec![]);
    }
    if group.mesh.faces.is_empty()
        && parts.len() == group.children.len()
        && group.children.iter().all(|g| {
            g.children.is_empty() && solid_parts(&g.mesh).map(|p| p.len() == 1).unwrap_or(false)
        })
    {
        return Ok(vec![]);
    }
    let key = |p: DVec3| {
        (
            (p.x * 1e5).round() as i64,
            (p.y * 1e5).round() as i64,
            (p.z * 1e5).round() as i64,
        )
    };
    let mut owners = BTreeMap::new();
    for (i, child) in group.children.iter().enumerate() {
        let mut mesh = Mesh::new();
        flatten(child, child.transform, &mut mesh, &mut 0, None, None)?;
        for v in &mesh.vertices {
            owners
                .entry(key(v.position))
                .or_insert_with(BTreeSet::new)
                .insert(i);
        }
    }
    let mut used = BTreeMap::new();
    let mut out = vec![];
    for (i, part) in parts.iter().enumerate() {
        let mut owner: Option<BTreeSet<usize>> = None;
        for &f in &part.faces {
            for &v in &local.faces[f].vertices {
                let current = owners
                    .get(&key(local.vertices[v].position))
                    .cloned()
                    .unwrap_or_default();
                owner = Some(if let Some(old) = owner {
                    old.intersection(&current).copied().collect()
                } else {
                    current
                });
            }
        }
        let name = owner.filter(|s| s.len() == 1).and_then(|s| {
            let name = group.children[*s.first().unwrap()].name.trim();
            (!name.is_empty() && !generic(name)).then(|| name.to_owned())
        });
        let name = if let Some(name) = name {
            let count = used.entry(name.clone()).or_insert(0);
            *count += 1;
            if *count > 1 {
                format!("{name} {count}")
            } else {
                name
            }
        } else {
            format!("Piece {}", i + 1)
        };
        out.push(Group::with_mesh(name, copy_part(&local, part)?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const QUADS: [[usize; 4]; 6] = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    fn cube(mesh: &mut Mesh, x: f64, z: f64, quads: &[[usize; 4]]) {
        let mut vs = vec![];
        for dz in [0., 1.] {
            for dy in [0., 1.] {
                for dx in [0., 1.] {
                    vs.push(mesh.add_vertex(DVec3::new(x + dx, dy, z + dz)));
                }
            }
        }
        for q in quads {
            mesh.add_face(q.iter().map(|&i| vs[i]).collect(), vec![]);
        }
    }
    #[test]
    fn touching_boards_are_three_closed_pieces_without_neighbours_edges() {
        let mut mesh = Mesh::new();
        cube(&mut mesh, 0., 0., &QUADS);
        cube(&mut mesh, 0., 1., &QUADS);
        cube(&mut mesh, 1., 0., &QUADS);
        assert_eq!(connected_parts(&mesh).unwrap().len(), 1);
        let parts = solid_parts(&mesh).unwrap();
        assert_eq!(
            parts.iter().map(|p| p.faces.len()).collect::<Vec<_>>(),
            vec![6, 6, 6]
        );
        let pieces = split_into_pieces(&Group::with_mesh("cabinet", mesh)).unwrap();
        assert_eq!(pieces.len(), 3);
        for p in pieces {
            assert_eq!(p.mesh.edges.len(), 12);
            assert!((p.mesh.volume().unwrap() - 1.).abs() < 1e-8);
            assert!(p.component_definition.is_none());
        }
    }
    #[test]
    fn child_fragments_join_names_and_flags_follow_correct_physical_parts() {
        let mut a = Mesh::new();
        let mut b = Mesh::new();
        cube(&mut a, 0., 0., &QUADS[..3]);
        cube(&mut b, 0., 0., &QUADS[3..]);
        let mut group = Group::new("stool");
        group.children = vec![Group::with_mesh("paintA", a), Group::with_mesh("paintB", b)];
        let parts = split_into_pieces(&group).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].mesh.faces.len(), 6);
        let mut a = Mesh::new();
        cube(&mut a, 0., 0., &QUADS);
        cube(&mut a, 10., 0., &QUADS);
        a.edges[0].soft = true;
        a.edges[0].hidden = true;
        a.edges[0]
            .attrs
            .insert("opaque".into(), serde_json::json!({"retain":1}));
        let mut b = Mesh::new();
        cube(&mut b, 3., 0., &QUADS);
        let mut c = Mesh::new();
        cube(&mut c, 6., 0., &QUADS);
        group.children = vec![
            Group::with_mesh("Carcass", a),
            Group::with_mesh("Worktop", b),
            Group::with_mesh("Part 7", c),
        ];
        let parts = split_into_pieces(&group).unwrap();
        let mut names: Vec<_> = parts.iter().map(|g| g.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["Carcass", "Carcass 2", "Piece 4", "Worktop"]);
        assert_eq!(
            parts
                .iter()
                .flat_map(|p| &p.mesh.edges)
                .filter(|e| e.hidden && e.soft && e.attrs.contains_key("opaque"))
                .count(),
            1
        );
        let mut regroup = group.clone();
        regroup.mesh = Mesh::new();
        regroup.children = parts;
        assert!(split_into_pieces(&regroup).unwrap().is_empty());
    }
    #[test]
    fn open_corner_patch_joins_one_solid_and_disjoint_sheet_stays_separate() {
        let mut mesh = Mesh::new();
        cube(&mut mesh, 0., 0., &QUADS);
        let ring = [
            DVec3::new(0.2, 0., 0.2),
            DVec3::new(0.8, 0., 0.2),
            DVec3::new(0.8, 0., 0.8),
            DVec3::new(0.2, 0., 0.8),
        ]
        .into_iter()
        .map(|p| mesh.add_vertex(p))
        .collect();
        mesh.add_face(ring, vec![]);
        let ring = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.)]
            .into_iter()
            .map(|p| mesh.add_vertex(p))
            .collect();
        mesh.add_face(ring, vec![]);
        let parts = solid_parts(&mesh).unwrap();
        let mut sizes: Vec<_> = parts.iter().map(|p| p.faces.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![1, 7]);
    }
}
