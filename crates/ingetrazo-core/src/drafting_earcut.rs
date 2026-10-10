// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Original Mapbox-style earcut ordering used for authoring coverage probes.
//! Port of ingetrazo/core/triangulate.py (GPL-3.0-or-later). The existing
//! earcutr remains the normal mesh triangulator. Touching transient hole loops
//! in AddFaceCommand depend on the source bridge/filter traversal ordering.
use glam::DVec2;
#[derive(Clone, Copy)]
struct Node {
    index: usize,
    p: DVec2,
    prev: usize,
    next: usize,
    steiner: bool,
}
struct Cut {
    nodes: Vec<Node>,
    triangles: Vec<[usize; 3]>,
    exhausted: bool,
}
impl Cut {
    fn insert(&mut self, index: usize, p: DVec2, last: Option<usize>) -> usize {
        let id = self.nodes.len();
        let (prev, next) = last.map(|v| (v, self.nodes[v].next)).unwrap_or((id, id));
        self.nodes.push(Node {
            index,
            p,
            prev,
            next,
            steiner: false,
        });
        self.nodes[prev].next = id;
        self.nodes[next].prev = id;
        id
    }
    fn remove(&mut self, p: usize) {
        let n = self.nodes[p];
        self.nodes[n.next].prev = n.prev;
        self.nodes[n.prev].next = n.next;
    }
    fn equal(&self, a: usize, b: usize) -> bool {
        self.nodes[a].p == self.nodes[b].p
    }
    fn area(&self, a: usize, b: usize, c: usize) -> f64 {
        let a = self.nodes[a].p;
        let b = self.nodes[b].p;
        let c = self.nodes[c].p;
        (b.y - a.y) * (c.x - b.x) - (b.x - a.x) * (c.y - b.y)
    }
    fn ring(
        &mut self,
        points: &[DVec2],
        range: std::ops::Range<usize>,
        ccw: bool,
    ) -> Option<usize> {
        let ids: Vec<_> = range.collect();
        if ids.is_empty() {
            return None;
        }
        let area = (0..ids.len())
            .map(|i| {
                let a = points[ids[i]];
                let b = points[ids[(i + 1) % ids.len()]];
                (b.x - a.x) * (a.y + b.y)
            })
            .sum::<f64>();
        let mut sequence = ids;
        if (area > 0.) == ccw {
            sequence.reverse();
        }
        let mut last = None;
        for i in sequence {
            last = Some(self.insert(i, points[i], last));
        }
        if let Some(p) = last {
            if self.equal(p, self.nodes[p].next) {
                let next = self.nodes[p].next;
                self.remove(p);
                last = Some(next);
            }
        }
        last
    }
    fn filter(&mut self, start: usize, end: Option<usize>) -> usize {
        let mut end = end.unwrap_or(start);
        let mut p = start;
        loop {
            let mut again = false;
            let n = self.nodes[p];
            if !n.steiner && (self.equal(p, n.next) || self.area(n.prev, p, n.next) == 0.) {
                self.remove(p);
                p = n.prev;
                end = p;
                if p == self.nodes[p].next {
                    break;
                }
                again = true;
            } else {
                p = n.next;
            }
            if !again && p == end {
                break;
            }
        }
        end
    }
    fn in_triangle(a: DVec2, b: DVec2, c: DVec2, p: DVec2) -> bool {
        (c - p).perp_dot(a - p) >= 0.
            && (a - p).perp_dot(b - p) >= 0.
            && (b - p).perp_dot(c - p) >= 0.
    }
    fn ear(&self, b: usize) -> bool {
        let a = self.nodes[b].prev;
        let c = self.nodes[b].next;
        if self.area(a, b, c) >= 0. {
            return false;
        }
        let ap = self.nodes[a].p;
        let bp = self.nodes[b].p;
        let cp = self.nodes[c].p;
        let min = ap.min(bp).min(cp);
        let max = ap.max(bp).max(cp);
        let mut p = self.nodes[c].next;
        while p != a {
            let n = self.nodes[p];
            if n.p.x >= min.x
                && n.p.y >= min.y
                && n.p.x <= max.x
                && n.p.y <= max.y
                && Self::in_triangle(ap, bp, cp, n.p)
                && self.area(n.prev, p, n.next) >= 0.
            {
                return false;
            }
            p = n.next;
        }
        true
    }
    fn local(&self, a: usize, b: usize) -> bool {
        let n = self.nodes[a];
        if self.area(n.prev, a, n.next) < 0. {
            self.area(a, b, n.next) >= 0. && self.area(a, n.prev, b) >= 0.
        } else {
            self.area(a, b, n.prev) < 0. || self.area(a, n.next, b) < 0.
        }
    }
    fn split(&mut self, a: usize, b: usize) -> usize {
        let na = self.nodes[a];
        let nb = self.nodes[b];
        let a2 = self.nodes.len();
        let b2 = a2 + 1;
        self.nodes.push(Node {
            next: na.next,
            prev: b2,
            ..na
        });
        self.nodes.push(Node {
            next: a2,
            prev: nb.prev,
            ..nb
        });
        self.nodes[a].next = b;
        self.nodes[b].prev = a;
        self.nodes[na.next].prev = a2;
        self.nodes[nb.prev].next = b2;
        b2
    }
    fn leftmost(&self, start: usize) -> usize {
        let mut p = start;
        let mut left = start;
        loop {
            let x = self.nodes[p].p;
            let y = self.nodes[left].p;
            if x.x < y.x || (x.x == y.x && x.y < y.y) {
                left = p;
            }
            p = self.nodes[p].next;
            if p == start {
                break;
            }
        }
        left
    }
    fn bridge(&self, hole: usize, outer: usize) -> Option<usize> {
        let h = self.nodes[hole].p;
        let mut qx = f64::NEG_INFINITY;
        let mut m = None;
        let mut p = outer;
        loop {
            let a = self.nodes[p].p;
            let ni = self.nodes[p].next;
            let b = self.nodes[ni].p;
            if h.y <= a.y && h.y >= b.y && b.y != a.y {
                let x = a.x + (h.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if h.x >= x && x > qx {
                    qx = x;
                    m = Some(if a.x < b.x { p } else { ni });
                    if x == h.x {
                        return m;
                    }
                }
            }
            p = ni;
            if p == outer {
                break;
            }
        }
        let mut m = m?;
        let stop = m;
        let mp = self.nodes[m].p;
        let mut tan_min = f64::INFINITY;
        p = m;
        loop {
            let np = self.nodes[p].p;
            if h.x >= np.x
                && np.x >= mp.x
                && h.x != np.x
                && Self::in_triangle(
                    DVec2::new(if h.y < mp.y { h.x } else { qx }, h.y),
                    mp,
                    DVec2::new(if h.y < mp.y { qx } else { h.x }, h.y),
                    np,
                )
            {
                let tan = (h.y - np.y).abs() / (h.x - np.x);
                let sector = self.area(self.nodes[m].prev, m, self.nodes[p].prev) < 0.
                    && self.area(self.nodes[p].next, m, self.nodes[m].next) < 0.;
                if self.local(p, hole)
                    && (tan < tan_min
                        || (tan == tan_min
                            && (np.x > self.nodes[m].p.x || (np.x == self.nodes[m].p.x && sector))))
                {
                    m = p;
                    tan_min = tan;
                }
            }
            p = self.nodes[p].next;
            if p == stop {
                break;
            }
        }
        Some(m)
    }
    fn holes(&mut self, points: &[DVec2], starts: &[usize], mut outer: usize) -> usize {
        let mut queue = vec![];
        for (i, &start) in starts.iter().enumerate() {
            if let Some(h) = self.ring(
                points,
                start..starts.get(i + 1).copied().unwrap_or(points.len()),
                false,
            ) {
                if self.nodes[h].next == h {
                    self.nodes[h].steiner = true;
                }
                queue.push(self.leftmost(h));
            }
        }
        queue.sort_by(|&a, &b| {
            let a = self.nodes[a].p;
            let b = self.nodes[b].p;
            a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
        });
        for h in queue {
            if let Some(b) = self.bridge(h, outer) {
                let rev = self.split(b, h);
                self.filter(rev, Some(self.nodes[rev].next));
                outer = self.filter(b, Some(self.nodes[b].next));
            }
        }
        outer
    }
    fn on(a: DVec2, b: DVec2, c: DVec2) -> bool {
        b.x >= a.x.min(c.x) && b.x <= a.x.max(c.x) && b.y >= a.y.min(c.y) && b.y <= a.y.max(c.y)
    }
    fn intersects(&self, a: usize, b: usize, c: usize, d: usize) -> bool {
        let sign = |x: f64| {
            if x > 0. {
                1
            } else if x < 0. {
                -1
            } else {
                0
            }
        };
        let o = [
            sign(self.area(a, b, c)),
            sign(self.area(a, b, d)),
            sign(self.area(c, d, a)),
            sign(self.area(c, d, b)),
        ];
        (o[0] != o[1] && o[2] != o[3])
            || (o[0] == 0 && Self::on(self.nodes[a].p, self.nodes[c].p, self.nodes[b].p))
            || (o[1] == 0 && Self::on(self.nodes[a].p, self.nodes[d].p, self.nodes[b].p))
            || (o[2] == 0 && Self::on(self.nodes[c].p, self.nodes[a].p, self.nodes[d].p))
            || (o[3] == 0 && Self::on(self.nodes[c].p, self.nodes[b].p, self.nodes[d].p))
    }
    fn middle(&self, a: usize, b: usize) -> bool {
        let p = (self.nodes[a].p + self.nodes[b].p) * 0.5;
        let mut i = a;
        let mut inside = false;
        loop {
            let n = self.nodes[i];
            let x = n.p;
            let y = self.nodes[n.next].p;
            if (x.y > p.y) != (y.y > p.y)
                && y.y != x.y
                && p.x < (y.x - x.x) * (p.y - x.y) / (y.y - x.y) + x.x
            {
                inside = !inside;
            }
            i = n.next;
            if i == a {
                break;
            }
        }
        inside
    }
    fn diagonal(&self, a: usize, b: usize) -> bool {
        let na = self.nodes[a];
        let nb = self.nodes[b];
        if self.nodes[na.next].index == nb.index || self.nodes[na.prev].index == nb.index {
            return false;
        }
        let mut p = a;
        loop {
            let n = self.nodes[p];
            let next = self.nodes[n.next];
            if n.index != na.index
                && next.index != na.index
                && n.index != nb.index
                && next.index != nb.index
                && self.intersects(p, n.next, a, b)
            {
                return false;
            }
            p = n.next;
            if p == a {
                break;
            }
        }
        (self.local(a, b)
            && self.local(b, a)
            && self.middle(a, b)
            && (self.area(na.prev, a, nb.prev) != 0. || self.area(a, nb.prev, b) != 0.))
            || (self.equal(a, b)
                && self.area(na.prev, a, na.next) > 0.
                && self.area(nb.prev, b, nb.next) > 0.)
    }
    fn cure(&mut self, mut start: usize) -> usize {
        let mut p = start;
        loop {
            let a = self.nodes[p].prev;
            let next = self.nodes[p].next;
            let b = self.nodes[next].next;
            if !self.equal(a, b)
                && self.intersects(a, p, next, b)
                && self.local(a, b)
                && self.local(b, a)
            {
                self.triangles.push([
                    self.nodes[a].index,
                    self.nodes[p].index,
                    self.nodes[b].index,
                ]);
                self.remove(p);
                self.remove(next);
                p = b;
                start = b;
            }
            p = self.nodes[p].next;
            if p == start {
                break;
            }
        }
        self.filter(p, None)
    }
    fn cut(&mut self, mut ear: usize, pass: u8, depth: usize) {
        if depth > 128 {
            self.exhausted = true;
            return;
        }
        let mut stop = ear;
        while self.nodes[ear].prev != self.nodes[ear].next {
            let prev = self.nodes[ear].prev;
            let next = self.nodes[ear].next;
            if self.ear(ear) {
                self.triangles.push([
                    self.nodes[prev].index,
                    self.nodes[ear].index,
                    self.nodes[next].index,
                ]);
                self.remove(ear);
                ear = self.nodes[next].next;
                stop = ear;
                continue;
            }
            ear = next;
            if ear == stop {
                match pass {
                    0 => {
                        let p = self.filter(ear, None);
                        self.cut(p, 1, depth + 1)
                    }
                    1 => {
                        let p = self.filter(ear, None);
                        let p = self.cure(p);
                        self.cut(p, 2, depth + 1)
                    }
                    _ => {
                        let mut a = ear;
                        loop {
                            let mut b = self.nodes[self.nodes[a].next].next;
                            while b != self.nodes[a].prev {
                                if self.nodes[a].index != self.nodes[b].index && self.diagonal(a, b)
                                {
                                    let c = self.split(a, b);
                                    let a = self.filter(a, Some(self.nodes[a].next));
                                    let c = self.filter(c, Some(self.nodes[c].next));
                                    self.cut(a, 0, depth + 1);
                                    self.cut(c, 0, depth + 1);
                                    return;
                                }
                                b = self.nodes[b].next;
                            }
                            a = self.nodes[a].next;
                            if a == ear {
                                break;
                            }
                        }
                    }
                }
                return;
            }
        }
    }
}
pub(crate) fn earcut(points: &[DVec2], holes: &[usize]) -> Result<Vec<[usize; 3]>, String> {
    let mut cut = Cut {
        nodes: vec![],
        triangles: vec![],
        exhausted: false,
    };
    if let Some(mut outer) = cut.ring(
        points,
        0..holes.first().copied().unwrap_or(points.len()),
        true,
    ) {
        if cut.nodes[outer].next != cut.nodes[outer].prev {
            outer = cut.holes(points, holes, outer);
            cut.cut(outer, 0, 0);
        }
    }
    if cut.exhausted {
        Err("Authoring coverage triangulation exceeded its recursion budget".into())
    } else {
        Ok(cut.triangles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_void_is_not_covered_by_compatibility_triangles() {
        let points = [
            (0., 0.),
            (4., 0.),
            (4., 4.),
            (0., 4.),
            (1., 1.),
            (1., 3.),
            (3., 3.),
            (3., 1.),
        ]
        .map(|(x, y)| DVec2::new(x, y));
        let triangles = earcut(&points, &[4]).unwrap();
        let area: f64 = triangles
            .iter()
            .map(|&[a, b, c]| ((points[b] - points[a]).perp_dot(points[c] - points[a])).abs() * 0.5)
            .sum();
        assert!((area - 12.).abs() < 1e-9);
        for [a, b, c] in triangles {
            assert!(!Cut::in_triangle(
                points[a],
                points[b],
                points[c],
                DVec2::splat(2.)
            ));
        }
    }
    #[test]
    fn degenerate_inputs_do_not_generate_coverage() {
        assert!(earcut(&[], &[]).unwrap().is_empty());
        assert!(earcut(&[DVec2::ZERO, DVec2::X, DVec2::X * 2.], &[])
            .unwrap()
            .is_empty());
    }
}
