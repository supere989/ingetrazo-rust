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
fn normal(mesh: &Mesh, face: usize) -> Vec3 {
    let n = newell(mesh, &mesh.faces[face].vertices);
    let l = qt_length(n);
    if l < 1e-10 {
        Vec3::ZERO
    } else {
        n / l
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
/// Reference fusion never touches an existing owner. Rows and attributes are
/// traversed once; adjacency/union roots delimit independently traceable regions.
pub(super) fn reference(mesh: &mut Mesh) -> Result<(), String> {
    let normals = (0..mesh.faces.len())
        .map(|i| normal(mesh, i))
        .collect::<Vec<_>>();
    let mut eligible = Vec::new();
    let mut triangles = HashMap::<Vec<usize>, usize>::new();
    for (id, f) in mesh.faces.iter().enumerate() {
        if normals[id] == Vec3::ZERO {
            continue;
        }
        if f.vertices.len() == 3 {
            let mut key = f.vertices.clone();
            key.sort_unstable();
            key.dedup();
            let rank = |id: usize| {
                if mesh.faces[id].attrs.contains_key("texture") {
                    2
                } else {
                    1
                }
            };
            if let Some(&slot) = triangles.get(&key) {
                if rank(id) > rank(eligible[slot]) {
                    eligible[slot] = id;
                }
                continue;
            }
            triangles.insert(key, eligible.len());
        }
        eligible.push(id);
    }
    let mut parent = (0..eligible.len()).collect::<Vec<_>>();
    let mut edges = HashMap::<(usize, usize), Vec<usize>>::new();
    let mut order = Vec::new();
    for (slot, &id) in eligible.iter().enumerate() {
        let r = &mesh.faces[id].vertices;
        for i in 0..r.len() {
            let (a, b) = (r[i], r[(i + 1) % r.len()]);
            if a == b {
                continue;
            }
            let k = (a.min(b), a.max(b));
            if !edges.contains_key(&k) {
                order.push(k);
            }
            edges.entry(k).or_default().push(slot);
        }
    }
    for key in order {
        let ids = &edges[&key];
        if ids.len() != 2 {
            continue;
        }
        let (a, b) = (ids[0], ids[1]);
        if dot(normals[eligible[a]], normals[eligible[b]]).abs() < 0.99999
            || !material_equal(
                &mesh.faces[eligible[a]].attrs,
                &mesh.faces[eligible[b]].attrs,
            )
        {
            continue;
        }
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb
            && dot(normals[eligible[ra]], normals[eligible[rb]]).abs() >= 0.99999
            && material_equal(
                &mesh.faces[eligible[ra]].attrs,
                &mesh.faces[eligible[rb]].attrs,
            )
        {
            parent[rb] = ra;
        }
    }
    let mut regions = Vec::<Vec<usize>>::new();
    let mut slots = HashMap::new();
    for (slot, &id) in eligible.iter().enumerate() {
        let root = find(&mut parent, slot);
        let next = regions.len();
        let index = *slots.entry(root).or_insert_with(|| {
            regions.push(Vec::new());
            next
        });
        regions[index].push(id);
    }
    let mut output = Mesh::new();
    for ids in regions {
        let first = &mesh.faces[ids[0]];
        let traced = if ids.len() == 1 {
            Some((first.vertices.clone(), first.holes.clone()))
        } else {
            trace(mesh, &ids, normals[ids[0]])
        };
        if let Some((outer, holes)) = traced {
            append(&mut output, mesh, &outer, &holes, first)?;
        } else {
            for id in ids {
                let f = &mesh.faces[id];
                append(&mut output, mesh, &f.vertices, &f.holes, f)?;
            }
        }
    }
    *mesh = output;
    soften(mesh, true);
    Ok(())
}
fn append(
    out: &mut Mesh,
    source: &Mesh,
    outer: &[usize],
    holes: &[Vec<usize>],
    face: &MeshFace,
) -> Result<(), String> {
    let ring = |out: &mut Mesh, r: &[usize]| {
        r.iter()
            .map(|&v| out.add_vertex(source.vertices[v].position))
            .collect::<Vec<_>>()
    };
    let vertices = ring(out, outer);
    let holes = holes.iter().map(|r| ring(out, r)).collect();
    let id = out
        .add_face(vertices, holes)
        .ok_or("Invalid imported fused face")?;
    out.faces[id].attrs = face.attrs.clone();
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
            .filter(|v| v.as_i64().is_some_and(|g| g != 0))
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
    } else {
        ingetrazo_core::heal::orient_boundary_candidate(mesh)?;
    }
    soften(mesh, false);
    Ok(())
}
