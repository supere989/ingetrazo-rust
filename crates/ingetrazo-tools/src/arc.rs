// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    base::{Tool, ToolContext, ToolType},
    shape_plane::{self as qt, ShapePlane},
};
use glam::DVec3;
use ingetrazo_core::{mesh::Mesh, scene::Scene, screen_pick::PixelViewport};
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcMode {
    TwoPoint,
    ThreePoint,
    Center,
    Pie,
}
#[derive(Debug, Clone)]
struct LastArc {
    base: Arc<Mesh>,
    start: DVec3,
    end: DVec3,
    apex: (f64, f64),
    u: DVec3,
    v: DVec3,
    trim: Vec<(DVec3, DVec3)>,
    revision: Option<u64>,
}
#[derive(Debug, Clone)]
pub struct ArcTool {
    pub mode: ArcMode,
    pub start_point: Option<DVec3>,
    pub second_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub segments: usize,
    radius_trim: bool,
    plane: ShapePlane,
    error: Option<String>,
    viewport: Option<PixelViewport>,
    start_edge: Option<usize>,
    tangent: Option<DVec3>,
    equidistant: Option<(DVec3, DVec3, usize)>,
    fillet: Option<(DVec3, DVec3, DVec3)>,
    last_fillet_distance: Option<f64>,
    last: Option<LastArc>,
    snapped_bulge: Option<f64>,
    replacing: bool,
    committed_now: bool,
}
impl Default for ArcTool {
    fn default() -> Self {
        Self {
            mode: ArcMode::TwoPoint,
            start_point: None,
            second_point: None,
            hover_point: None,
            plane_normal: DVec3::Z,
            segments: 16,
            radius_trim: false,
            plane: Default::default(),
            error: None,
            viewport: None,
            start_edge: None,
            tangent: None,
            equidistant: None,
            fillet: None,
            last_fillet_distance: None,
            last: None,
            snapped_bulge: None,
            replacing: false,
            committed_now: false,
        }
    }
}
fn wrap(mut a: f64) -> f64 {
    while a <= -std::f64::consts::PI {
        a += std::f64::consts::TAU;
    }
    while a > std::f64::consts::PI {
        a -= std::f64::consts::TAU;
    }
    a
}
fn arc2(s: (f64, f64), m: (f64, f64), e: (f64, f64), n: usize) -> Vec<(f64, f64)> {
    let (ax, ay) = s;
    let (bx, by) = m;
    let (cx, cy) = e;
    let d = 2. * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return vec![s, e];
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    let r = (ax - ux).hypot(ay - uy);
    let a0 = (ay - uy).atan2(ax - ux);
    let a1 = (cy - uy).atan2(cx - ux);
    let am = (by - uy).atan2(bx - ux);
    let mut delta = wrap(a1 - a0);
    let da = wrap(am - a0);
    if delta >= 0. && !(0. <= da && da <= delta) {
        delta -= std::f64::consts::TAU;
    } else if delta < 0. && !(delta <= da && da <= 0.) {
        delta += std::f64::consts::TAU;
    }
    (0..=n)
        .map(|k| {
            let a = a0 + delta * (k as f64 / n as f64);
            (ux + r * a.cos(), uy + r * a.sin())
        })
        .collect()
}
impl ArcTool {
    pub fn set_repeat_distance(&mut self, d: Option<f64>) {
        self.last_fillet_distance = d;
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn three_point() -> Self {
        Self::with_mode(ArcMode::ThreePoint)
    }
    pub fn with_mode(mode: ArcMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }
    pub fn with_segments(segments: usize) -> Self {
        Self {
            segments: segments.max(2),
            ..Self::default()
        }
    }
    pub fn with_normal(normal: DVec3) -> Self {
        let mut t = Self::default();
        t.plane_normal = qt::norm(normal);
        t.plane.note(DVec3::ZERO, t.plane_normal);
        t
    }
    pub fn reset(&mut self) {
        self.radius_trim = false;
        self.start_point = None;
        self.second_point = None;
        self.plane.reset();
        self.start_edge = None;
        self.tangent = None;
        self.equidistant = None;
        self.fillet = None;
        self.snapped_bulge = None;
        self.error = None;
    }
    fn normal(&self, start: DVec3, end: DVec3, third: Option<DVec3>) -> DVec3 {
        let mut plane = self.plane.clone();
        if plane.hover.is_none() && plane.captured.is_none() {
            plane.note(start, self.plane_normal);
        }
        let mut points = vec![start, end];
        if let Some(p) = third {
            points.push(p);
        }
        plane.drawing(&points).1
    }
    fn basis(&self, start: DVec3, end: DVec3, third: Option<DVec3>) -> (DVec3, DVec3) {
        self.plane.axes_for(self.normal(start, end, third))
    }
    fn project(start: DVec3, p: DVec3, u: DVec3, v: DVec3) -> (f64, f64) {
        let d = qt::sub(p, start);
        (qt::dot(d, u), qt::dot(d, v))
    }
    fn lift(start: DVec3, p: (f64, f64), u: DVec3, v: DVec3) -> DVec3 {
        qt::add(qt::add(start, qt::mul(u, p.0)), qt::mul(v, p.1))
    }
    pub fn compute_3pt_arc(&self, s: DVec3, m: DVec3, e: DVec3) -> Vec<DVec3> {
        let (u, v) = self.basis(s, m, Some(e));
        let m = Self::project(s, m, u, v);
        let e = Self::project(s, e, u, v);
        if e.0.hypot(e.1) < 1e-9 || m.0.hypot(m.1) < 1e-9 {
            return vec![];
        }
        arc2((0., 0.), m, e, 16)
            .into_iter()
            .map(|p| Self::lift(s, p, u, v))
            .collect()
    }
    fn bulge(&self, s: DVec3, e: DVec3, p: DVec3) -> f64 {
        let (u, v) = self.basis(s, e, None);
        let e2 = Self::project(s, e, u, v);
        let l = e2.0.hypot(e2.1);
        if l < 1e-9 {
            return 0.;
        }
        let p = Self::project(s, p, u, v);
        (p.0 - e2.0 / 2.) * (-e2.1 / l) + (p.1 - e2.1 / 2.) * (e2.0 / l)
    }
    fn apex(&self, s: DVec3, e: DVec3, h: f64) -> ((f64, f64), DVec3, DVec3) {
        let (u, v) = self.basis(s, e, None);
        let p = Self::project(s, e, u, v);
        let l = p.0.hypot(p.1);
        ((p.0 / 2. - p.1 / l * h, p.1 / 2. + p.0 / l * h), u, v)
    }
    pub fn compute_2pt_arc_with_bulge(&self, s: DVec3, e: DVec3, h: f64) -> Vec<DVec3> {
        let (u, v) = self.basis(s, e, None);
        let p = Self::project(s, e, u, v);
        if p.0.hypot(p.1) < 1e-9 {
            return vec![];
        }
        if h.abs() < 1e-4 {
            return vec![s, e];
        }
        let (apex, _, _) = self.apex(s, e, h);
        arc2((0., 0.), apex, p, self.segments)
            .into_iter()
            .map(|p| Self::lift(s, p, u, v))
            .collect()
    }
    pub fn compute_2pt_arc(&self, s: DVec3, e: DVec3, p: DVec3) -> Vec<DVec3> {
        self.compute_2pt_arc_with_bulge(s, e, self.bulge(s, e, p))
    }
    fn sweep(&self, p: DVec3) -> f64 {
        let s = self.start_point.unwrap();
        let e = self.second_point.unwrap();
        let (u, v) = self.basis(s, e, None);
        let a = Self::project(s, e, u, v);
        let b = Self::project(s, p, u, v);
        wrap(b.1.atan2(b.0) - a.1.atan2(a.0)).to_degrees()
    }
    fn center_points(&self, sweep: f64) -> Vec<DVec3> {
        let (Some(s), Some(e)) = (self.start_point, self.second_point) else {
            return vec![];
        };
        if sweep.abs() < 1e-6 {
            return vec![];
        }
        let (u, v) = self.basis(s, e, None);
        let a = Self::project(s, e, u, v);
        let r = a.0.hypot(a.1);
        if r < 1e-6 {
            return vec![];
        }
        let a0 = a.1.atan2(a.0);
        let n = (sweep.abs() / 15.).round_ties_even().max(1.) as usize;
        (0..=n)
            .map(|k| {
                let a = a0 + sweep.to_radians() * k as f64 / n as f64;
                qt::add(
                    s,
                    qt::mul(qt::add(qt::mul(u, a.cos()), qt::mul(v, a.sin())), r),
                )
            })
            .collect()
    }
    fn tangent_bulge(&self, s: DVec3, e: DVec3, t: DVec3) -> Option<f64> {
        let (u, v) = self.basis(s, e, None);
        let p = Self::project(s, e, u, v);
        let l = p.0.hypot(p.1);
        let (tx, ty) = (qt::dot(t, u), qt::dot(t, v));
        let tl = tx.hypot(ty);
        if l < 1e-9 || tl < 1e-6 {
            return None;
        }
        let (cx, cy) = (p.0 / l, p.1 / l);
        let (tx, ty) = (tx / tl, ty / tl);
        let a = (cx * ty - cy * tx).atan2(cx * tx + cy * ty);
        if (a.abs() - std::f64::consts::PI).abs() < 1e-3 {
            return None;
        }
        Some(l / 2. * (a / 2.).tan())
    }
    fn curve_tangent_in(mesh: &Mesh, p: DVec3) -> Option<DVec3> {
        for edge in &mesh.edges {
            let Some(curve) = edge.curve_id else {
                continue;
            };
            if qt::length(qt::sub(mesh.vertices[edge.v0].position, p)) > 1e-6
                && qt::length(qt::sub(mesh.vertices[edge.v1].position, p)) > 1e-6
            {
                continue;
            }
            let chain = mesh
                .edges
                .iter()
                .filter(|e| e.curve_id == Some(curve))
                .collect::<Vec<_>>();
            let incident = chain
                .iter()
                .filter(|e| {
                    qt::length(qt::sub(mesh.vertices[e.v0].position, p)) <= 1e-6
                        || qt::length(qt::sub(mesh.vertices[e.v1].position, p)) <= 1e-6
                })
                .collect::<Vec<_>>();
            if incident.len() != 1 {
                continue;
            }
            let e = incident[0];
            let q = if qt::length(qt::sub(mesh.vertices[e.v0].position, p)) <= 1e-6 {
                mesh.vertices[e.v1].position
            } else {
                mesh.vertices[e.v0].position
            };
            let mut ids = std::collections::HashSet::new();
            let mut points = vec![];
            for e in chain {
                for id in [e.v0, e.v1] {
                    if ids.insert(id) {
                        points.push(mesh.vertices[id].position);
                    }
                }
            }
            let Some((center, _)) = ingetrazo_core::circle_inference::fit_circle(&points) else {
                continue;
            };
            let radial = qt::sub(p, qt::q(center));
            if qt::length(radial) < 1e-9 {
                continue;
            }
            let r = qt::norm(radial);
            let leaving = qt::sub(p, q);
            let tangent = qt::sub(leaving, qt::mul(r, qt::dot(leaving, r)));
            if qt::length(tangent) >= 1e-9 {
                return Some(qt::norm(tangent));
            }
        }
        None
    }
    fn curve_tangent(&self, scene: &Scene) -> Option<DVec3> {
        let start = self.start_point?;
        if let Some(t) = Self::curve_tangent_in(&scene.mesh, start) {
            return Some(t);
        }
        fn visit(
            items: &[ingetrazo_core::groups::Group],
            parent: glam::Mat4,
            start: DVec3,
        ) -> Option<DVec3> {
            for g in items {
                let xf = parent * g.transform.as_mat4();
                if !g.billboard && xf.is_finite() && xf.determinant().abs() > 1e-12 {
                    let p = xf.inverse().transform_point3(start.as_vec3()).as_dvec3();
                    if let Some(t) = ArcTool::curve_tangent_in(&g.mesh, p) {
                        let t = xf.transform_vector3(t.as_vec3()).as_dvec3();
                        if qt::length(t) >= 1e-9 {
                            return Some(qt::norm(t));
                        }
                    }
                }
                if let Some(t) = visit(&g.children, xf, start) {
                    return Some(t);
                }
            }
            None
        }
        visit(&scene.groups, glam::Mat4::IDENTITY, start)
    }
    fn edge_tangent(&self, m: &Mesh, p: DVec3) -> Option<DVec3> {
        let s = self.start_point?;
        let edge = m.edges.get(self.start_edge?)?;
        let mut t = qt::norm(qt::sub(
            m.vertices[edge.v1].position,
            m.vertices[edge.v0].position,
        ));
        if qt::dot(t, qt::sub(p, s)) < 0. {
            t = -t;
        }
        Some(t)
    }
    fn effective_bulge(&self, s: DVec3, e: DVec3, p: DVec3) -> f64 {
        self.snapped_bulge.unwrap_or_else(|| self.bulge(s, e, p))
    }
    fn snap_bulge(&mut self, p: DVec3) {
        self.snapped_bulge = None;
        let (Some(s), Some(e)) = (self.start_point, self.second_point) else {
            return;
        };
        let (u, v) = self.basis(s, e, None);
        let p2 = Self::project(s, e, u, v);
        let l = p2.0.hypot(p2.1);
        if l < 1e-9 {
            return;
        }
        let perp = qt::add(qt::mul(u, -p2.1 / l), qt::mul(v, p2.0 / l));
        let mid = qt::mul(qt::add(s, e), 0.5);
        let Some(scale) = self.plane.world_per_pixel(mid, perp) else {
            return;
        };
        let tol = 8. * scale;
        let h = self.bulge(s, e, p);
        if let Some(t) = self.tangent {
            if let Some(th) = self.tangent_bulge(s, e, t) {
                if (h - th).abs() <= tol {
                    self.snapped_bulge = Some(th);
                    return;
                }
            }
        }
        let half = l / 2.;
        if (h.abs() - half).abs() <= tol {
            self.snapped_bulge = Some(half.copysign(h));
        }
    }
    fn corner_trim(&self, m: &Mesh, h: Option<f64>, alt: bool) -> Vec<(DVec3, DVec3)> {
        if alt {
            return vec![];
        }
        let (Some((v, a, b)), Some(e), Some(h)) = (self.fillet, self.start_edge, h) else {
            return vec![];
        };
        if m.edges.get(e).map_or(true, |e| e.faces.is_empty()) {
            return vec![];
        }
        let Some(id) = m.vertex_at_position(v) else {
            return vec![];
        };
        if m.vertices[id].edges.len() != 2 {
            return vec![];
        }
        let Some(th) = self.tangent.and_then(|t| self.tangent_bulge(a, b, t)) else {
            return vec![];
        };
        if !self.radius_trim && (h - th).abs() > 1e-6 {
            return vec![];
        }
        vec![(v, a), (v, b)]
    }
    fn repeat_corner(&mut self, s: &mut Scene, c: &ToolContext) -> Result<bool, String> {
        let Some(d) = self.last_fillet_distance else {
            return Ok(false);
        };
        let mut best = None;
        for (id, v) in s.mesh.vertices.iter().enumerate() {
            if v.edges.len() != 2 {
                continue;
            }
            let ids = v.edges.iter().copied().collect::<Vec<_>>();
            let a = &s.mesh.edges[ids[0]];
            let b = &s.mesh.edges[ids[1]];
            if a.faces.is_empty() && b.faces.is_empty() {
                continue;
            }
            let other = |e: &ingetrazo_core::mesh::MeshEdge| if e.v0 == id { e.v1 } else { e.v0 };
            let va = qt::sub(s.mesh.vertices[other(a)].position, v.position);
            let vb = qt::sub(s.mesh.vertices[other(b)].position, v.position);
            if qt::length(va) < d - 1e-6 || qt::length(vb) < d - 1e-6 {
                continue;
            }
            let distance = self.pixel_distance(c, v.position);
            if distance <= 20. && best.as_ref().map_or(true, |(old, _)| distance < *old) {
                best = Some((distance, (v.position, va, vb, ids[0])));
            }
        }
        let Some((_, (v, a, b, id))) = best else {
            return Ok(false);
        };
        self.reset();
        let start = qt::add(v, qt::mul(qt::norm(a), d));
        let end = qt::add(v, qt::mul(qt::norm(b), d));
        self.start_point = Some(start);
        self.second_point = Some(end);
        self.start_edge = Some(id);
        self.fillet = Some((v, start, end));
        let n = qt::cross(a, b);
        if qt::length(n) < 1e-9 {
            self.reset();
            return Ok(false);
        }
        self.plane.note(v, qt::norm(n));
        self.tangent = Some(qt::norm(qt::sub(v, start)));
        let Some(h) = self.tangent_bulge(start, end, self.tangent.unwrap()) else {
            self.reset();
            return Ok(false);
        };
        self.snapped_bulge = Some(h);
        let pts = self.compute_2pt_arc_with_bulge(start, end, h);
        self.commit(pts, s, Some(h), c.alt_held)?;
        Ok(true)
    }
    pub fn preview_points(&self) -> Option<Vec<DVec3>> {
        let s = self.start_point?;
        let e = self.second_point?;
        let p = self.hover_point?;
        Some(match self.mode {
            ArcMode::TwoPoint => {
                self.compute_2pt_arc_with_bulge(s, e, self.effective_bulge(s, e, p))
            }
            ArcMode::ThreePoint => self.compute_3pt_arc(s, e, p),
            ArcMode::Center | ArcMode::Pie => self.center_points(self.sweep(p)),
        })
    }
    fn pixel_distance(&self, c: &ToolContext, p: DVec3) -> f64 {
        if let (Some(vp), Some(screen)) = (self.viewport, c.screen) {
            if let Some(q) = vp.project_for_snapping(p) {
                return q.distance(screen);
            }
        }
        qt::length(qt::sub(c.world, p)) * 100.
    }
    fn edge_under(m: &Mesh, p: DVec3) -> Option<usize> {
        m.edges.iter().enumerate().find_map(|(i, e)| {
            let a = m.vertices[e.v0].position;
            let b = m.vertices[e.v1].position;
            let d = qt::sub(b, a);
            let l = qt::length(d);
            if l < 1e-6 || qt::length(qt::sub(p, a)) <= 1e-4 || qt::length(qt::sub(p, b)) <= 1e-4 {
                return None;
            }
            let t = qt::dot(qt::sub(p, a), d) / (l * l);
            (t >= 0. && t <= 1. && qt::length(qt::sub(p, qt::add(a, qt::mul(d, t)))) < 1e-4)
                .then_some(i)
        })
    }
    fn nearest_equal(&self, m: &Mesh, c: &ToolContext) -> Option<(DVec3, DVec3, usize)> {
        let s = self.start_point?;
        let a = m.edges.get(self.start_edge?)?;
        let mut best = None;
        for id in [a.v0, a.v1] {
            let corner = m.vertices[id].position;
            let d = qt::length(qt::sub(s, corner));
            if d < 1e-6 {
                continue;
            }
            for &i in &m.vertices[id].edges {
                if i == self.start_edge.unwrap() {
                    continue;
                }
                let b = &m.edges[i];
                let other = if b.v0 == id { b.v1 } else { b.v0 };
                let span = qt::sub(m.vertices[other].position, corner);
                let l = qt::length(span);
                if l < 1e-9 || d > l + 1e-6 {
                    continue;
                }
                let p = qt::add(corner, qt::mul(span, d / l));
                let dist = self.pixel_distance(c, p);
                if dist <= 10. && best.as_ref().map_or(true, |(bd, _)| dist < *bd) {
                    best = Some((dist, (corner, p, i)));
                }
            }
        }
        best.map(|(_, v)| v)
    }
    fn trimmed_mesh(mut mesh: Mesh, trim: &[(DVec3, DVec3)]) -> Mesh {
        if trim.is_empty() {
            return mesh;
        }
        let doomed = mesh
            .edges
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let a = mesh.vertices[e.v0].position;
                let b = mesh.vertices[e.v1].position;
                trim.iter()
                    .any(|(s, t)| {
                        (qt::length(qt::sub(a, *s)) < 1e-4 && qt::length(qt::sub(b, *t)) < 1e-4)
                            || (qt::length(qt::sub(a, *t)) < 1e-4
                                && qt::length(qt::sub(b, *s)) < 1e-4)
                    })
                    .then_some(i)
            })
            .collect::<std::collections::HashSet<_>>();
        let faces = doomed
            .iter()
            .flat_map(|i| mesh.edges[*i].faces.iter().copied())
            .collect::<std::collections::HashSet<_>>();
        let mut i = 0;
        mesh.faces.retain(|_| {
            let keep = !faces.contains(&i);
            i += 1;
            keep
        });
        let mut i = 0;
        mesh.edges.retain(|_| {
            let keep = !doomed.contains(&i);
            i += 1;
            keep
        });
        mesh.rebuild_incidence();
        mesh
    }
    fn prepare_mesh(
        &self,
        base: &Mesh,
        pts: &[DVec3],
        close: Option<DVec3>,
        trim: &[(DVec3, DVec3)],
        s: &Scene,
    ) -> Result<Mesh, String> {
        let mut segments = pts.windows(2).map(|p| (p[0], p[1])).collect::<Vec<_>>();
        let explicit = close.map(|c| {
            segments.insert(0, (c, pts[0]));
            segments.push((*pts.last().unwrap(), c));
            std::iter::once(c)
                .chain(pts.iter().copied())
                .collect::<Vec<_>>()
        });
        let curve = base
            .edges
            .iter()
            .filter_map(|e| e.curve_id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Curve identity exhausted")?;
        let mut mesh = ingetrazo_core::drafting::add_segments_tagged_with_id(
            base,
            &segments,
            explicit.as_deref(),
            Some(curve),
            s.active_ifc.as_ref(),
            s.bim_draw_id.as_ref(),
        )?;
        // Pie radius spokes are plain edges; only the actual arc is a curve.
        if close.is_some() {
            for e in &mut mesh.edges {
                if e.curve_id == Some(curve) {
                    let a = mesh.vertices[e.v0].position;
                    let b = mesh.vertices[e.v1].position;
                    if !pts.windows(2).any(|p| {
                        (a.distance(p[0]) < 1e-4 && b.distance(p[1]) < 1e-4)
                            || (b.distance(p[0]) < 1e-4 && a.distance(p[1]) < 1e-4)
                    }) {
                        e.curve_id = None;
                    }
                }
            }
        }
        let mesh = Self::trimmed_mesh(mesh, trim);
        ingetrazo_core::planar::validate_mesh(&mesh)?;
        Ok(mesh)
    }
    fn commit(
        &mut self,
        pts: Vec<DVec3>,
        s: &mut Scene,
        h: Option<f64>,
        alt: bool,
    ) -> Result<(), String> {
        if pts.len() < 2 {
            return Err("Arc requires distinct points".into());
        }
        let trim = self.corner_trim(&s.mesh, h, alt);
        let close = (self.mode == ArcMode::Pie).then_some(self.start_point.unwrap());
        let mesh = self.prepare_mesh(&s.mesh, &pts, close, &trim, s)?;
        let last =
            if let (Some(h), Some(start), Some(end)) = (h, self.start_point, self.second_point) {
                let (apex, u, v) = self.apex(start, end, h);
                Some(LastArc {
                    base: Arc::new(s.mesh.clone()),
                    start,
                    end,
                    apex,
                    u,
                    v,
                    trim: trim.clone(),
                    revision: None,
                })
            } else {
                None
            };
        if !trim.is_empty() {
            if let Some((v, a, _)) = self.fillet {
                self.last_fillet_distance = Some(qt::length(qt::sub(a, v)));
            }
        }
        s.mesh = mesh;
        s.bump_version();
        self.reset();
        self.last = last;
        self.committed_now = true;
        Ok(())
    }
    fn click_result(&mut self, s: &mut Scene, c: &ToolContext) -> Result<(), String> {
        self.plane.axes = s.drawing_axes();
        let p = qt::q(c.world);
        if self.start_point.is_none() {
            self.last = None;
            self.start_point = Some(p);
            self.plane.start(p);
            if self.mode == ArcMode::TwoPoint {
                self.start_edge = Self::edge_under(&s.mesh, p);
            }
            return Ok(());
        }
        if self.second_point.is_none() {
            let p = self.equidistant.map(|(_, p, _)| p).unwrap_or(p);
            if qt::length(qt::sub(p, self.start_point.unwrap())) < 1e-6 {
                return Ok(());
            }
            self.second_point = Some(p);
            self.plane.adopt(&[self.start_point.unwrap(), p]);
            if let Some((v, b, _)) = self.equidistant.take() {
                self.fillet = Some((v, self.start_point.unwrap(), b));
            }
            self.tangent = self
                .curve_tangent(s)
                .or_else(|| self.edge_tangent(&s.mesh, p));
            return Ok(());
        }
        let start = self.start_point.unwrap();
        let end = self.second_point.unwrap();
        let h = (self.mode == ArcMode::TwoPoint).then(|| self.effective_bulge(start, end, p));
        let pts = match self.mode {
            ArcMode::TwoPoint => self.compute_2pt_arc_with_bulge(start, end, h.unwrap()),
            ArcMode::ThreePoint => self.compute_3pt_arc(start, end, p),
            ArcMode::Center | ArcMode::Pie => self.center_points(self.sweep(p)),
        };
        self.commit(pts, s, h, c.alt_held)
    }
}
impl Tool for ArcTool {
    fn shape_segments(&self) -> Option<usize> {
        (self.mode == ArcMode::TwoPoint).then_some(self.segments)
    }
    fn arc_mode(&self) -> Option<ArcMode> {
        Some(self.mode)
    }
    fn arc_repeat_distance(&self) -> Option<f64> {
        self.last_fillet_distance
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Arc
    }
    fn name(&self) -> &'static str {
        match self.mode {
            ArcMode::TwoPoint => "2-Point Arc",
            ArcMode::ThreePoint => "3-Point Arc",
            ArcMode::Center => "Center Arc",
            ArcMode::Pie => "Pie",
        }
    }
    fn shortcut(&self) -> Option<&'static str> {
        match self.mode {
            ArcMode::TwoPoint => Some("A"),
            ArcMode::ThreePoint => Some("J"),
            ArcMode::Center => Some("Shift+O"),
            ArcMode::Pie => None,
        }
    }
    fn vcb_label(&self) -> &'static str {
        match self.mode {
            ArcMode::TwoPoint => "Bulge",
            ArcMode::ThreePoint => "",
            ArcMode::Center | ArcMode::Pie => {
                if self.second_point.is_some() {
                    "Angle"
                } else {
                    "Radius"
                }
            }
        }
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.start_point
    }
    fn magnetic_axis_deg(&self) -> Option<f64> {
        (self.mode != ArcMode::TwoPoint || self.second_point.is_none()).then_some(3.)
    }
    fn screen_axis_px(&self) -> Option<f64> {
        self.magnetic_axis_deg().map(|_| 9.)
    }
    fn note_cursor_plane(&mut self, p: DVec3, n: DVec3) {
        self.plane.note(p, n);
        self.plane_normal = n;
    }
    fn note_pick_viewport(&mut self, v: PixelViewport, _: DVec3, _: u64) {
        self.plane.viewport = Some(v);
        self.viewport = Some(v);
    }
    fn lock_drawing_plane(&mut self, n: DVec3) -> bool {
        self.start_point.is_none() && self.plane.lock_axis(n)
    }
    fn lock_drawing_reference(
        &mut self,
        r: Option<ingetrazo_core::screen_pick::HoverReference>,
    ) -> bool {
        self.start_point.is_none() && self.plane.lock_reference(r)
    }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> {
        let Some(s) = self.start_point else {
            return self.plane.locked(self.hover_point);
        };
        let e = self.second_point.or(self.hover_point).unwrap_or(s);
        let mut p = vec![s, e];
        if self.mode == ArcMode::ThreePoint {
            if let Some(h) = self.hover_point {
                p.push(h);
            }
        }
        Some(self.plane.drawing(&p))
    }
    fn on_activate(&mut self, s: &mut Scene) {
        self.plane.axes = s.drawing_axes();
        self.reset();
        self.hover_point = None;
        self.last = None;
    }
    fn on_deactivate(&mut self, _: &mut Scene) {
        self.reset();
        self.hover_point = None;
        self.last = None;
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_move(&mut self, s: &mut Scene, c: &ToolContext) {
        self.plane.axes = s.drawing_axes();
        self.hover_point = Some(qt::q(c.world));
        self.equidistant = None;
        if self.mode == ArcMode::TwoPoint
            && self.start_point.is_some()
            && self.second_point.is_none()
        {
            self.tangent = self.edge_tangent(&s.mesh, c.world);
            self.equidistant = self.nearest_equal(&s.mesh, c);
            if let Some((_, p, _)) = self.equidistant {
                self.hover_point = Some(p);
            }
        } else if self.mode == ArcMode::TwoPoint && self.second_point.is_some() {
            self.snap_bulge(c.world);
        }
    }
    fn on_click(&mut self, s: &mut Scene, c: &ToolContext) {
        if let Err(e) = self.click_result(s, c) {
            self.error = Some(e);
        }
    }
    fn on_double_click(&mut self, s: &mut Scene, c: &ToolContext) {
        if self.mode == ArcMode::TwoPoint
            && self.start_point.is_some()
            && self.second_point.is_none()
        {
            match self.repeat_corner(s, c) {
                Ok(true) => return,
                Err(e) => {
                    self.error = Some(e);
                    return;
                }
                Ok(false) => {}
            }
        }
        if self.mode == ArcMode::TwoPoint && self.fillet.is_some() && self.second_point.is_some() {
            let start = self.start_point.unwrap();
            let end = self.second_point.unwrap();
            if let Some(t) = self.tangent {
                if let Some(h) = self.tangent_bulge(start, end, t) {
                    let pts = self.compute_2pt_arc_with_bulge(start, end, h);
                    if let Err(e) = self.commit(pts, s, Some(h), c.alt_held) {
                        self.error = Some(e);
                    }
                    return;
                }
            }
        }
        self.on_click(s, c);
    }
    fn on_vcb_input(&mut self, s: &mut Scene, text: &str) -> Result<(), String> {
        let text = text.trim();
        if self.mode == ArcMode::ThreePoint {
            return Err("Three-point arc has no numeric override".into());
        }
        if text.ends_with(['s', 'S']) {
            if self.mode != ArcMode::TwoPoint {
                return Err("Center arc uses fixed 15 degree pitch".into());
            }
            let n = ingetrazo_core::units::vcb::parse_single(&text[..text.len() - 1], 1.)?;
            if n < 2. || n != n.trunc() {
                return Err("Arc requires integer segments >=2".into());
            }
            let n = n as usize;
            if let Some(last) = self.last.clone() {
                let e = Self::project(last.start, last.end, last.u, last.v);
                let pts = arc2((0., 0.), last.apex, e, n)
                    .into_iter()
                    .map(|p| Self::lift(last.start, p, last.u, last.v))
                    .collect::<Vec<_>>();
                let old = self.segments;
                self.segments = n;
                let result = self.prepare_mesh(&last.base, &pts, None, &last.trim, s);
                match result {
                    Ok(mesh) => {
                        s.mesh = mesh;
                        s.bump_version();
                        self.replacing = true;
                        self.committed_now = true;
                    }
                    Err(e) => {
                        self.segments = old;
                        return Err(e);
                    }
                }
            } else {
                self.segments = n;
            }
            return Ok(());
        }
        let start = self.start_point.ok_or("Place an arc start first")?;
        let end = self.second_point.ok_or("Place the second point first")?;
        if matches!(self.mode, ArcMode::Center | ArcMode::Pie) {
            let angle = ingetrazo_core::units::vcb::parse_angle(text)?.abs();
            let sign = self
                .hover_point
                .map(|p| if self.sweep(p) < 0. { -1. } else { 1. })
                .unwrap_or(-1.);
            return self.commit(self.center_points(sign * angle), s, None, false);
        }
        let radius = text.ends_with(['r', 'R']);
        let value = ingetrazo_core::units::vcb::parse_single(
            if radius {
                &text[..text.len() - 1]
            } else {
                text
            },
            ingetrazo_core::units::model_scale(s),
        )?;
        let sign = self
            .hover_point
            .map(|p| {
                if self.bulge(start, end, p) < 0. {
                    -1.
                } else {
                    1.
                }
            })
            .unwrap_or(1.);
        let h = if radius {
            let half = qt::length(qt::sub(end, start)) * 0.5;
            if value <= 0. || half < 1e-9 {
                return Err("Radius must be positive with a nonzero chord".into());
            }
            if value < half - 1e-9 {
                self.error = Some("Radius is smaller than half the chord".into());
                return Ok(());
            }
            let r = value.max(half);
            let magnitude = r - (r * r - half * half).max(0.).sqrt();
            let mut sign = sign;
            if self.fillet.is_some() {
                if let Some(th) = self.tangent.and_then(|t| self.tangent_bulge(start, end, t)) {
                    if (th.abs() - magnitude).abs() <= 1e-6 * r.max(1.) {
                        sign = th.signum();
                        self.radius_trim = true;
                    }
                }
            }
            sign * magnitude
        } else {
            self.hover_point
                .ok_or("Move to set the bulge direction first")?;
            if let Some((corner, a, b)) = self.fillet {
                if value <= 0. {
                    return Err("Fillet radius must be positive".into());
                }
                let ua = qt::norm(qt::sub(a, corner));
                let ub = qt::norm(qt::sub(b, corner));
                let theta = qt::dot(ua, ub).clamp(-1., 1.).acos();
                if theta < 1e-3 || (theta - std::f64::consts::PI).abs() < 1e-3 {
                    return Err("No corner to round".into());
                }
                let d = value / (theta / 2.).tan();
                let reach = s
                    .mesh
                    .vertex_at_position(corner)
                    .map(|id| {
                        s.mesh.vertices[id]
                            .edges
                            .iter()
                            .filter_map(|i| {
                                let e = &s.mesh.edges[*i];
                                let span = qt::sub(
                                    s.mesh.vertices[e.v1].position,
                                    s.mesh.vertices[e.v0].position,
                                );
                                let n = qt::norm(span);
                                ((qt::dot(n, ua).abs() > 1. - 1e-5)
                                    || (qt::dot(n, ub).abs() > 1. - 1e-5))
                                    .then_some(qt::length(span))
                            })
                            .fold(f64::INFINITY, f64::min)
                    })
                    .unwrap_or(f64::INFINITY);
                if d > reach + 1e-6 {
                    self.error = Some("Fillet radius exceeds the shorter edge".into());
                    return Ok(());
                }
                let a = qt::add(corner, qt::mul(ua, d));
                let b = qt::add(corner, qt::mul(ub, d));
                self.start_point = Some(a);
                self.second_point = Some(b);
                self.fillet = Some((corner, a, b));
                self.tangent = Some(qt::norm(qt::sub(corner, a)));
                let h = self
                    .tangent_bulge(a, b, self.tangent.unwrap())
                    .ok_or("No tangent arc")?;
                self.radius_trim = true;
                return self.commit(self.compute_2pt_arc_with_bulge(a, b, h), s, Some(h), false);
            }
            sign * value
        };
        let pts = self.compute_2pt_arc_with_bulge(start, end, h);
        self.commit(pts, s, Some(h), false)
    }
    fn take_history_replacement(&mut self) -> bool {
        std::mem::take(&mut self.replacing)
    }
    fn note_history_revision(&mut self, r: u64, committed: bool) {
        if committed && self.committed_now {
            if let Some(last) = &mut self.last {
                last.revision = Some(r);
            }
            self.committed_now = false;
        } else if self
            .last
            .as_ref()
            .is_some_and(|l| l.revision.is_some_and(|old| old != r))
        {
            self.last = None;
        }
    }
    fn preview_color(&self) -> Option<[f32; 3]> {
        if self.mode == ArcMode::TwoPoint && self.start_point.is_some() {
            if self.second_point.is_none() && self.start_edge.is_some() {
                return Some(if self.equidistant.is_some() {
                    ingetrazo_core::snap::COLOR_REFERENCE
                } else {
                    ingetrazo_core::snap::COLOR_MIDPOINT
                });
            }
            if let Some(h) = self.snapped_bulge {
                let tangent = self
                    .start_point
                    .zip(self.second_point)
                    .and_then(|(s, e)| self.tangent.and_then(|t| self.tangent_bulge(s, e, t)))
                    .is_some_and(|t| t == h);
                return Some(if tangent && self.fillet.is_some() {
                    ingetrazo_core::snap::COLOR_REFERENCE
                } else {
                    ingetrazo_core::snap::COLOR_MIDPOINT
                });
            }
        }
        self.plane.lock_color()
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        let (Some(start), Some(p)) = (self.start_point, self.hover_point) else {
            return vec![];
        };
        if self.second_point.is_none() {
            if self.mode == ArcMode::TwoPoint {
                if let Some(t) = self.tangent {
                    if let Some(h) = self.tangent_bulge(start, p, t) {
                        let pts = self.compute_2pt_arc_with_bulge(start, p, h);
                        return pts.windows(2).map(|p| (p[0], p[1])).collect();
                    }
                }
            }
            return vec![(start, p)];
        }
        let mut out = self
            .preview_points()
            .unwrap_or_default()
            .windows(2)
            .map(|p| (p[0], p[1]))
            .collect::<Vec<_>>();
        if matches!(self.mode, ArcMode::Center | ArcMode::Pie) {
            out.insert(0, (start, self.second_point.unwrap()));
            out.insert(1, (start, p));
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_2point_arc_tool() {
        let mut tool = ArcTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx1 = ToolContext::new(DVec3::new(4.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);
        assert_eq!(tool.second_point, Some(DVec3::new(4.0, 0.0, 0.0)));

        let ctx2 = ToolContext::new(DVec3::new(2.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx2);

        assert_eq!(scene.mesh.edges.len(), tool.segments);
        assert!(tool.start_point.is_none());
    }

    #[test]
    fn test_3point_arc_tool() {
        let mut tool = ArcTool::three_point();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 2.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        let ctx2 = ToolContext::new(DVec3::new(4.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx2);

        assert_eq!(scene.mesh.edges.len(), tool.segments);
    }
}
