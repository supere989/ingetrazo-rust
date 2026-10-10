// SPDX-License-Identifier: GPL-3.0-or-later
//! Pinned source core.hlr clip_to_section / section_loops for paper only.
use glam::DVec3;
use ingetrazo_core::section::SectionPlane;
use std::collections::HashMap;

pub fn clip_triangles(
    triangles: Vec<[DVec3; 3]>,
    plane: &SectionPlane,
) -> (Vec<[DVec3; 3]>, Vec<(DVec3, DVec3)>) {
    let mut kept = Vec::new();
    let mut cuts = Vec::new();
    let n = plane.normal.as_vec3().as_dvec3();
    let p = plane.point.as_vec3().as_dvec3();
    let c = n.x * p.x + n.y * p.y + n.z * p.z;
    for triangle in triangles {
        let d = triangle.map(|p| p.x * n.x + p.y * n.y + p.z * n.z - c);
        let count = d.iter().filter(|&&v| v > 1e-9).count();
        if count == 0 {
            kept.push(triangle);
            continue;
        }
        if count == 3 {
            continue;
        }
        let mut polygon = Vec::with_capacity(4);
        let mut crossings = Vec::with_capacity(2);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if d[i] <= 1e-9 {
                polygon.push(triangle[i]);
            }
            if (d[i] > 1e-9) != (d[j] > 1e-9) {
                let t = d[i] / (d[i] - d[j]);
                let x = triangle[i] + (triangle[j] - triangle[i]) * t;
                polygon.push(x);
                crossings.push(x);
            }
        }
        if crossings.len() == 2 {
            cuts.push((crossings[0], crossings[1]));
        }
        for i in 1..polygon.len() - 1 {
            kept.push([polygon[0], polygon[i], polygon[i + 1]]);
        }
    }
    (kept, cuts)
}
// Python's floor cells can exceed i64. Such f64-derived integer cells have
// spacing >1, so only the zero-neighbour can match another floored f64 input.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Cell {
    Small(i64),
    Large(u64),
}
impl Cell {
    fn of(x: f64) -> Self {
        let f = x.floor();
        if f >= i64::MIN as f64 && f < (i64::MAX as f64) {
            Self::Small(f as i64)
        } else {
            Self::Large(f.to_bits())
        }
    }
    fn neighbour(self, delta: i64) -> Option<Self> {
        match self {
            Self::Small(x) => x.checked_add(delta).map(Self::Small),
            Self::Large(_) => {
                if delta == 0 {
                    Some(self)
                } else {
                    None
                }
            }
        }
    }
}
pub fn closed_loops(cuts: &[(DVec3, DVec3)]) -> Vec<Vec<DVec3>> {
    if cuts.is_empty() {
        return Vec::new();
    }
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for &(a, b) in cuts {
        lo = lo.min(a).min(b);
        hi = hi.max(a).max(b);
    }
    let tol = ((hi - lo).max_element() * 1e-7).max(1e-9);
    let inv = 1. / tol;
    let mut cells: HashMap<[Cell; 3], Vec<usize>> = HashMap::new();
    let mut nodes: Vec<DVec3> = Vec::new();
    let mut node_of = |p: DVec3| {
        let cell = p.to_array().map(|x| Cell::of(x * inv));
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let (Some(x), Some(y), Some(z)) = (
                        cell[0].neighbour(dx),
                        cell[1].neighbour(dy),
                        cell[2].neighbour(dz),
                    ) {
                        if let Some(ids) = cells.get(&[x, y, z]) {
                            for &id in ids {
                                if (nodes[id] - p).abs().max_element() <= tol {
                                    return id;
                                }
                            }
                        }
                    }
                }
            }
        }
        let id = nodes.len();
        nodes.push(p);
        cells.entry(cell).or_default().push(id);
        id
    };
    let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut chords = Vec::new();
    for &(a, b) in cuts {
        let a = node_of(a);
        let b = node_of(b);
        if a == b {
            continue;
        }
        let id = chords.len();
        chords.push((a, b));
        adjacency.entry(a).or_default().push(id);
        adjacency.entry(b).or_default().push(id);
    }
    let mut used = vec![false; chords.len()];
    let mut loops = Vec::new();
    for start in 0..chords.len() {
        if used[start] {
            continue;
        }
        let (a, b) = chords[start];
        used[start] = true;
        let mut ring = vec![a, b];
        let mut current = b;
        let mut previous = start;
        let mut closed = false;
        loop {
            let next = adjacency
                .get(&current)
                .and_then(|ids| ids.iter().copied().find(|&id| id != previous && !used[id]));
            let Some(next) = next else {
                break;
            };
            used[next] = true;
            let (x, y) = chords[next];
            current = if x == current { y } else { x };
            previous = next;
            if current == a {
                closed = true;
                break;
            }
            ring.push(current);
        }
        if closed && ring.len() >= 3 {
            loops.push(ring.into_iter().map(|i| nodes[i]).collect());
        }
    }
    loops
}

/// Qt float-vector normalization boundary used by source plane_axes. Qt's
/// qHypot float overload scales by its largest component; division remains f32.
fn qt_length(v: glam::Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let v = v / m;
    m * ((v.x * v.x + v.y * v.y) + v.z * v.z).sqrt()
}
fn qt_normalized(v: glam::Vec3) -> glam::Vec3 {
    let len = qt_length(v);
    if (len - 1.).abs() <= 0.00001 {
        v
    } else if len.abs() <= 0.00001 {
        glam::Vec3::ZERO
    } else {
        glam::Vec3::new(v.x / len, v.y / len, v.z / len)
    }
}
pub fn plane_axes(normal: DVec3) -> (DVec3, DVec3) {
    let n = qt_normalized(normal.as_vec3());
    let mut u = glam::Vec3::X - n * n.x;
    if qt_length(u) < 0.1 {
        u = glam::Vec3::Y - n * n.y;
    }
    let u = qt_normalized(u);
    let v = qt_normalized(n.cross(u));
    (u.as_dvec3(), v.as_dvec3())
}
pub fn depth_triangle(plane: &SectionPlane) -> [DVec3; 3] {
    let (u, v) = plane_axes(plane.normal);
    let p = plane.point.as_vec3().as_dvec3();
    [p, p + u, p + v]
}
