// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Orbital camera for the 3D viewport.
//!
//! Z-up convention: X (East), Y (North), Z (Up).
//! Spherical coordinates (target, distance, yaw, pitch).
//! Supports perspective, orthographic (parallel), and two-point perspective.

use std::f64::consts::PI;
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};

use crate::math::Ray;

pub const MIN_DISTANCE: f64 = 0.02;
pub const MAX_DISTANCE: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StandardView {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    Iso,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrbitCamera {
    pub target: DVec3,
    pub distance: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub up: DVec3,
    pub fov_deg: f64,
    pub aspect: f64,
    pub znear: f64,
    pub zfar: f64,
    pub perspective: bool,
    pub two_point: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}

impl OrbitCamera {
    pub fn new() -> Self {
        Self {
            target: DVec3::ZERO,
            distance: 20.0,
            yaw: (-45.0_f64).to_radians(),
            pitch: 30.0_f64.to_radians(),
            up: DVec3::Z,
            fov_deg: 45.0,
            aspect: 1.0,
            znear: 0.1,
            zfar: 10_000.0,
            perspective: true,
            two_point: false,
        }
    }

    /// Eye position in world space.
    pub fn eye(&self) -> DVec3 {
        let cp = self.pitch.cos();
        let sp = self.pitch.sin();
        let cy = self.yaw.cos();
        let sy = self.yaw.sin();
        self.target + DVec3::new(
            self.distance * cp * cy,
            self.distance * cp * sy,
            self.distance * sp,
        )
    }

    /// Unit view direction eye -> target.
    pub fn forward(&self) -> DVec3 {
        let d = self.target - self.eye();
        if d.length_squared() > 1e-18 {
            d.normalize()
        } else {
            DVec3::Y
        }
    }

    /// Right vector perpendicular to view and up.
    pub fn right(&self) -> DVec3 {
        self.forward().cross(self.up_vector()).normalize_or_zero()
    }

    /// Effective up vector (handling exact vertical poles for top/bottom views).
    pub fn up_vector(&self) -> DVec3 {
        let f = self.forward();
        if f.dot(self.up).abs() < 0.9999 {
            return self.up;
        }
        let sign = if self.pitch > 0.0 { -1.0 } else { 1.0 };
        DVec3::new(sign * self.yaw.cos(), sign * self.yaw.sin(), 0.0)
    }

    /// Orbit around target.
    pub fn orbit(&mut self, delta_yaw: f64, delta_pitch: f64) {
        self.yaw = (self.yaw + delta_yaw) % (2.0 * PI);
        let max_pitch = 89.0_f64.to_radians();
        let min_pitch = -89.0_f64.to_radians();
        self.pitch = (self.pitch + delta_pitch).clamp(min_pitch, max_pitch);
    }

    /// Pan camera laterally.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let right = self.right();
        let up = self.up_vector();
        let pan_scale = self.distance * 0.002;
        self.target -= right * (dx * pan_scale) - up * (dy * pan_scale);
    }

    /// Zoom in / out with factor (> 1.0 zooms in, < 1.0 zooms out).
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance / factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Set standard orthogonal or perspective view.
    pub fn set_standard_view(&mut self, view: StandardView) {
        match view {
            StandardView::Top => {
                self.yaw = (-90.0_f64).to_radians();
                self.pitch = 89.9_f64.to_radians();
            }
            StandardView::Bottom => {
                self.yaw = (-90.0_f64).to_radians();
                self.pitch = -89.9_f64.to_radians();
            }
            StandardView::Front => {
                self.yaw = (-90.0_f64).to_radians();
                self.pitch = 0.0;
            }
            StandardView::Back => {
                self.yaw = 90.0_f64.to_radians();
                self.pitch = 0.0;
            }
            StandardView::Left => {
                self.yaw = 180.0_f64.to_radians();
                self.pitch = 0.0;
            }
            StandardView::Right => {
                self.yaw = 0.0;
                self.pitch = 0.0;
            }
            StandardView::Iso => {
                self.yaw = (-45.0_f64).to_radians();
                self.pitch = 30.0_f64.to_radians();
            }
        }
    }

    /// Zoom Extents: frame the bounding box as the camera looks at it now.
    pub fn fit_box(&mut self, min_pt: DVec3, max_pt: DVec3, margin: f64) {
        let f = self.forward();
        let r = self.right();
        let u = self.up_vector();

        let xs = [min_pt.x, max_pt.x];
        let ys = [min_pt.y, max_pt.y];
        let zs = [min_pt.z, max_pt.z];

        let mut corners = Vec::with_capacity(8);
        for &x in &xs {
            for &y in &ys {
                for &z in &zs {
                    corners.push(DVec3::new(x, y, z));
                }
            }
        }

        let center = (min_pt + max_pt) * 0.5;
        let mut min_pr = f64::INFINITY;
        let mut max_pr = f64::NEG_INFINITY;
        let mut min_pu = f64::INFINITY;
        let mut max_pu = f64::NEG_INFINITY;

        for &c in &corners {
            let offset = c - center;
            let pr = offset.dot(r);
            let pu = offset.dot(u);
            min_pr = min_pr.min(pr);
            max_pr = max_pr.max(pr);
            min_pu = min_pu.min(pu);
            max_pu = max_pu.max(pu);
        }

        let target = center + r * ((max_pr + min_pr) * 0.5) + u * ((max_pu + min_pu) * 0.5);
        let half_w = (max_pr - min_pr) * 0.5;
        let half_h = (max_pu - min_pu) * 0.5;

        let t = (self.fov_deg.to_radians() * 0.5).tan();
        let aspect = if self.aspect > 1e-6 { self.aspect } else { 1.0 };
        let half = half_h.max(half_w / aspect).max(0.1) * margin;
        let mut distance = half / t;

        if self.perspective {
            let mut min_pf = f64::INFINITY;
            for &c in &corners {
                let pf = (c - target).dot(f);
                min_pf = min_pf.min(pf);
            }
            if min_pf.is_finite() {
                distance -= min_pf;
            }
        }

        self.target = target;
        self.distance = distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// View transformation matrix (World -> Camera).
    pub fn view_matrix(&self) -> DMat4 {
        DMat4::look_at_rh(self.eye(), self.target, self.up_vector())
    }

    /// Projection transformation matrix (Camera -> Clip).
    pub fn projection_matrix(&self) -> DMat4 {
        if self.perspective {
            DMat4::perspective_rh(
                self.fov_deg.to_radians(),
                self.aspect,
                self.znear,
                self.zfar,
            )
        } else {
            let half_h = self.distance * (self.fov_deg.to_radians() * 0.5).tan();
            let half_w = half_h * self.aspect;
            DMat4::orthographic_rh(
                -half_w,
                half_w,
                -half_h,
                half_h,
                self.znear,
                self.zfar,
            )
        }
    }

    /// Construct a 3D ray through screen coordinates (x, y) given viewport dimensions.
    pub fn screen_to_ray(&self, screen_x: f64, screen_y: f64, vp_width: f64, vp_height: f64) -> Ray {
        // Convert screen coordinates to NDC [-1, 1]
        let ndc_x = (2.0 * screen_x / vp_width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / vp_height);

        let inv_vp = (self.projection_matrix() * self.view_matrix()).inverse();
        let near_ndc = glam::DVec4::new(ndc_x, ndc_y, 0.0, 1.0);
        let far_ndc = glam::DVec4::new(ndc_x, ndc_y, 1.0, 1.0);

        let near_world = inv_vp * near_ndc;
        let far_world = inv_vp * far_ndc;

        let near_pt = near_world.truncate() / near_world.w;
        let far_pt = far_world.truncate() / far_world.w;

        Ray::new(near_pt, (far_pt - near_pt).normalize_or_zero())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_orbit_and_view() {
        let mut cam = OrbitCamera::new();
        cam.set_standard_view(StandardView::Front);
        assert!((cam.pitch - 0.0).abs() < 1e-4);
        let eye = cam.eye();
        assert!((eye.x - 0.0).abs() < 1e-3);
        assert!(eye.y < 0.0); // Looking from -Y to 0
    }

    #[test]
    fn test_compare_lookat() {
        let eye = DVec3::new(12.247449, -12.247449, 10.0);
        let target = DVec3::ZERO;
        let up = DVec3::Z;
        let m = DMat4::look_at_rh(eye, target, up);
        println!("=== GLAM DMat4::look_at_rh ===");
        for row in 0..4 {
            let r = m.row(row);
            println!("[{:.4}, {:.4}, {:.4}, {:.4}]", r.x, r.y, r.z, r.w);
        }
    }
}
