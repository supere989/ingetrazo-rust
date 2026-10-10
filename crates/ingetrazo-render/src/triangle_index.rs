// SPDX-License-Identifier: GPL-3.0-or-later
//! Conservative projected-triangle broad phase; final source predicates stay authoritative.
use glam::{DVec2, DVec3};
#[derive(Clone, Copy)]
struct Bounds {
    lo: DVec2,
    hi: DVec2,
}
impl Bounds {
    fn triangle(t: [DVec3; 3]) -> Self {
        let lo = t
            .iter()
            .fold(DVec2::splat(f64::INFINITY), |v, p| v.min(p.truncate()));
        let hi = t
            .iter()
            .fold(DVec2::splat(f64::NEG_INFINITY), |v, p| v.max(p.truncate()));
        let extent = hi - lo;
        let a = (t[1] - t[0]).truncate();
        let b = (t[2] - t[0]).truncate();
        let area = (a.x * b.y - a.y * b.x).abs();
        // Ill-conditioned triangles retain the exhaustive source behavior.
        if !lo.is_finite() || !hi.is_finite() || area < extent.x * extent.y * 1e-12 {
            return Self {
                lo: DVec2::splat(f64::NEG_INFINITY),
                hi: DVec2::splat(f64::INFINITY),
            };
        }
        // Source point inclusion allows barycentric weights down to -1e-9.
        let pad = extent.max_element() * 4e-9
            + lo.abs().max(hi.abs()).max_element() * f64::EPSILON * 64.
            + 1e-12;
        Self {
            lo: lo - DVec2::splat(pad),
            hi: hi + DVec2::splat(pad),
        }
    }
    fn overlap(self, lo: DVec2, hi: DVec2) -> bool {
        self.lo.x <= hi.x && self.hi.x >= lo.x && self.lo.y <= hi.y && self.hi.y >= lo.y
    }
    fn union(self, b: Self) -> Self {
        Self {
            lo: self.lo.min(b.lo),
            hi: self.hi.max(b.hi),
        }
    }
}
struct Node {
    bounds: Bounds,
    begin: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
/// Stores only triangle IDs/bounds; geometry remains in the caller's immutable mapping.
pub(crate) struct TriangleIndex {
    boxes: Vec<Bounds>,
    ids: Vec<usize>,
    nodes: Vec<Node>,
}
impl TriangleIndex {
    pub(crate) fn new(triangles: impl IntoIterator<Item = [DVec3; 3]>) -> Self {
        let boxes = triangles
            .into_iter()
            .map(Bounds::triangle)
            .collect::<Vec<_>>();
        let mut index = Self {
            ids: (0..boxes.len()).collect(),
            boxes,
            nodes: vec![],
        };
        if !index.ids.is_empty() {
            index.build(0, index.ids.len());
        }
        index
    }
    fn build(&mut self, begin: usize, end: usize) -> usize {
        let bounds = self.ids[begin..end]
            .iter()
            .map(|&id| self.boxes[id])
            .reduce(Bounds::union)
            .unwrap();
        let node = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            begin,
            end,
            children: None,
        });
        if end - begin > 8 {
            let axis = usize::from((bounds.hi - bounds.lo).y > (bounds.hi - bounds.lo).x);
            let boxes = &self.boxes;
            self.ids[begin..end].sort_unstable_by(|&a, &b| {
                let center = |id: usize| (boxes[id].lo[axis] + boxes[id].hi[axis]) * 0.5;
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let mid = begin + (end - begin) / 2;
            let a = self.build(begin, mid);
            let b = self.build(mid, end);
            self.nodes[node].children = Some((a, b));
        }
        node
    }
    pub(crate) fn query(&self, lo: DVec2, hi: DVec2, out: &mut Vec<usize>) {
        out.clear();
        if !self.nodes.is_empty() {
            self.visit(0, lo, hi, out);
        }
        out.sort_unstable();
    }
    fn visit(&self, node: usize, lo: DVec2, hi: DVec2, out: &mut Vec<usize>) {
        let n = &self.nodes[node];
        if !n.bounds.overlap(lo, hi) {
            return;
        }
        if let Some((a, b)) = n.children {
            self.visit(a, lo, hi, out);
            self.visit(b, lo, hi, out);
        } else {
            for &id in &self.ids[n.begin..n.end] {
                if self.boxes[id].overlap(lo, hi) {
                    out.push(id);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_candidates_keep_source_order_and_barycentric_margin() {
        let triangles = (0..1000)
            .map(|i| {
                let x = i as f64 * 10.;
                [
                    DVec3::new(x, 0., 0.),
                    DVec3::new(x + 1., 0., 0.),
                    DVec3::new(x, 1., 0.),
                ]
            })
            .collect::<Vec<_>>();
        let index = TriangleIndex::new(triangles);
        let mut ids = vec![];
        index.query(DVec2::new(100., -1e-10), DVec2::new(101., 0.), &mut ids);
        assert_eq!(ids, vec![10]);
        index.query(DVec2::new(-1., -1.), DVec2::new(10000., 2.), &mut ids);
        assert_eq!(ids, (0..1000).collect::<Vec<_>>());
    }
    #[test]
    fn near_degenerate_triangle_is_never_culled() {
        let index = TriangleIndex::new([[
            DVec3::ZERO,
            DVec3::new(1., 1., 0.),
            DVec3::new(2., 2. + 1e-14, 0.),
        ]]);
        let mut ids = vec![];
        index.query(DVec2::splat(100.), DVec2::splat(101.), &mut ids);
        assert_eq!(ids, vec![0]);
    }

    #[test]
    fn indexed_occlusion_is_bit_exact_against_exhaustive_translated_scales() {
        let mut seed = 73u64;
        let mut number = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 11) as f64 / (1u64 << 53) as f64 * 2. - 1.
        };
        for scale in [1e-4, 1., 1e4] {
            let offset = DVec3::new(1e6, -1e6, 0.);
            let triangles = (0..256)
                .map(|_| [0; 3].map(|_| offset + DVec3::new(number(), number(), number()) * scale))
                .collect::<Vec<_>>();
            let index = TriangleIndex::new(triangles.iter().copied());
            let mut ids = vec![];
            for _ in 0..128 {
                let a = offset + DVec3::new(number(), number(), number()) * scale;
                let b = offset + DVec3::new(number(), number(), number()) * scale;
                let epsilon = scale * 1e-4;
                let exact = triangles
                    .iter()
                    .filter_map(|&t| crate::vector_hlr::occluded_interval(a, b, t, epsilon))
                    .collect::<Vec<_>>();
                index.query(
                    a.truncate().min(b.truncate()),
                    a.truncate().max(b.truncate()),
                    &mut ids,
                );
                let indexed = ids
                    .iter()
                    .filter_map(|&id| {
                        crate::vector_hlr::occluded_interval(a, b, triangles[id], epsilon)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(exact, indexed);
            }
        }
    }
}
