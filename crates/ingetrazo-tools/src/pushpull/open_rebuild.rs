// SPDX-License-Identifier: GPL-3.0-or-later
// Pinned core/cap_rebuild.py: open perpendicular sweep affected-plane policy.
use glam::{DVec2, DVec3, Vec3};
use ingetrazo_core::{
    mesh::{Mesh, MeshFace},
    planar::PlanarCell,
};
use std::collections::{BTreeMap, HashMap, HashSet};
fn divide(v: Vec3, d: f32) -> Vec3 {
    Vec3::new(v.x / d, v.y / d, v.z / d)
}
fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        0.
    } else {
        let x = divide(v, m);
        m * (x.x * x.x + x.y * x.y + x.z * x.z).sqrt()
    }
}
fn normalized(v: Vec3) -> Vec3 {
    let l = length(v);
    if (l - 1.).abs() < 1e-5 {
        v
    } else if l.abs() < 1e-5 {
        Vec3::ZERO
    } else {
        divide(v, l)
    }
}
fn normal(mesh: &Mesh, id: usize) -> Vec3 {
    let p = mesh.faces[id]
        .vertices
        .iter()
        .map(|&v| mesh.vertices[v].position.as_vec3().as_dvec3())
        .collect::<Vec<_>>();
    normalized(ingetrazo_core::math::newell_vector(&p).as_vec3())
}
fn basis(n: Vec3) -> (Vec3, Vec3) {
    let r = if n.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
    let u = normalized(r - n * r.dot(n));
    (u, normalized(n.cross(u)))
}
fn inside(p: DVec2, r: &[DVec2]) -> bool {
    let mut yes = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            yes = !yes;
        }
        j = i;
    }
    yes
}
fn projected(mesh: &Mesh, f: &MeshFace, o: Vec3, n: Vec3) -> Vec<Vec<DVec2>> {
    let (u, v) = basis(n);
    std::iter::once(&f.vertices)
        .chain(&f.holes)
        .map(|r| {
            r.iter()
                .map(|&id| {
                    let d = mesh.vertices[id].position.as_vec3() - o;
                    DVec2::new(d.dot(u) as f64, d.dot(v) as f64)
                })
                .collect()
        })
        .collect()
}
fn covered(p: DVec2, r: &[Vec<DVec2>]) -> bool {
    inside(p, &r[0]) && !r.iter().skip(1).any(|h| inside(p, h))
}
fn probe(c: &PlanarCell, o: Vec3, n: Vec3) -> Result<(Vec3, DVec2), String> {
    let (u, v) = basis(n);
    let project = |p: DVec3| {
        let d = p.as_vec3() - o;
        DVec2::new(d.dot(u) as f64, d.dot(v) as f64)
    };
    let outer = c.outer.iter().copied().map(project).collect::<Vec<_>>();
    let holes = c
        .holes
        .iter()
        .map(|h| h.iter().copied().map(project).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let covers = |p| inside(p, &outer) && !holes.iter().any(|h| inside(p, h));
    let mean = outer.iter().copied().sum::<DVec2>() / outer.len() as f64;
    let mut p = mean;
    if !covers(p) {
        let mut found = None;
        for i in 0..outer.len() {
            let a = outer[i];
            let b = outer[(i + 1) % outer.len()];
            let d = b - a;
            if d.length() < 1e-7 {
                continue;
            }
            let q = (a + b) * 0.5 + DVec2::new(-d.y, d.x).normalize() * 1e-3;
            if covers(q) {
                found = Some(q);
                break;
            }
        }
        p = found.ok_or("Cannot probe an open rebuild region without crossing a hole")?;
    }
    Ok((o + u * (p.x as f32) + v * (p.y as f32), p))
}
// CPython Random(int) MT19937 initialization, then its 53-bit random().
struct Random {
    state: [u32; 624],
    index: usize,
}
impl Random {
    fn new(seed: u32) -> Self {
        let mut s = Self {
            state: [0; 624],
            index: 624,
        };
        s.state[0] = 19650218;
        for i in 1..624 {
            s.state[i] = 1812433253u32
                .wrapping_mul(s.state[i - 1] ^ (s.state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        let mut i = 1;
        for _ in 0..624 {
            s.state[i] = (s.state[i]
                ^ (s.state[i - 1] ^ (s.state[i - 1] >> 30)).wrapping_mul(1664525))
            .wrapping_add(seed);
            i += 1;
            if i >= 624 {
                s.state[0] = s.state[623];
                i = 1;
            }
        }
        for _ in 0..623 {
            s.state[i] = (s.state[i]
                ^ (s.state[i - 1] ^ (s.state[i - 1] >> 30)).wrapping_mul(1566083941))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                s.state[0] = s.state[623];
                i = 1;
            }
        }
        s.state[0] = 0x80000000;
        s
    }
    fn word(&mut self) -> u32 {
        if self.index == 624 {
            for i in 0..624 {
                let y = (self.state[i] & 0x80000000) | (self.state[(i + 1) % 624] & 0x7fffffff);
                self.state[i] = self.state[(i + 397) % 624]
                    ^ (y >> 1)
                    ^ if y & 1 != 0 { 0x9908b0df } else { 0 };
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^ (y >> 18)
    }
    fn uniform(&mut self) -> f32 {
        let a = self.word() >> 5;
        let b = self.word() >> 6;
        let x = ((a as f64) * 67108864. + b as f64) / 9007199254740992.;
        (-3e-4 + 6e-4 * x) as f32
    }
}
fn material(o: Vec3, n: Vec3, tris: &[(DVec3, DVec3, DVec3)], rng: &mut Random) -> bool {
    let reference = if n.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
    let u = normalized(n.cross(reference));
    let v = normalized(n.cross(u));
    let mut votes = 0;
    for r in 0..7 {
        let d = if r == 0 {
            n
        } else {
            normalized(n + u * rng.uniform() + v * rng.uniform())
        }
        .as_dvec3();
        let o = o.as_dvec3();
        let count = tris
            .iter()
            .filter(|&&(a, b, c)| {
                let e1 = b - a;
                let e2 = c - a;
                let p = d.cross(e2);
                let det = e1.dot(p);
                if det.abs() < 1e-7 {
                    return false;
                }
                let inv = 1. / det;
                let t = o - a;
                let u = t.dot(p) * inv;
                if !(-1e-7..=1. + 1e-7).contains(&u) {
                    return false;
                }
                let q = t.cross(e1);
                let v = d.dot(q) * inv;
                if v < -1e-7 || u + v > 1. + 1e-7 {
                    return false;
                }
                e2.dot(q) * inv > 1e-6
            })
            .count();
        if count % 2 == 1 {
            votes += 1;
        }
    }
    votes > 3
}
fn canonical(mesh: &Mesh) -> Vec<(Vec<[i64; 3]>, Vec<Vec<[i64; 3]>>, bool)> {
    fn ring(mesh: &Mesh, r: &[usize]) -> Vec<[i64; 3]> {
        let mut a = r
            .iter()
            .map(|&v| {
                mesh.vertices[v]
                    .position
                    .to_array()
                    .map(|x| (x / 1e-4).round_ties_even() as i64)
            })
            .collect::<Vec<_>>();
        if let Some(i) = (0..a.len()).min_by_key(|&i| a[i]) {
            a.rotate_left(i);
        }
        a
    }
    let mut out = mesh
        .faces
        .iter()
        .map(|f| {
            let mut hs = f.holes.iter().map(|h| ring(mesh, h)).collect::<Vec<_>>();
            hs.sort();
            (ring(mesh, &f.vertices), hs, f.interior)
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}
fn add(out: &mut Mesh, source: &Mesh, f: &MeshFace) -> Result<(), String> {
    let outer = f
        .vertices
        .iter()
        .map(|&v| out.add_vertex(source.vertices[v].position))
        .collect();
    let holes = f
        .holes
        .iter()
        .map(|h| {
            h.iter()
                .map(|&v| out.add_vertex(source.vertices[v].position))
                .collect()
        })
        .collect();
    let id = out
        .add_face(outer, holes)
        .ok_or("Invalid open rebuilt face")?;
    let vertices = out.faces[id].vertices.clone();
    let holes = out.faces[id].holes.clone();
    out.faces[id] = f.clone();
    out.faces[id].vertices = vertices;
    out.faces[id].holes = holes;
    Ok(())
}
fn on(p: DVec3, a: DVec3, b: DVec3) -> bool {
    let d = b - a;
    let l = d.length_squared();
    if l < 1e-18 {
        return false;
    }
    let t = (p - a).dot(d) / l;
    t >= -5e-5 && t <= 1. + 5e-5 && (p - a - d * t).length() < 5e-5
}
// Union only adjacent cells in the same side class. Creases and user seams
// stay separate; ambiguous/point-connected outlines refuse atomically.
fn union(cells: &Mesh, kinds: &[u8], keep: &[(DVec3, DVec3)]) -> Result<Mesh, String> {
    let mut parents = (0..cells.faces.len()).collect::<Vec<_>>();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for e in &cells.edges {
        if e.faces.len() != 2 {
            continue;
        }
        let (a, b) = (e.faces[0], e.faces[1]);
        if kinds[a] != kinds[b] {
            continue;
        }
        let x = cells.vertices[e.v0].position;
        let y = cells.vertices[e.v1].position;
        if keep.iter().any(|&(p, q)| on(x, p, q) && on(y, p, q)) {
            continue;
        }
        let (a, b) = (root(&mut parents, a), root(&mut parents, b));
        parents[b] = a;
    }
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..parents.len() {
        let r = root(&mut parents, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out = Mesh::new();
    for ids in groups.values() {
        let mut edges = HashMap::new();
        for &id in ids {
            for ring in std::iter::once(&cells.faces[id].vertices).chain(&cells.faces[id].holes) {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    if edges.remove(&(b, a)).is_none() {
                        edges.insert((a, b), ());
                    }
                }
            }
        }
        let mut next = BTreeMap::new();
        for &(a, b) in edges.keys() {
            if next.insert(a, b).is_some() {
                return Err("Open sweep union has an ambiguous branching boundary".into());
            }
        }
        let mut loops = vec![];
        while let Some((&start, _)) = next.iter().next() {
            let mut ring = vec![start];
            let mut v = start;
            loop {
                v = next
                    .remove(&v)
                    .ok_or("Open sweep union boundary does not close")?;
                if v == start {
                    break;
                }
                ring.push(v);
                if ring.len() > edges.len() {
                    return Err("Open sweep union exceeds its boundary".into());
                }
            }
            loops.push(ring);
        }
        let n = normal(cells, ids[0]);
        let mut outer = vec![];
        let mut holes = vec![];
        for ring in loops {
            let pts = ring
                .iter()
                .map(|&v| cells.vertices[v].position)
                .collect::<Vec<_>>();
            if ingetrazo_core::math::newell_vector(&pts).dot(n.as_dvec3()) > 0. {
                outer.push(ring)
            } else {
                holes.push(ring)
            }
        }
        if outer.len() != 1 {
            return Err("Open sweep union has disconnected or pinched regions".into());
        }
        let mut f = MeshFace::new(outer.remove(0));
        f.holes = holes;
        f.interior = kinds[ids[0]] == 2;
        add(&mut out, cells, &f)?;
    }
    Ok(out)
}
pub(super) fn rebuild(
    mesh: &Mesh,
    original: &Mesh,
    component: &HashSet<usize>,
    removing: bool,
) -> Result<Mesh, String> {
    let original_ids = original
        .faces
        .iter()
        .map(|f| f.history_id)
        .collect::<HashSet<_>>();
    let connected_ids = component
        .iter()
        .map(|&i| original.faces[i].history_id)
        .collect::<HashSet<_>>();
    let mut domain = Mesh::new();
    let mut parked = Mesh::new();
    for f in &mesh.faces {
        if connected_ids.contains(&f.history_id) || !original_ids.contains(&f.history_id) {
            add(&mut domain, mesh, f)?;
        } else {
            add(&mut parked, mesh, f)?;
        }
    }
    let mut fresh = domain
        .faces
        .iter()
        .filter(|f| !original_ids.contains(&f.history_id))
        .map(|f| f.history_id)
        .collect::<HashSet<_>>();
    let mut rims = domain
        .faces
        .iter()
        .filter(|f| fresh.contains(&f.history_id))
        .flat_map(|f| {
            std::iter::once(&f.vertices).chain(&f.holes).flat_map(|r| {
                (0..r.len())
                    .map(|i| {
                        (
                            domain.vertices[r[i]].position,
                            domain.vertices[r[(i + 1) % r.len()]].position,
                        )
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    for _ in 0..4 {
        let normals = (0..domain.faces.len())
            .map(|i| normal(&domain, i))
            .collect::<Vec<_>>();
        let mut affected = HashSet::new();
        for e in &domain.edges {
            if e.faces.len() == 1 {
                affected.insert(e.faces[0]);
            }
            for &i in &e.faces {
                if fresh.contains(&domain.faces[i].history_id)
                    && e.faces
                        .iter()
                        .any(|&g| g != i && normals[g].dot(normals[i]).abs() > 0.999)
                {
                    affected.insert(i);
                }
            }
        }
        let mut planes = BTreeMap::new();
        for i in 0..domain.faces.len() {
            if !affected.contains(&i) {
                continue;
            }
            let f = &domain.faces[i];
            let mut n = normals[i];
            let axis = if n.x.abs() >= n.y.abs() && n.x.abs() >= n.z.abs() {
                0
            } else if n.y.abs() >= n.z.abs() {
                1
            } else {
                2
            };
            if n[axis] < 0. {
                n = -n;
            }
            let o = (f
                .vertices
                .iter()
                .map(|&v| domain.vertices[v].position.as_vec3().as_dvec3())
                .sum::<DVec3>()
                / f.vertices.len() as f64)
                .as_vec3();
            let d = o.dot(n);
            let key = [n.x, n.y, n.z, d].map(|v| ((v as f64) * 1000.).round_ties_even() as i64);
            planes.entry(key).or_insert((o, n));
        }
        let mut changed = false;
        for (o, n) in planes.values().copied() {
            let old = (0..domain.faces.len())
                .filter(|&i| {
                    normal(&domain, i).dot(n).abs() > 0.999
                        && (domain.vertices[domain.faces[i].vertices[0]]
                            .position
                            .as_vec3()
                            - o)
                            .dot(n)
                            .abs()
                            < 1e-4
                })
                .collect::<Vec<_>>();
            if old.is_empty() {
                continue;
            }
            let regions =
                ingetrazo_core::planar::sweep_plane_cells(&domain, o.as_dvec3(), n.as_dvec3())?;
            if regions.is_empty() {
                continue;
            }
            let tris = (0..domain.faces.len())
                .filter(|&i| !domain.faces[i].interior)
                .flat_map(|i| domain.triangulate_face(i))
                .map(|(a, b, c)| {
                    (
                        a.as_vec3().as_dvec3(),
                        b.as_vec3().as_dvec3(),
                        c.as_vec3().as_dvec3(),
                    )
                })
                .collect::<Vec<_>>();
            let projected = old
                .iter()
                .map(|&i| (i, projected(&domain, &domain.faces[i], o, n)))
                .collect::<Vec<_>>();
            let mut rng = Random::new(54321);
            let mut cells = Mesh::new();
            let mut kinds = vec![];
            for mut c in regions {
                let (ip, p) = probe(&c, o, n)?;
                let plus = material(ip + n * 1e-3, n, &tris, &mut rng);
                let minus = material(ip - n * 1e-3, -n, &tris, &mut rng);
                let decl = projected
                    .iter()
                    .filter(|(i, r)| {
                        fresh.contains(&domain.faces[*i].history_id)
                            && !domain.faces[*i].interior
                            && covered(p, r)
                    })
                    .map(|(i, _)| normal(&domain, *i).dot(n) > 0.)
                    .collect::<HashSet<_>>();
                let oldcover = projected.iter().any(|(i, r)| {
                    (!fresh.contains(&domain.faces[*i].history_id) || domain.faces[*i].interior)
                        && covered(p, r)
                });
                let opposite_old = projected.iter().any(|(i, r)| {
                    !fresh.contains(&domain.faces[*i].history_id)
                        && !domain.faces[*i].interior
                        && covered(p, r)
                        && decl.contains(&(normal(&domain, *i).dot(n) <= 0.))
                });
                let kind = if plus != minus {
                    Some(if plus { 0 } else { 1 })
                } else if plus {
                    if removing && decl.len() == 1 {
                        Some(if !*decl.iter().next().unwrap() { 0 } else { 1 })
                    } else if !removing && opposite_old {
                        for ring in std::iter::once(&c.outer).chain(&c.holes) {
                            for i in 0..ring.len() {
                                rims.push((ring[i], ring[(i + 1) % ring.len()]));
                            }
                        }
                        None
                    } else if (!removing || decl.is_empty()) && oldcover {
                        Some(2)
                    } else {
                        None
                    }
                } else if !removing && decl.len() == 1 {
                    Some(if !*decl.iter().next().unwrap() { 0 } else { 1 })
                } else if decl.is_empty() && oldcover {
                    Some(3)
                } else {
                    None
                };
                let Some(kind) = kind else {
                    continue;
                };
                if kind == 0 {
                    c.outer.reverse();
                    for h in &mut c.holes {
                        h.reverse();
                    }
                }
                let ids = c.outer.iter().map(|&p| cells.add_vertex(p)).collect();
                let hs = c
                    .holes
                    .iter()
                    .map(|h| h.iter().map(|&p| cells.add_vertex(p)).collect())
                    .collect();
                let id = cells
                    .add_face(ids, hs)
                    .ok_or("Open sweep arrangement region collapsed")?;
                cells.faces[id].interior = kind == 2;
                kinds.push(kind);
            }
            let keep = domain
                .edges
                .iter()
                .filter_map(|e| {
                    let a = domain.vertices[e.v0].position;
                    let b = domain.vertices[e.v1].position;
                    if (a.as_vec3() - o).dot(n).abs() >= 1e-4
                        || (b.as_vec3() - o).dot(n).abs() >= 1e-4
                    {
                        return None;
                    }
                    let crease = e
                        .faces
                        .iter()
                        .any(|&f| normal(&domain, f).dot(n).abs() < 0.999);
                    let authored = !e.faces.is_empty()
                        && !rims.iter().any(|&(x, y)| on(a, x, y) && on(b, x, y));
                    (crease || authored).then_some((a, b))
                })
                .collect::<Vec<_>>();
            let mut rebuilt = union(&cells, &kinds, &keep)?;
            let mut oldmesh = Mesh::new();
            for &i in &old {
                add(&mut oldmesh, &domain, &domain.faces[i])?;
            }
            if canonical(&oldmesh) == canonical(&rebuilt) {
                continue;
            }
            for id in 0..rebuilt.faces.len() {
                let cell = PlanarCell {
                    outer: rebuilt.faces[id]
                        .vertices
                        .iter()
                        .map(|&v| rebuilt.vertices[v].position)
                        .collect(),
                    holes: rebuilt.faces[id]
                        .holes
                        .iter()
                        .map(|h| h.iter().map(|&v| rebuilt.vertices[v].position).collect())
                        .collect(),
                };
                let (_, p) = probe(&cell, o, n)?;
                if let Some((i, _)) = projected
                    .iter()
                    .find(|(i, r)| !domain.faces[*i].attrs.is_empty() && covered(p, r))
                {
                    rebuilt.faces[id].attrs = domain.faces[*i].attrs.clone();
                    rebuilt.faces[id].layer = domain.faces[*i].layer.clone();
                }
                fresh.insert(rebuilt.faces[id].history_id);
            }
            let removed = old.iter().copied().collect::<HashSet<_>>();
            let mut next = Mesh::new();
            for (i, f) in domain.faces.iter().enumerate() {
                if !removed.contains(&i) {
                    add(&mut next, &domain, f)?;
                }
            }
            for f in &rebuilt.faces {
                add(&mut next, &rebuilt, f)?;
            }
            domain = next;
            changed = true;
        }
        if !changed {
            break;
        }
    }
    if domain.faces.is_empty() {
        return Err("Open sweep rebuild would remove its entire affected component".into());
    }
    for f in &domain.faces {
        add(&mut parked, &domain, f)?;
    }
    Ok(parked)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_seed_54321_stream() {
        let mut rng = Random::new(54321);
        let expected = [0.48866987066258627, 0.9882259597584993, 0.24456828827706578];
        for x in expected {
            let actual = rng.uniform();
            assert_eq!(actual, (-3e-4 + 6e-4 * x) as f32);
        }
    }
}
