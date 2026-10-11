//! Original OBJ editable/reference geometry policies. Unpublished candidates only.
use glam::{DVec3, Vec3};
use ingetrazo_core::mesh::{Mesh, MeshFace};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

type Attrs = HashMap<String, Value>;
fn newell(mesh: &Mesh, ring: &[usize]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for i in 0..ring.len() {
        let a = mesh.vertices[ring[i]].position;
        let b = mesh.vertices[ring[(i + 1) % ring.len()]].position;
        n += Vec3::new(
            ((a.y - b.y) * (a.z + b.z)) as f32,
            ((a.z - b.z) * (a.x + b.x)) as f32,
            ((a.x - b.x) * (a.y + b.y)) as f32,
        );
    }
    n
}
fn qt_length(n: Vec3) -> f32 {
    let m = n.abs().max_element();
    if m == 0. {
        0.
    } else {
        let p = n / m;
        m * (p.x * p.x + p.y * p.y + p.z * p.z).sqrt()
    }
}
fn face_normal(mesh: &Mesh, id: usize) -> Vec3 {
    let ring = &mesh.faces[id].vertices;
    let mut n = DVec3::ZERO;
    for i in 0..ring.len() {
        let a = mesh.vertices[ring[i]].position;
        let b = mesh.vertices[ring[(i + 1) % ring.len()]].position;
        n += DVec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    let n = n.as_vec3();
    let l = qt_length(n);
    if l < 1e-9 {
        Vec3::Z
    } else {
        n / l
    }
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    (a.x * b.x + a.y * b.y + a.z * b.z) as f64
}
fn uvw(attrs: &Attrs) -> Option<Vec<f64>> {
    attrs
        .get("texture")?
        .get("uvw")?
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect()
}
fn signature(attrs: &Attrs) -> Attrs {
    let mut out = attrs.clone();
    if let Some(values) = out
        .get_mut("texture")
        .and_then(|t| t.get_mut("uvw"))
        .and_then(Value::as_array_mut)
    {
        for value in values {
            if let Some(n) = value.as_f64() {
                *value = Value::from(
                    format!("{n:.4}")
                        .parse::<f64>()
                        .expect("finite authored UV fit"),
                );
            }
        }
    }
    out
}
fn material_equal(a: &Attrs, b: &Attrs) -> bool {
    if a == b {
        return true;
    }
    if a.get("color") != b.get("color") {
        return false;
    }
    if a.iter()
        .any(|(k, v)| k != "color" && k != "texture" && b.get(k) != Some(v))
        || b.iter()
            .any(|(k, v)| k != "color" && k != "texture" && a.get(k) != Some(v))
    {
        return false;
    }
    let (Some(ta), Some(tb)) = (a.get("texture"), b.get("texture")) else {
        return false;
    };
    if ta.get("path") != tb.get("path") {
        return false;
    }
    if let (Some(ua), Some(ub)) = (uvw(a), uvw(b)) {
        if ua.is_empty() || ub.is_empty() {
            return false;
        }
        // Original zip deliberately ignores trailing values; import fits eight.
        return ua
            .iter()
            .zip(&ub)
            .all(|(a, b)| (a - b).abs() <= 0.02 + 0.01 * a.abs().max(b.abs()));
    }
    // Exact source texture signature ignores extension keys and defaults rot0.
    ["sw", "sh"].iter().all(|k| ta.get(*k) == tb.get(*k))
        && ta.get("rot").unwrap_or(&Value::from(0)) == tb.get("rot").unwrap_or(&Value::from(0))
        && ta.get("uvw").is_none()
        && tb.get("uvw").is_none()
}
fn smooth_equal(a: &Attrs, b: &Attrs) -> bool {
    a.get("color") == b.get("color")
        && a.get("texture").and_then(|v| v.get("path"))
            == b.get("texture").and_then(|v| v.get("path"))
}
fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}
fn trace(mesh: &Mesh, members: &[usize], n: Vec3) -> Option<(Vec<usize>, Vec<Vec<usize>>)> {
    let mut count = HashMap::<(usize, usize), usize>::new();
    let mut order = Vec::new();
    for &f in members {
        for ring in std::iter::once(&mesh.faces[f].vertices).chain(&mesh.faces[f].holes) {
            for i in 0..ring.len() {
                let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                if a == b {
                    return None;
                }
                let key = (a.min(b), a.max(b));
                if !count.contains_key(&key) {
                    order.push((a, b));
                }
                *count.entry(key).or_default() += 1;
            }
        }
    }
    if count.values().any(|&c| c > 2) {
        return None;
    }
    let mut adj = HashMap::<usize, Vec<usize>>::new();
    let mut unused = HashSet::new();
    for &(a, b) in &order {
        let key = (a.min(b), a.max(b));
        if count[&key] == 1 {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
            unused.insert(key);
        }
    }
    if unused.is_empty() || adj.values().any(|v| v.len() != 2) {
        return None;
    }
    let mut rings = Vec::new();
    for &(a, b) in &order {
        if !unused.remove(&(a.min(b), a.max(b))) {
            continue;
        }
        let mut ring = vec![a, b];
        let (mut prev, mut cur) = (a, b);
        loop {
            let neighbors = &adj[&cur];
            let next = if neighbors[0] == prev {
                neighbors[1]
            } else {
                neighbors[0]
            };
            if next == a {
                unused.remove(&(cur.min(next), cur.max(next)));
                break;
            }
            if !unused.remove(&(cur.min(next), cur.max(next))) || ring.len() > adj.len() {
                return None;
            }
            ring.push(next);
            prev = cur;
            cur = next;
        }
        if ring.len() < 3 {
            return None;
        }
        rings.push(ring);
    }
    rings.sort_by(|a, b| qt_length(newell(mesh, b)).total_cmp(&qt_length(newell(mesh, a))));
    let mut outer = rings.remove(0);
    if dot(newell(mesh, &outer), n) < 0. {
        outer.reverse();
    }
    for ring in &mut rings {
        if dot(newell(mesh, ring), n) > 0. {
            ring.reverse();
        }
    }
    Some((outer, rings))
}
// Raw loop keys are the source fusion grid, distinct from Mesh's tolerant
// registry. No native vertices, edges or history IDs exist until emission.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct RawKey([u64; 3]);
fn raw_key(p: DVec3) -> RawKey {
    RawKey(p.to_array().map(|x| {
        let rounded = format!("{x:.4}")
            .parse::<f64>()
            .expect("finite authored point");
        if rounded == 0. {
            0
        } else {
            rounded.to_bits()
        }
    }))
}
fn raw_newell(points: &[DVec3]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        n += Vec3::new(
            ((a.y - b.y) * (a.z + b.z)) as f32,
            ((a.z - b.z) * (a.x + b.x)) as f32,
            ((a.x - b.x) * (a.y + b.y)) as f32,
        );
    }
    n
}
struct RawFace<'a> {
    points: &'a [DVec3],
    attrs: &'a Attrs,
    keys: Vec<usize>,
    normal: Vec3,
    signature: Attrs,
}
/// Borrow parsed loops, fuse on source round-four keys, then weld only output.
pub(super) fn reference<'a>(
    loops: impl Iterator<Item = (&'a [DVec3], &'a Attrs)>,
) -> Result<Mesh, String> {
    reference_with_admission(loops, |_, _| Ok(()))
}
fn reference_with_admission<'a>(
    loops: impl Iterator<Item = (&'a [DVec3], &'a Attrs)>,
    mut admit: impl FnMut(&[DVec3], &[Vec<DVec3>]) -> Result<(), String>,
) -> Result<Mesh, String> {
    let mut faces = Vec::<RawFace<'a>>::new();
    let mut keys = HashMap::<RawKey, usize>::new();
    let mut triangles = HashMap::<Vec<usize>, usize>::new();
    for (points, attrs) in loops {
        if points.len() < 3 {
            continue;
        }
        if points.iter().any(|p| !p.is_finite()) {
            return Err("Nonfinite imported raw loop".into());
        }
        let n = raw_newell(points);
        let length = qt_length(n);
        if length < 1e-10 {
            continue;
        }
        let ids = points
            .iter()
            .map(|&p| {
                let next = keys.len();
                *keys.entry(raw_key(p)).or_insert(next)
            })
            .collect::<Vec<_>>();
        let face = RawFace {
            points,
            attrs,
            keys: ids,
            normal: n / length,
            signature: signature(attrs),
        };
        if points.len() == 3 {
            let mut key = face.keys.clone();
            key.sort_unstable();
            key.dedup();
            if let Some(&slot) = triangles.get(&key) {
                let rank = |a: &Attrs| if a.contains_key("texture") { 2 } else { 1 };
                if rank(attrs) > rank(faces[slot].attrs) {
                    faces[slot] = face;
                }
                continue;
            }
            triangles.insert(key, faces.len());
        }
        faces.push(face);
    }
    let mut parent = (0..faces.len()).collect::<Vec<_>>();
    let mut edges = HashMap::<(usize, usize), Vec<usize>>::new();
    let mut order = Vec::new();
    for (slot, face) in faces.iter().enumerate() {
        let r = &face.keys;
        for i in 0..r.len() {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            if a == b {
                continue;
            }
            let key = (a.min(b), a.max(b));
            if !edges.contains_key(&key) {
                order.push(key);
            }
            edges.entry(key).or_default().push(slot);
        }
    }
    for key in order {
        let ids = &edges[&key];
        if ids.len() != 2 {
            continue;
        }
        let (a, b) = (ids[0], ids[1]);
        if dot(faces[a].normal, faces[b].normal).abs() < 0.99999
            || !material_equal(&faces[a].signature, &faces[b].signature)
        {
            continue;
        }
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb
            && dot(faces[ra].normal, faces[rb].normal).abs() >= 0.99999
            && material_equal(&faces[ra].signature, &faces[rb].signature)
        {
            parent[rb] = ra;
        }
    }
    let mut regions = Vec::<Vec<usize>>::new();
    let mut slots = HashMap::new();
    for i in 0..faces.len() {
        let root = find(&mut parent, i);
        let next = regions.len();
        let slot = *slots.entry(root).or_insert_with(|| {
            regions.push(Vec::new());
            next
        });
        regions[slot].push(i);
    }
    let mut output = Mesh::new();
    for ids in regions {
        let first = &faces[ids[0]];
        if ids.len() == 1 {
            if admit(first.points, &[])
                .and_then(|()| append_raw(&mut output, first.points, &[], first.attrs))
                .is_err()
            {
                if admit(first.points, &[]).is_ok() {
                    let _ = append_raw(&mut output, first.points, &[], first.attrs);
                }
            }
            continue;
        }
        let traced = trace_raw(&faces, &ids, first.normal);
        if traced.is_none() {
            // fuse_coplanar_loops emits separate singleton candidates here,
            // preserving each original's attrs (unlike _add_fused's region
            // admission fallback below).
            for &id in &ids {
                let original = &faces[id];
                if admit(original.points, &[])
                    .and_then(|()| append_raw(&mut output, original.points, &[], original.attrs))
                    .is_err()
                    && admit(original.points, &[]).is_ok()
                {
                    let _ = append_raw(&mut output, original.points, &[], original.attrs);
                }
            }
            continue;
        }
        let accepted = if let Some((outer, holes)) = traced {
            admit(&outer, &holes)
                .and_then(|()| append_raw(&mut output, &outer, &holes, first.attrs))
                .is_ok()
        } else {
            false
        };
        if !accepted {
            // _add_fused retries each original with the REGION's paint. An
            // individual refusal does not discard the other original loops.
            for &id in &ids {
                let original = &faces[id];
                if admit(original.points, &[]).is_ok() {
                    let _ = append_raw(&mut output, original.points, &[], first.attrs);
                }
            }
        }
    }
    soften(&mut output, true);
    Ok(output)
}
fn trace_raw(
    faces: &[RawFace<'_>],
    ids: &[usize],
    n: Vec3,
) -> Option<(Vec<DVec3>, Vec<Vec<DVec3>>)> {
    let mut counts = HashMap::<(usize, usize), usize>::new();
    let mut order = Vec::new();
    let mut positions = HashMap::new();
    for &id in ids {
        let face = &faces[id];
        for i in 0..face.keys.len() {
            let (a, b) = (face.keys[i], face.keys[(i + 1) % face.keys.len()]);
            if a == b {
                continue;
            }
            let key = (a.min(b), a.max(b));
            if !counts.contains_key(&key) {
                order.push((a, b));
            }
            *counts.entry(key).or_default() += 1;
            positions.entry(a).or_insert(face.points[i]);
            positions
                .entry(b)
                .or_insert(face.points[(i + 1) % face.keys.len()]);
        }
    }
    if counts.values().any(|&c| c > 2) {
        return None;
    }
    let mut adj = HashMap::<usize, Vec<usize>>::new();
    let mut unused = HashSet::new();
    for &(a, b) in &order {
        let key = (a.min(b), a.max(b));
        if counts[&key] == 1 {
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
            unused.insert(key);
        }
    }
    if unused.is_empty() || adj.values().any(|r| r.len() != 2) {
        return None;
    }
    let mut rings = Vec::new();
    for &(a, b) in &order {
        if !unused.remove(&(a.min(b), a.max(b))) {
            continue;
        }
        let mut ring = vec![a, b];
        let (mut prev, mut cur) = (a, b);
        loop {
            let r = &adj[&cur];
            let next = if r[0] == prev { r[1] } else { r[0] };
            if next == a {
                unused.remove(&(cur.min(next), cur.max(next)));
                break;
            }
            if !unused.remove(&(cur.min(next), cur.max(next))) || ring.len() > adj.len() {
                return None;
            }
            ring.push(next);
            prev = cur;
            cur = next;
        }
        if ring.len() < 3 {
            return None;
        }
        rings.push(ring.into_iter().map(|v| positions[&v]).collect::<Vec<_>>());
    }
    rings.sort_by(|a, b| qt_length(raw_newell(b)).total_cmp(&qt_length(raw_newell(a))));
    let mut outer = rings.remove(0);
    if dot(raw_newell(&outer), n) < 0. {
        outer.reverse();
    }
    for ring in &mut rings {
        if dot(raw_newell(ring), n) > 0. {
            ring.reverse();
        }
    }
    Some((outer, rings))
}
fn append_raw(
    out: &mut Mesh,
    outer: &[DVec3],
    holes: &[Vec<DVec3>],
    attrs: &Attrs,
) -> Result<(), String> {
    if outer.len() < 3
        || outer
            .iter()
            .chain(holes.iter().flatten())
            .any(|p| !p.is_finite())
    {
        return Err("Invalid imported reference ring".into());
    }
    let vertices = outer.iter().map(|&p| out.add_vertex(p)).collect();
    let holes = holes
        .iter()
        .map(|ring| ring.iter().map(|&p| out.add_vertex(p)).collect())
        .collect();
    let id = out
        .add_face(vertices, holes)
        .ok_or("Native reference face admission refused")?;
    out.faces[id].attrs = attrs.clone();
    Ok(())
}

pub(super) fn soften(mesh: &mut Mesh, automatic: bool) {
    let normals = (0..mesh.faces.len())
        .map(|i| face_normal(mesh, i))
        .collect::<Vec<_>>();
    for e in &mut mesh.edges {
        if e.soft || e.faces.len() != 2 || e.faces[0] == e.faces[1] {
            continue;
        }
        let (a, b) = (e.faces[0], e.faces[1]);
        let (aa, bb) = (&mesh.faces[a].attrs, &mesh.faces[b].attrs);
        if !smooth_equal(aa, bb) {
            continue;
        }
        let same_group = aa
            .get("_s")
            .filter(|v| v.as_u64().is_some_and(|g| g != 0))
            .is_some_and(|g| bb.get("_s") == Some(g));
        let d = dot(normals[a], normals[b]).abs();
        if (same_group && (normals[a] == Vec3::ZERO || normals[b] == Vec3::ZERO || d > 0.17))
            || (automatic && d > 0.85)
        {
            e.soft = true;
        }
    }
    for face in &mut mesh.faces {
        face.attrs.remove("_s");
    }
}

/// Rebuild derived edges once, moving unchanged records/tokens rather than
/// regenerating owner data. Removed interior seams are not retained as wires.
fn rebuild_edges(mesh: &mut Mesh, removed: &HashSet<(usize, usize)>) {
    let old = std::mem::take(&mut mesh.edges);
    let mut records = old
        .into_iter()
        .map(|e| ((e.v0.min(e.v1), e.v0.max(e.v1)), e))
        .collect::<HashMap<_, _>>();
    for v in &mut mesh.vertices {
        v.edges.clear();
    }
    for face in 0..mesh.faces.len() {
        let rings = std::iter::once(&mesh.faces[face].vertices).chain(&mesh.faces[face].holes);
        let mut links = Vec::new();
        for ring in rings {
            for i in 0..ring.len() {
                links.push((ring[i], ring[(i + 1) % ring.len()]));
            }
        }
        for (a, b) in links {
            let id = mesh.add_edge(a, b).expect("validated imported edge");
            if mesh.edges[id].faces.is_empty() {
                if let Some(mut e) = records.remove(&(a.min(b), a.max(b))) {
                    e.faces.clear();
                    mesh.edges[id] = e;
                }
            }
            if mesh.edges[id].faces.last() != Some(&face) {
                mesh.edges[id].faces.push(face);
            }
        }
    }
    for (key, mut e) in records {
        if e.faces.is_empty() && !removed.contains(&key) {
            let id = mesh.add_edge(e.v0, e.v1).expect("validated loose edge");
            e.faces.clear();
            mesh.edges[id] = e;
        }
    }
    mesh.rebuild_incidence();
}
fn split_stations(mesh: &mut Mesh, seed: &[DVec3]) -> Result<(), String> {
    if seed.is_empty() {
        return Ok(());
    }
    let lo = seed
        .iter()
        .copied()
        .fold(DVec3::splat(f64::INFINITY), DVec3::min)
        - DVec3::splat(1e-4);
    let hi = seed
        .iter()
        .copied()
        .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max)
        + DVec3::splat(1e-4);
    let mut sorted = (0..mesh.vertices.len()).collect::<Vec<_>>();
    sorted.sort_by(|&a, &b| {
        mesh.vertices[a]
            .position
            .x
            .total_cmp(&mesh.vertices[b].position.x)
    });
    let mut splits = HashMap::<(usize, usize), Vec<usize>>::new();
    for e in &mesh.edges {
        let (a, b) = (mesh.vertices[e.v0].position, mesh.vertices[e.v1].position);
        let min = a.min(b);
        let max = a.max(b);
        if min.cmpgt(hi).any() || max.cmplt(lo).any() {
            continue;
        }
        let d = b - a;
        let len = d.length_squared();
        if len < 1e-12 {
            continue;
        }
        let begin = sorted.partition_point(|&v| mesh.vertices[v].position.x < min.x - 1e-4);
        let end = sorted.partition_point(|&v| mesh.vertices[v].position.x <= max.x + 1e-4);
        let mut found = Vec::new();
        for &v in &sorted[begin..end] {
            if v == e.v0 || v == e.v1 {
                continue;
            }
            let p = mesh.vertices[v].position;
            let t = (p - a).dot(d) / len;
            if t > 1e-4 && t < 1. - 1e-4 && (p - a - d * t).length() < 1e-4 {
                found.push((t, v));
            }
        }
        if !found.is_empty() {
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut ids = found.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
            if e.v0 > e.v1 {
                ids.reverse();
            }
            splits.insert((e.v0.min(e.v1), e.v0.max(e.v1)), ids);
        }
    }
    if splits.is_empty() {
        return Ok(());
    }
    for face in &mut mesh.faces {
        for ring in std::iter::once(&mut face.vertices).chain(&mut face.holes) {
            let old = std::mem::take(ring);
            for i in 0..old.len() {
                let (a, b) = (old[i], old[(i + 1) % old.len()]);
                ring.push(a);
                if let Some(stations) = splits.get(&(a.min(b), a.max(b))) {
                    if a < b {
                        ring.extend(stations.iter().copied());
                    } else {
                        ring.extend(stations.iter().rev().copied());
                    }
                }
            }
        }
    }
    rebuild_edges(mesh, &HashSet::new());
    Ok(())
}
fn collapse_stations(mesh: &mut Mesh) -> Result<(), String> {
    use std::collections::BTreeSet;
    let mut adj = vec![Vec::<usize>::new(); mesh.vertices.len()];
    for (i, e) in mesh.edges.iter().enumerate() {
        adj[e.v0].push(i);
        adj[e.v1].push(i);
    }
    let mut dead = vec![false; mesh.edges.len()];
    let mut removed = vec![false; mesh.vertices.len()];
    let mut pending = (0..mesh.vertices.len()).collect::<BTreeSet<_>>();
    while let Some(v) = pending.pop_first() {
        if removed[v] || adj[v].len() != 2 {
            continue;
        }
        let (a, b) = (adj[v][0], adj[v][1]);
        let other = |e: usize| {
            if mesh.edges[e].v0 == v {
                mesh.edges[e].v1
            } else {
                mesh.edges[e].v0
            }
        };
        let (x, y) = (other(a), other(b));
        let d1 = (mesh.vertices[v].position - mesh.vertices[x].position).as_vec3();
        let d2 = (mesh.vertices[y].position - mesh.vertices[v].position).as_vec3();
        let (l1, l2) = (qt_length(d1), qt_length(d2));
        if l1 < 1e-4 || l2 < 1e-4 || dot(d1 / l1, d2 / l2) < 0.999 {
            continue;
        }
        let mut fa = mesh.edges[a].faces.clone();
        fa.sort_unstable();
        fa.dedup();
        let mut fb = mesh.edges[b].faces.clone();
        fb.sort_unstable();
        fb.dedup();
        if fa != fb {
            continue;
        }
        for id in [a, b] {
            dead[id] = true;
            let e = &mesh.edges[id];
            adj[e.v0].retain(|&i| i != id);
            adj[e.v1].retain(|&i| i != id);
        }
        let existing = adj[x]
            .iter()
            .copied()
            .find(|&i| mesh.edges[i].other_vertex(x) == Some(y));
        let id = if let Some(i) = existing {
            i
        } else {
            let i = mesh.edges.len();
            mesh.edges.push(ingetrazo_core::mesh::MeshEdge::new(x, y));
            dead.push(false);
            adj[x].push(i);
            adj[y].push(i);
            i
        };
        for f in fa {
            let face = &mut mesh.faces[f];
            for ring in std::iter::once(&mut face.vertices).chain(&mut face.holes) {
                ring.retain(|&i| i != v);
                if ring.len() < 3 {
                    return Err("Invalid imported station collapse".into());
                }
            }
            if !mesh.edges[id].faces.contains(&f) {
                mesh.edges[id].faces.push(f);
            }
        }
        removed[v] = true;
        pending.insert(x);
        pending.insert(y);
    }
    if !removed.iter().any(|&v| v) {
        return Ok(());
    }
    let mut map = vec![usize::MAX; removed.len()];
    let old = std::mem::take(&mut mesh.vertices);
    for (i, v) in old.into_iter().enumerate() {
        if !removed[i] {
            map[i] = mesh.vertices.len();
            mesh.vertices.push(v);
        }
    }
    mesh.edges = std::mem::take(&mut mesh.edges)
        .into_iter()
        .enumerate()
        .filter_map(|(i, mut e)| {
            if dead[i] {
                None
            } else {
                e.v0 = map[e.v0];
                e.v1 = map[e.v1];
                Some(e)
            }
        })
        .collect();
    for f in &mut mesh.faces {
        for ring in std::iter::once(&mut f.vertices).chain(&mut f.holes) {
            for v in ring {
                *v = map[*v];
            }
        }
    }
    mesh.rebuild_spatial_index();
    mesh.rebuild_incidence();
    Ok(())
}
pub(super) fn editable(mesh: &mut Mesh, first_new: usize, seed: &[DVec3]) -> Result<(), String> {
    ingetrazo_core::planar::validate_mesh(mesh)?;
    // Dedupe original cycles before connectivity repair, retaining authored paint.
    let mut seen = HashMap::<Vec<(usize, usize)>, usize>::new();
    let mut discard = vec![false; mesh.faces.len()];
    let mut fresh = (0..mesh.faces.len())
        .map(|i| i >= first_new)
        .collect::<Vec<_>>();
    for id in 0..mesh.faces.len() {
        let r = &mesh.faces[id].vertices;
        let mut key = (0..r.len())
            .map(|i| {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                (a.min(b), a.max(b))
            })
            .collect::<Vec<_>>();
        key.sort_unstable();
        key.dedup();
        if let Some(&old) = seen.get(&key) {
            let (drop, keep) = if mesh.faces[id].holes.len() > mesh.faces[old].holes.len() {
                (old, id)
            } else {
                (id, old)
            };
            if mesh.faces[keep].attrs.is_empty() {
                mesh.faces[keep].attrs = mesh.faces[drop].attrs.clone();
            }
            discard[drop] = true;
            fresh[keep] |= fresh[drop];
            seen.insert(key, keep);
        } else {
            seen.insert(key, id);
        }
    }
    let mut new_flags = Vec::new();
    mesh.faces = std::mem::take(&mut mesh.faces)
        .into_iter()
        .enumerate()
        .filter_map(|(i, f)| {
            if discard[i] {
                None
            } else {
                new_flags.push(fresh[i]);
                Some(f)
            }
        })
        .collect();
    rebuild_edges(mesh, &HashSet::new());
    split_stations(mesh, seed)?;
    collapse_stations(mesh)?;
    let normals = (0..mesh.faces.len())
        .map(|i| face_normal(mesh, i))
        .collect::<Vec<_>>();
    let mut eligible = vec![false; mesh.faces.len()];
    let key = |p: DVec3| {
        [
            format!("{:.4}", p.x),
            format!("{:.4}", p.y),
            format!("{:.4}", p.z),
        ]
    };
    let seeds = seed.iter().copied().map(key).collect::<HashSet<_>>();
    for (i, f) in mesh.faces.iter().enumerate() {
        eligible[i] = f
            .vertices
            .iter()
            .any(|&v| seeds.contains(&key(mesh.vertices[v].position)));
    }
    let mut face_edges = vec![Vec::new(); mesh.faces.len()];
    for (id, e) in mesh.edges.iter().enumerate() {
        for &f in &e.faces {
            if face_edges[f].last() != Some(&id) {
                face_edges[f].push(id);
            }
        }
    }
    let mut visited = vec![false; mesh.faces.len()];
    let mut regions = Vec::<Vec<usize>>::new();
    for start in 0..mesh.faces.len() {
        if visited[start] || !eligible[start] {
            continue;
        }
        let mut ids = vec![start];
        let mut todo = vec![start];
        visited[start] = true;
        while let Some(id) = todo.pop() {
            for &edge in &face_edges[id] {
                let e = &mesh.edges[edge];
                if e.faces
                    .iter()
                    .any(|&f| dot(normals[start], normals[f]).abs() < 0.999)
                {
                    continue;
                }
                for &f in &e.faces {
                    if !visited[f] && eligible[f] && dot(normals[start], normals[f]).abs() > 0.999 {
                        visited[f] = true;
                        ids.push(f);
                        todo.push(f);
                    }
                }
            }
        }
        if ids.len() > 1 && ids.iter().any(|&i| new_flags[i]) {
            regions.push(ids);
        }
    }
    let mut retired = vec![false; mesh.faces.len()];
    let mut merged = Vec::new();
    let mut removed_edges = HashSet::new();
    for ids in regions {
        if let Some((vertices, holes)) = trace(mesh, &ids, normals[ids[0]]) {
            let dominant = ids
                .iter()
                .copied()
                .filter(|&i| !mesh.faces[i].attrs.is_empty())
                .max_by(|&a, &b| {
                    (qt_length(newell(mesh, &mesh.faces[a].vertices)) as f64)
                        .total_cmp(&(qt_length(newell(mesh, &mesh.faces[b].vertices)) as f64))
                });
            let mut f = MeshFace::new(vertices);
            f.holes = holes;
            if let Some(d) = dominant {
                f.attrs = mesh.faces[d].attrs.clone();
            }
            for &id in &ids {
                retired[id] = true;
                for ring in std::iter::once(&mesh.faces[id].vertices).chain(&mesh.faces[id].holes) {
                    for i in 0..ring.len() {
                        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                        removed_edges.insert((a.min(b), a.max(b)));
                    }
                }
            }
            merged.push(f);
        }
    }
    mesh.faces = std::mem::take(&mut mesh.faces)
        .into_iter()
        .enumerate()
        .filter_map(|(i, f)| if retired[i] { None } else { Some(f) })
        .chain(merged)
        .collect();
    rebuild_edges(mesh, &removed_edges);
    let flat = mesh.faces.is_empty() || {
        let n = face_normal(mesh, 0);
        let origin = mesh.vertices[mesh.faces[0].vertices[0]].position;
        mesh.faces.iter().all(|f| {
            f.vertices
                .iter()
                .all(|&v| (mesh.vertices[v].position - origin).dot(n.as_dvec3()).abs() < 1e-4)
        })
    };
    if flat {
        for f in &mut mesh.faces {
            f.interior = false;
        }
    } else if mesh.edges.iter().all(|edge| edge.faces.len() >= 2) {
        // Original orientation flips only when the entire active mesh is
        // closed, including free wires and other disconnected components.
        ingetrazo_core::heal::orient_boundary_candidate(mesh)?;
    } else {
        ingetrazo_core::heal::refresh_interior(mesh)?;
    }
    soften(mesh, false);
    Ok(())
}

