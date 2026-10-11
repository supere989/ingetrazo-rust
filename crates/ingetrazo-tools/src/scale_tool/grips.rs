// SPDX-License-Identifier: GPL-3.0-or-later
//! Source Scale grip-box geometry. Built only when selection bounds change.
use glam::{DVec3, Vec3};
use ingetrazo_core::math::Ray;
use serde::{Deserialize, Serialize};
pub const FLAT: f64 = 1e-6;
pub const MIN_FACTOR: f64 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grip {
    pub params: [f64; 3],
    pub mask: [bool; 3],
}
impl Grip {
    pub fn axes(self) -> impl Iterator<Item = usize> {
        (0..3).filter(move |&i| self.mask[i])
    }
    pub fn is_corner(self, active: usize) -> bool {
        self.axes().count() >= active
    }
}
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub origin: DVec3,
    pub axes: [DVec3; 3],
}
#[derive(Debug, Clone)]
pub struct GripBox {
    pub low: DVec3,
    pub high: DVec3,
    pub frame: Option<Frame>,
    pub grips: Vec<Grip>,
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    (a.x * b.x + a.y * b.y + a.z * b.z) as f64
}
fn length(v: Vec3) -> f64 {
    let m = v.abs().max_element();
    if m == 0. { return 0.; }
    let n = Vec3::new(v.x / m, v.y / m, v.z / m);
    (m * (n.x * n.x + n.y * n.y + n.z * n.z).sqrt()) as f64
}
impl GripBox {
    pub fn new(low: DVec3, high: DVec3, frame: Option<Frame>) -> Option<Self> {
        if !low.is_finite() || !high.is_finite() || low.cmpgt(high).any() {
            return None;
        }
        let mut result = Self {
            low: low.as_vec3().as_dvec3(),
            high: high.as_vec3().as_dvec3(),
            frame,
            grips: Vec::with_capacity(26),
        };
        if !result.low.is_finite() || !result.high.is_finite() { return None; }
        let ext = result.extents();
        if !(0..3).any(|i| ext[i] > FLAT) { return None; }
        if result.frame.is_some_and(|f| !f.origin.is_finite()
            || f.axes.iter().any(|a| !a.is_finite())) { return None; }
        for x in [0., 0.5, 1.] {
            for y in [0., 0.5, 1.] {
                for z in [0., 0.5, 1.] {
                    let params = [x, y, z];
                    if (0..3).any(|i| ext[i] <= FLAT && params[i] != 0.5) { continue; }
                    let mask = std::array::from_fn(|i| ext[i] > FLAT && params[i] != 0.5);
                    if mask.iter().any(|&m| m) { result.grips.push(Grip { params, mask }); }
                }
            }
        }
        Some(result)
    }
    pub fn extents(&self) -> DVec3 { self.high - self.low }
    pub fn active_axes(&self) -> usize {
        let ext = self.extents();
        (0..3).filter(|&i| ext[i] > FLAT).count()
    }
    pub fn to_local(&self, point: DVec3) -> DVec3 {
        let p = point.as_vec3();
        let Some(frame) = self.frame else { return p.as_dvec3(); };
        let d = p - frame.origin.as_vec3();
        DVec3::from_array(frame.axes.map(|a| dot(d, a.as_vec3())))
    }
    pub fn to_world(&self, point: DVec3) -> DVec3 {
        let p = point.as_vec3();
        let Some(frame) = self.frame else { return p.as_dvec3(); };
        (frame.origin.as_vec3() + frame.axes[0].as_vec3() * p.x
            + frame.axes[1].as_vec3() * p.y + frame.axes[2].as_vec3() * p.z).as_dvec3()
    }
    pub fn position(&self, grip: Grip) -> DVec3 {
        self.to_world(self.low + self.extents() * DVec3::from_array(grip.params))
    }
    pub fn anchor(&self, grip: Grip, center: bool) -> DVec3 {
        if center {
            return self.to_world(((self.low.as_vec3() + self.high.as_vec3()) * 0.5).as_dvec3());
        }
        let opposite = Grip {
            params: std::array::from_fn(|i| if grip.mask[i] { 1. - grip.params[i] } else { 0.5 }),
            mask: grip.mask,
        };
        self.position(opposite)
    }
    fn uniform(&self, grip: Grip, toggled: bool) -> bool {
        toggled || grip.is_corner(self.active_axes())
    }
    pub fn factors_from_point(
        &self, grip: Grip, anchor: DVec3, point: DVec3, uniform: bool,
    ) -> Option<DVec3> {
        if !point.is_finite() || !anchor.is_finite() { return None; }
        let g = self.position(grip).as_vec3();
        let a = anchor.as_vec3();
        let uniform = self.uniform(grip, uniform);
        if uniform || grip.axes().count() == 1 {
            let direction = g - a;
            let len = length(direction);
            if len < 1e-12 { return None; }
            let f = dot(point.as_vec3() - a, direction) / (len * len);
            let ext = self.extents();
            Some(DVec3::from_array(std::array::from_fn(|i| {
                if (uniform && ext[i] > FLAT) || (!uniform && grip.mask[i]) { f } else { 1. }
            })))
        } else {
            let gl = self.to_local(g.as_dvec3()).as_vec3() - self.to_local(anchor).as_vec3();
            let pl = self.to_local(point).as_vec3() - self.to_local(anchor).as_vec3();
            let mut factors = DVec3::ONE;
            for i in grip.axes() {
                if gl[i].abs() < 1e-12 { return None; }
                factors[i] = pl[i] as f64 / gl[i] as f64;
            }
            Some(factors)
        }
    }
    pub fn factors_from_ray(
        &self, grip: Grip, anchor: DVec3, ray: &Ray, uniform: bool,
    ) -> Option<DVec3> {
        if !ray.origin.is_finite() || !ray.dir.is_finite() { return None; }
        let a = anchor.as_vec3();
        let g = self.position(grip).as_vec3();
        let o = ray.origin.as_vec3();
        let d = ray.dir.as_vec3();
        let uniform = self.uniform(grip, uniform);
        if uniform || grip.axes().count() == 1 {
            let axis = g - a;
            let len = length(axis);
            if len < 1e-12 { return None; }
            let u = Vec3::new(axis.x / len as f32, axis.y / len as f32, axis.z / len as f32);
            let b = dot(d, u);
            let denominator = 1. - b * b;
            if denominator.abs() < 1e-9 { return None; }
            let w = o - a;
            let s = (dot(u, w) - b * dot(d, w)) / denominator;
            let p = a + u * s as f32;
            let f = dot(p - a, u) / len;
            let ext = self.extents();
            return Some(DVec3::from_array(std::array::from_fn(|i| {
                if (uniform && ext[i] > FLAT) || (!uniform && grip.mask[i]) { f } else { 1. }
            })));
        }
        let normal_axis = (0..3).find(|&i| !grip.mask[i])?;
        let n = self.frame.map(|f| f.axes[normal_axis]).unwrap_or([
            DVec3::X, DVec3::Y, DVec3::Z,
        ][normal_axis]).as_vec3();
        let denominator = dot(n, d);
        if denominator.abs() < 1e-9 { return None; }
        let t = dot(n, g - o) / denominator;
        if t < 0. { return None; }
        let point = o + d * t as f32;
        self.factors_from_point(grip, anchor, point.as_dvec3(), false)
    }
    /// Values are already parsed: plain values are factors, explicit units are metres.
    pub fn typed_factors(
        &self, grip: Grip, uniform: bool, values: &[f64], absolute: bool,
    ) -> Option<DVec3> {
        if values.is_empty() || values.iter().any(|v| !v.is_finite() || v.abs() < MIN_FACTOR) {
            return None;
        }
        let uniform = self.uniform(grip, uniform);
        let ext = self.extents();
        let axes = (0..3).filter(|&i| if uniform { ext[i] > FLAT } else { grip.mask[i] })
            .collect::<Vec<_>>();
        if values.len() != 1 && values.len() != axes.len() { return None; }
        let mut factors = DVec3::ONE;
        if values.len() == 1 {
            let f = if absolute { values[0] / ext[axes[0]] } else { values[0] };
            for i in axes { factors[i] = f; }
        } else {
            for (i, value) in axes.into_iter().zip(values.iter()) {
                factors[i] = if absolute { value / ext[i] } else { *value };
            }
        }
        factors.is_finite().then_some(factors)
    }
    pub fn segments(&self) -> Vec<(DVec3, DVec3)> {
        let ext = self.extents();
        let active = (0..3).filter(|&i| ext[i] > FLAT).collect::<Vec<_>>();
        let mut lines = Vec::with_capacity(12);
        if active.len() == 1 {
            let mut a = [0.5; 3];
            let mut b = a;
            a[active[0]] = 0.;
            b[active[0]] = 1.;
            lines.push((self.position(Grip { params:a, mask:[false;3] }),
                self.position(Grip { params:b, mask:[false;3] })));
        } else {
            for axis in active {
                let others = (0..3).filter(|&i| i != axis).collect::<Vec<_>>();
                for a in [0., 1.] {
                    for b in [0., 1.] {
                        if (ext[others[0]] <= FLAT && a != 0.)
                            || (ext[others[1]] <= FLAT && b != 0.) { continue; }
                        let mut low = [0.; 3];
                        low[others[0]] = a;
                        low[others[1]] = b;
                        let mut high = low;
                        high[axis] = 1.;
                        lines.push((self.position(Grip { params:low, mask:[false;3] }),
                            self.position(Grip { params:high, mask:[false;3] })));
                    }
                }
            }
        }
        lines
    }
}
