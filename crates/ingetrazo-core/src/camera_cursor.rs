// SPDX-License-Identifier: GPL-3.0-or-later
//! Source cursor-pinned camera operations. Qt vector/quaternion boundaries are
//! binary32; the authored orbit angles and distance remain binary64.
use crate::camera::{OrbitCamera, MAX_DISTANCE, MIN_DISTANCE};
use glam::{DVec3, Vec3};
fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        0.
    } else {
        let s = v / m;
        m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
    }
}
fn eye(c: &OrbitCamera) -> Vec3 {
    c.target.as_vec3()
        + Vec3::new(
            (c.distance * c.pitch.cos() * c.yaw.cos()) as f32,
            (c.distance * c.pitch.cos() * c.yaw.sin()) as f32,
            (c.distance * c.pitch.sin()) as f32,
        )
}
fn normal(v: Vec3) -> Vec3 {
    let n = length(v);
    if (n - 1.).abs() <= 1e-5 {
        v
    } else if n.abs() <= 1e-5 {
        Vec3::ZERO
    } else {
        v / n
    }
}
fn mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [w, x, y, z] = a;
    let [v, i, j, k] = b;
    let yy = (w - y) * (v + k);
    let zz = (w + y) * (v - k);
    let ww = (z + x) * (i + j);
    let xx = ww + yy + zz;
    let qq = 0.5 * (xx + (z - x) * (i - j));
    [
        qq - ww + (z - y) * (j - k),
        qq - xx + (x + w) * (i + v),
        qq - yy + (w - x) * (j + k),
        qq - zz + (z + y) * (v - i),
    ]
}
fn axis_angle(axis: Vec3, degrees: f64) -> [f32; 4] {
    let half = (degrees as f32) / 2.;
    let angle = half * (std::f32::consts::PI / 180.);
    let s = angle.sin();
    let c = angle.cos();
    let axis = normal(axis);
    let q = [c, axis.x * s, axis.y * s, axis.z * s];
    // Qt QHypotHelper accumulates x,y,z,w while rescaling, not a max-first sum.
    let mut scale = q[1].abs();
    let mut total = 1f32;
    for next in [q[2], q[3], q[0]] {
        let val = next.abs();
        if !(scale > 0.) {
            scale = val;
            total = 1.;
        } else if val > 0. {
            if val > scale {
                let ratio = scale / next;
                total = total * ratio * ratio + 1.;
                scale = val;
            } else {
                let ratio = next / scale;
                total += ratio * ratio;
            }
        }
    }
    let n = if scale > 0. { scale * total.sqrt() } else { 0. };
    if n.abs() <= 1e-5 {
        [0.; 4]
    } else {
        q.map(|v| v / n)
    }
}
impl OrbitCamera {
    /// Pixel-law orbit with one acquired world pivot. Refusal is nonmutating.
    pub fn orbit_about(&mut self, pivot: DVec3, dx: f64, dy: f64, height: f64) -> bool {
        if !pivot.is_finite()
            || !pivot.as_vec3().is_finite()
            || !self.target.is_finite()
            || !self.target.as_vec3().is_finite()
            || ![dx, dy, height, self.yaw, self.pitch]
                .iter()
                .all(|v| v.is_finite())
        {
            return false;
        }
        let scale = std::f64::consts::PI / height.max(1.);
        let yaw = self.yaw - dx * scale;
        let pitch = (self.pitch + dy * scale).clamp(-89f64.to_radians(), 89f64.to_radians());
        let da = yaw - self.yaw;
        let dp = pitch - self.pitch;
        if da.abs() < 1e-12 && dp.abs() < 1e-12 {
            return true;
        }
        let ry = axis_angle(Vec3::Z, da.to_degrees());
        let cp = self.pitch.cos();
        let offset = Vec3::new(
            (cp * yaw.cos()) as f32,
            (cp * yaw.sin()) as f32,
            self.pitch.sin() as f32,
        );
        let axis = offset.cross(Vec3::Z);
        let rot = if length(axis) < 1e-9 {
            ry
        } else {
            mul(axis_angle(normal(axis), dp.to_degrees()), ry)
        };
        let v = self.target.as_vec3() - pivot.as_vec3();
        let conjugate = [rot[0], -rot[1], -rot[2], -rot[3]];
        let r = mul(mul(rot, [0., v.x, v.y, v.z]), conjugate);
        let target = (pivot.as_vec3() + Vec3::new(r[1], r[2], r[3])).as_dvec3();
        if !target.is_finite() || !yaw.is_finite() || !pitch.is_finite() {
            return false;
        }
        self.yaw = yaw;
        self.pitch = pitch;
        self.target = target;
        true
    }
    /// Source wheel magnitude: 0.9^steps, with target and distance pinned to focus.
    pub fn zoom_to(&mut self, steps: f64, focus: DVec3, min_step: f64) -> bool {
        if !focus.is_finite()
            || !focus.as_vec3().is_finite()
            || !self.target.is_finite()
            || !self.target.as_vec3().is_finite()
            || ![steps, min_step, self.distance, self.yaw, self.pitch]
                .iter()
                .all(|v| v.is_finite())
        {
            return false;
        }
        let factor = 0.9f64.powf(steps);
        if !factor.is_finite() || !(factor as f32).is_finite() {
            return false;
        }
        let before = eye(self);
        if !before.is_finite() {
            return false;
        }
        let mut candidate = self.clone();
        candidate.target = (focus.as_vec3()
            + (self.target.as_vec3() - focus.as_vec3()) * (factor as f32))
            .as_dvec3();
        candidate.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        if !eye(&candidate).is_finite() {
            return false;
        }
        if steps < 0. && min_step > 0. {
            let eye = eye(&candidate);
            let moved = length(eye - before);
            if f64::from(moved) < min_step {
                let back = eye - candidate.target.as_vec3();
                if length(back) > 1e-9 {
                    candidate.target = (candidate.target.as_vec3()
                        + normal(back) * ((min_step - f64::from(moved)) as f32))
                        .as_dvec3();
                }
            }
        }
        if !candidate.target.is_finite() || !eye(&candidate).is_finite() {
            return false;
        }
        self.target = candidate.target;
        self.distance = candidate.distance;
        true
    }
    pub fn pan_pixels(&mut self, dx: f64, dy: f64, height: f64, depth: Option<f64>) -> bool {
        if ![
            dx,
            dy,
            height,
            self.distance,
            self.yaw,
            self.pitch,
            self.fov_deg,
        ]
        .iter()
        .all(|v| v.is_finite())
            || !self.target.as_vec3().is_finite()
        {
            return false;
        }
        let forward = Vec3::new(
            (-self.pitch.cos() * self.yaw.cos()) as f32,
            (-self.pitch.cos() * self.yaw.sin()) as f32,
            (-self.pitch.sin()) as f32,
        );
        let right = normal(forward.cross(self.up_vector().as_vec3()));
        let up = normal(right.cross(forward));
        let span = if self.perspective {
            depth
                .filter(|v| v.is_finite() && *v > 1e-6)
                .unwrap_or(self.distance)
        } else {
            self.distance
        };
        let scale = 2. * span * (self.fov_deg.to_radians() / 2.).tan() / height.max(1.);
        let target =
            (self.target.as_vec3() - right * ((dx * scale) as f32)) + up * ((dy * scale) as f32);
        if !target.is_finite() {
            return false;
        }
        self.target = target.as_dvec3();
        true
    }
    pub fn zoom_steps(&mut self, steps: f64) -> bool {
        if !steps.is_finite() || !self.distance.is_finite() {
            return false;
        }
        let factor = 0.9f64.powf(steps);
        if !factor.is_finite() {
            return false;
        }
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refused_camera_operations_preserve_entire_camera() {
        let mut c = OrbitCamera::new();
        let old = c.clone();
        assert!(!c.orbit_about(DVec3::splat(f64::INFINITY), 1., 2., 600.));
        assert_eq!(c, old);
        assert!(!c.orbit_about(DVec3::ZERO, f64::MAX, 0., 1.));
        assert_eq!(c, old);
        assert!(!c.zoom_to(1., DVec3::splat(1e300), 0.05));
        assert_eq!(c, old);
        assert!(!c.zoom_to(f64::NEG_INFINITY, DVec3::ZERO, 0.05));
        assert_eq!(c, old);
        assert!(!c.pan_pixels(f64::MAX, 0., 600., None));
        assert_eq!(c, old);
    }
    #[test]
    fn closeup_retreat_and_wheel_magnitude_are_not_sign_only() {
        let mut a = OrbitCamera::new();
        a.distance = 0.02;
        let old = eye(&a);
        assert!(a.zoom_to(-1., DVec3::ZERO, 0.4));
        assert!(length(eye(&a) - old) >= 0.399999);
        let mut a = OrbitCamera::new();
        let mut b = a.clone();
        assert!(a.zoom_to(2., DVec3::X, 0.));
        assert!(b.zoom_to(1., DVec3::X, 0.));
        assert_ne!(a, b);
        assert_eq!(a.distance, 20. * 0.9f64.powf(2.));
    }
}
