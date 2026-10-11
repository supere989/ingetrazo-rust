// SPDX-License-Identifier: GPL-3.0-or-later
//! Source PlaneLock for the shape tools. Only newly authored tool coordinates
//! use QVector3D arithmetic; existing document positions remain untouched.
use glam::{DVec3, Vec3};
use ingetrazo_core::{
    drawing_axes::DrawingAxes,
    screen_pick::{HoverReference, PixelViewport, ReferenceKind},
};
pub(crate) fn q(p: DVec3) -> DVec3 {
    p.as_vec3().as_dvec3()
}
pub(crate) fn add(a: DVec3, b: DVec3) -> DVec3 {
    (a.as_vec3() + b.as_vec3()).as_dvec3()
}
pub(crate) fn sub(a: DVec3, b: DVec3) -> DVec3 {
    (a.as_vec3() - b.as_vec3()).as_dvec3()
}
pub(crate) fn mul(a: DVec3, b: f64) -> DVec3 {
    (a.as_vec3() * b as f32).as_dvec3()
}
pub(crate) fn dot(a: DVec3, b: DVec3) -> f64 {
    let a = a.as_vec3();
    let b = b.as_vec3();
    ((a.x * b.x + a.y * b.y) + a.z * b.z) as f64
}
pub(crate) fn cross(a: DVec3, b: DVec3) -> DVec3 {
    a.as_vec3().cross(b.as_vec3()).as_dvec3()
}
pub(crate) fn length(a: DVec3) -> f64 {
    let a = a.as_vec3();
    let m = a.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let n = Vec3::new(a.x / m, a.y / m, a.z / m);
    (m * ((n.x * n.x + n.y * n.y) + n.z * n.z).sqrt()) as f64
}
pub(crate) fn norm(a: DVec3) -> DVec3 {
    let l = length(a) as f32;
    if (l - 1.).abs() < 1e-5 {
        return q(a);
    }
    if l.abs() < 1e-5 {
        return DVec3::ZERO;
    }
    let a = a.as_vec3();
    Vec3::new(a.x / l, a.y / l, a.z / l).as_dvec3()
}
#[derive(Debug, Clone)]
pub(crate) struct ShapePlane {
    pub axes: DrawingAxes,
    pub viewport: Option<PixelViewport>,
    pub hover: Option<(DVec3, DVec3)>,
    pub captured: Option<(DVec3, DVec3)>,
    lock: Option<DVec3>,
    reference: Option<ReferenceKind>,
}
impl Default for ShapePlane {
    fn default() -> Self {
        Self {
            axes: Default::default(),
            viewport: None,
            hover: None,
            captured: None,
            lock: None,
            reference: None,
        }
    }
}
impl ShapePlane {
    pub fn reset(&mut self) {
        self.hover = None;
        self.captured = None;
        self.lock = None;
        self.reference = None;
    }
    pub fn note(&mut self, p: DVec3, n: DVec3) {
        if p.is_finite() && n.is_finite() && length(n) > 1e-9 {
            self.hover = Some((q(p), q(n)));
        }
    }
    pub fn lock_axis(&mut self, n: DVec3) -> bool {
        if !n.is_finite() || length(n) < 1e-9 {
            return false;
        }
        let had = self.reference.take().is_some();
        self.lock = if !had && self.lock.is_some_and(|v| dot(v, n).abs() > 0.999) {
            None
        } else {
            Some(norm(n))
        };
        true
    }
    pub fn lock_reference(&mut self, r: Option<HoverReference>) -> bool {
        if self.reference.is_some() {
            self.reference = None;
            self.lock = None;
        } else if let Some(r) = r {
            if r.normal.is_finite() && length(r.normal) > 1e-9 {
                self.reference = Some(r.kind);
                self.lock = Some(norm(r.normal));
            }
        }
        true
    }
    pub fn locked(&self, p: Option<DVec3>) -> Option<(DVec3, DVec3)> {
        self.lock.map(|n| (p.unwrap_or(self.axes.origin), n))
    }
    pub fn start(&mut self, p: DVec3) {
        self.captured = self.lock.map(|n| (q(p), n));
    }
    pub fn own(&self) -> (DVec3, DVec3) {
        self.captured
            .or(self.hover)
            .unwrap_or((q(self.axes.origin), q(self.axes.z)))
    }
    pub fn drawing(&self, pts: &[DVec3]) -> (DVec3, DVec3) {
        let plane = self.own();
        if self.lock.is_some() || pts.len() < 2 {
            return plane;
        }
        let n = norm(plane.1);
        if pts.iter().all(|p| dot(sub(*p, plane.0), n).abs() < 1e-3) {
            return plane;
        }
        let a = q(pts[0]);
        if pts.len() >= 3 {
            let through = cross(sub(pts[1], a), sub(pts[2], a));
            if length(through) > 1e-6 {
                return (a, norm(through));
            }
        }
        let span = sub(*pts.last().unwrap(), a);
        let mut flat = None;
        for n in [self.axes.x, self.axes.y, self.axes.z] {
            if dot(span, n).abs() < 1e-3 {
                if flat.is_some() {
                    return plane;
                }
                flat = Some(q(n));
            }
        }
        flat.map(|n| (a, n)).unwrap_or(plane)
    }
    pub fn adopt(&mut self, pts: &[DVec3]) {
        let plane = self.drawing(pts);
        if plane != self.own() {
            self.captured = Some(plane);
        }
    }
    pub fn world_per_pixel(&self, p: DVec3, along: DVec3) -> Option<f64> {
        let view = self.viewport?;
        let a = view.project_for_snapping(p)?;
        let b = view.project_for_snapping(add(p, mul(norm(along), 0.25)))?;
        let d = a.distance(b);
        (d > 1e-6).then_some(0.25 / d)
    }
    pub fn lock_color(&self) -> Option<[f32; 3]> {
        if self.reference.is_some() {
            return Some(ingetrazo_core::snap::COLOR_REFERENCE);
        }
        let n = self.lock?;
        for (axis, color) in [
            (self.axes.x, ingetrazo_core::snap::COLOR_AXIS_X),
            (self.axes.y, ingetrazo_core::snap::COLOR_AXIS_Y),
            (self.axes.z, ingetrazo_core::snap::COLOR_AXIS_Z),
        ] {
            if dot(norm(n), axis).abs() > 0.99 {
                return Some(color);
            }
        }
        None
    }
    pub fn cursor_ring(&self, p: DVec3, n: usize) -> Vec<(DVec3, DVec3)> {
        let normal = self.lock.unwrap_or(self.own().1);
        let (u, v) = self.axes_for(normal);
        let Some(scale) = self.world_per_pixel(p, u) else {
            return vec![];
        };
        if n < 3 {
            return vec![];
        }
        let r = 22. * scale;
        let points = (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                add(add(p, mul(u, r * a.cos())), mul(v, r * a.sin()))
            })
            .collect::<Vec<_>>();
        (0..n).map(|i| (points[i], points[(i + 1) % n])).collect()
    }
    pub fn axes_for(&self, n: DVec3) -> (DVec3, DVec3) {
        let n = norm(n);
        let mut u = sub(self.axes.x, mul(n, dot(self.axes.x, n)));
        if length(u) < 0.1 {
            u = sub(self.axes.y, mul(n, dot(self.axes.y, n)));
        }
        let u = norm(u);
        (u, norm(cross(n, u)))
    }
}