#[cfg(test)]
mod raw_reference_tests {
    use super::*;
    #[test]
    fn trace_refusal_preserves_each_original_paint() {
        let points = [1., 2., -1.].map(|y| [DVec3::ZERO, DVec3::X, DVec3::new(0.5, y, 0.)]);
        let attrs = [0,1,2].map(|i| HashMap::from([("texture".into(), serde_json::json!({"path":"witness.png","uvw":[1.+i as f64*0.000001,0.,0.,0.,0.,1.,0.,0.]}))]));
        let out = reference(points.iter().zip(attrs.iter()).map(|(p, a)| (&p[..], a))).unwrap();
        let actual = serde_json::json!({"attrs":out.faces.iter().map(|f|f.attrs.clone()).collect::<Vec<_>>(),"loops":out.faces.iter().map(|f|f.vertices.iter().map(|&v|out.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()});
        let source: Value = serde_json::from_str(include_str!(
            "../../../docs/audits/obj-raw-fusion/trace-source.json"
        ))
        .unwrap();
        assert_eq!(actual, source);
    }
    #[test]
    fn reference_admission_retries_originals_and_preserves_region_paint() {
        let a = [DVec3::ZERO, DVec3::X, DVec3::new(1., 1., 0.)];
        let b = [DVec3::ZERO, DVec3::new(1., 1., 0.), DVec3::Y];
        let attrs = HashMap::from([
            ("color".into(), serde_json::json!([0.2, 0.3, 0.4])),
            ("opaque".into(), serde_json::json!({"preserve":[1]})),
        ]);
        let source: Value = serde_json::from_str(include_str!(
            "../../../docs/audits/obj-raw-fusion/admission-source.json"
        ))
        .unwrap();
        let mut controls = Vec::new();
        let mut attempts = Vec::new();
        let out = reference_with_admission(
            [(&a[..], &attrs), (&b[..], &attrs)].into_iter(),
            |outer, _| {
                attempts.push(outer.len());
                if outer.len() > 3 {
                    Err("injected fused admission refusal".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(attempts, [4, 3, 3]);
        assert_eq!(out.faces.len(), 2);
        assert!(out.faces.iter().all(|f| f.attrs == attrs));
        assert_eq!(out.vertices.len(), 4);
        controls.push(serde_json::json!({"case":"fused","attempts":attempts,"faces":out.faces.len(),"attrs":out.faces.iter().map(|f|f.attrs.clone()).collect::<Vec<_>>(),"loops":out.faces.iter().map(|f|f.vertices.iter().map(|&v|out.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()}));
        let mut attempts = Vec::new();
        let single = reference_with_admission([(&a[..], &attrs)].into_iter(), |outer, _| {
            attempts.push(outer.len());
            if attempts.len() == 1 {
                Err("injected singleton refusal".into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(attempts, [3, 3]);
        assert_eq!(single.faces.len(), 1);
        assert_eq!(single.faces[0].attrs, attrs);
        controls.push(serde_json::json!({"case":"singleton","attempts":attempts,"faces":single.faces.len(),"attrs":single.faces.iter().map(|f|f.attrs.clone()).collect::<Vec<_>>(),"loops":single.faces.iter().map(|f|f.vertices.iter().map(|&v|single.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()}));
        let mut attempts = Vec::new();
        let partial = reference_with_admission(
            [(&a[..], &attrs), (&b[..], &attrs)].into_iter(),
            |outer, _| {
                attempts.push(outer.len());
                if attempts.len() <= 2 {
                    Err("injected original refusal".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(attempts, [4, 3, 3]);
        assert_eq!(partial.faces.len(), 1);
        assert_eq!(partial.faces[0].attrs, attrs);
        controls.push(serde_json::json!({"case":"partial-original","attempts":attempts,"faces":partial.faces.len(),"attrs":partial.faces.iter().map(|f|f.attrs.clone()).collect::<Vec<_>>(),"loops":partial.faces.iter().map(|f|f.vertices.iter().map(|&v|partial.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()}));
        assert_eq!(serde_json::json!(controls), source);
    }
}
