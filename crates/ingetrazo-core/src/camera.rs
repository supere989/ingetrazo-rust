// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Orbital camera for the 3D viewport.
//!
//! Z-up convention: X (East), Y (North), Z (Up).
//! Spherical coordinates (target, distance, yaw, pitch).
//! Supports perspective, orthographic (parallel), and two-point perspective.

use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

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
        self.target
            + DVec3::new(
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

    /// Place the eye and look direction using the existing orbit representation.
    /// Returns false without mutation for nonfinite input.
    pub fn look_from(&mut self, eye: DVec3, direction: DVec3) -> bool {
        if !eye.is_finite() || !direction.is_finite() || !self.distance.is_finite() { return false; }
        let d = if direction.length_squared() < 1e-18 { self.forward() } else { direction.normalize() };
        if !d.is_finite() { return false; }
        let pitch = (-d.z).clamp(-1.,1.).asin().clamp(-89_f64.to_radians(),89_f64.to_radians());
        let yaw = if pitch.cos().abs() > 1e-9 { (-d.y).atan2(-d.x) } else { self.yaw };
        let target = eye + d * self.distance;
        if !target.is_finite() { return false; }
        self.yaw = yaw; self.pitch = pitch; self.target = target; true
    }

    /// Turn the head clockwise and upward while retaining the eye.
    pub fn turn(&mut self, yaw_degrees: f64, pitch_degrees: f64) -> bool {
        if !yaw_degrees.is_finite() || !pitch_degrees.is_finite() { return false; }
        let eye = self.eye(); let d = self.forward();
        let yaw = d.y.atan2(d.x) - yaw_degrees.to_radians();
        let pitch = (d.z.clamp(-1.,1.).asin() + pitch_degrees.to_radians()).clamp(-89_f64.to_radians(),89_f64.to_radians());
        self.look_from(eye,DVec3::new(pitch.cos()*yaw.cos(),pitch.cos()*yaw.sin(),pitch.sin()))
    }

    /// Translate eye and target together, preserving the view direction.
    pub fn move_eye(&mut self, delta: DVec3) -> bool {
        let target = self.target + delta;
        if !delta.is_finite() || !target.is_finite() { return false; }
        self.target = target; true
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
                self.pitch = 90.0_f64.to_radians();
            }
            StandardView::Bottom => {
                self.yaw = (-90.0_f64).to_radians();
                self.pitch = -90.0_f64.to_radians();
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

    /// Original two-point projection levels the view unless the pitch is too steep.
    fn level_forward(&self) -> Option<DVec3> {
        if !self.two_point || !self.perspective {
            return None;
        }
        let f = self.forward();
        let h = DVec3::new(f.x, f.y, 0.);
        if h.length() < 0.2 {
            None
        } else {
            Some(h.normalize())
        }
    }
    /// View transformation matrix (World -> Camera).
    pub fn view_matrix(&self) -> DMat4 {
        if let Some(level) = self.level_forward() {
            let eye = self.eye();
            DMat4::look_at_rh(eye, eye + level, DVec3::Z)
        } else {
            DMat4::look_at_rh(self.eye(), self.target, self.up_vector())
        }
    }

    /// Projection transformation matrix (Camera -> Clip).
    pub fn projection_matrix(&self) -> DMat4 {
        if self.perspective {
            let near = self.znear.min((self.distance * 0.02).max(1e-4));
            if let Some(level) = self.level_forward() {
                let half_h = near * (self.fov_deg.to_radians() / 2.).tan();
                let half_w = half_h * self.aspect;
                let relative = self.target - self.eye();
                let depth = relative.dot(level);
                let shift = relative.z * near / depth;
                // Off-centre RH frustum, with glam's depth interval [0,1].
                DMat4::from_cols(
                    glam::DVec4::new(near / half_w, 0., 0., 0.),
                    glam::DVec4::new(0., near / half_h, 0., 0.),
                    glam::DVec4::new(0., shift / half_h, self.zfar / (near - self.zfar), -1.),
                    glam::DVec4::new(0., 0., near * self.zfar / (near - self.zfar), 0.),
                )
            } else {
                DMat4::perspective_rh(self.fov_deg.to_radians(), self.aspect, near, self.zfar)
            }
        } else {
            let half_h = self.distance * (self.fov_deg.to_radians() * 0.5).tan();
            let half_w = half_h * self.aspect;
            DMat4::orthographic_rh(-half_w, half_w, -half_h, half_h, -self.zfar, self.zfar)
        }
    }

    /// Construct a 3D ray through screen coordinates (x, y) given viewport dimensions.
    pub fn screen_to_ray(
        &self,
        screen_x: f64,
        screen_y: f64,
        vp_width: f64,
        vp_height: f64,
    ) -> Ray {
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

#[cfg(test)]
mod original_ortho_tests {
    use super::*;
    #[test]
    fn orthographic_depth_matches_original_symmetric_range_even_inside_model() {
        let mut camera = OrbitCamera::default();
        camera.perspective = false;
        camera.target = DVec3::ZERO;
        camera.distance = 0.2;
        camera.pitch = std::f64::consts::FRAC_PI_2;
        camera.up = DVec3::Y;
        let matrix = camera.projection_matrix() * camera.view_matrix();
        for point in [
            camera.eye() + camera.forward(),
            camera.eye() - camera.forward(),
        ] {
            let clip = matrix * point.extend(1.);
            assert!(
                (0. ..1.).contains(&(clip.z / clip.w)),
                "parallel sourceview includes geometry on either side of its eye"
            );
        }
        let near = matrix * (camera.eye() - camera.forward() * camera.zfar).extend(1.);
        let far = matrix * (camera.eye() + camera.forward() * camera.zfar).extend(1.);
        assert!((near.z / near.w).abs() < 1e-12);
        assert!((far.z / far.w - 1.).abs() < 1e-12);
    }
}

#[cfg(test)]
mod original_perspective_tests {
    use super::*;
    #[test]
    fn two_point_levels_verticals_centres_target_and_falls_back_when_steep() {
        let mut camera = OrbitCamera::default();
        camera.two_point = true;
        camera.pitch = 0.6;
        camera.yaw = -1.1;
        camera.target = DVec3::new(2., 3., 4.);
        let matrix = camera.projection_matrix() * camera.view_matrix();
        let target = matrix * camera.target.extend(1.);
        assert!((target.x / target.w).abs() < 1e-12);
        assert!((target.y / target.w).abs() < 1e-12);
        let p = camera.target + DVec3::X;
        let a = matrix * p.extend(1.);
        let b = matrix * (p + DVec3::Z * 3.).extend(1.);
        assert!((a.x / a.w - b.x / b.w).abs() < 1e-12);
        assert!((a.y / a.w - b.y / b.w).abs() > 0.1);
        camera.pitch = 1.5;
        let mut ordinary = camera.clone();
        ordinary.two_point = false;
        assert_eq!(camera.view_matrix(), ordinary.view_matrix());
        assert_eq!(camera.projection_matrix(), ordinary.projection_matrix());
    }
    #[test]
    fn perspective_near_plane_follows_close_eye_original_distance_rule() {
        let mut camera = OrbitCamera::default();
        camera.distance = 0.001;
        let near = 0.0001;
        let p = camera.eye() + camera.forward() * near;
        let matrix = camera.projection_matrix() * camera.view_matrix();
        let clip = matrix * p.extend(1.);
        assert!((clip.z / clip.w).abs() < 1e-10);
        let p = camera.eye() + camera.forward() * 0.01;
        let clip = matrix * p.extend(1.);
        assert!((0. ..1.).contains(&(clip.z / clip.w)));
    }
}

#[cfg(test)]mod straight_standard_view_tests {
 use super::*;
 #[test]fn top_bottom_collapse_vertical_edge_and_keep_finite_basis(){
  for view in [StandardView::Top,StandardView::Bottom]{let mut camera=OrbitCamera::default();camera.set_standard_view(view);camera.perspective=false;let m=camera.projection_matrix()*camera.view_matrix();let a=m*DVec3::new(0.3,0.9,-0.2).extend(1.);let b=m*DVec3::new(0.3,0.9,0.2).extend(1.);assert!((a.x-b.x).abs()<1e-12&&(a.y-b.y).abs()<1e-12,"Architectural plan projects a vertical edge as a line");assert!(m.is_finite());assert!((camera.pitch.abs()-std::f64::consts::FRAC_PI_2).abs()<1e-12);}
 }
}

#[cfg(test)]
mod walkthrough_camera_tests {
    use super::*;
    #[test] fn turn_and_translation_keep_the_orbit_representation() {
        let mut c=OrbitCamera::new(); let eye=c.eye(); let distance=c.distance;
        assert!(c.turn(35.,12.)); assert!(c.eye().distance(eye)<1e-12); assert_eq!(c.distance,distance);
        let direction=c.forward(); let eye=c.eye(); assert!(c.move_eye(DVec3::new(2.,3.,4.)));
        assert!(c.eye().distance(eye+DVec3::new(2.,3.,4.))<1e-12); assert!(c.forward().distance(direction)<1e-12);
    }
    #[test] fn refused_nonfinite_navigation_is_atomic() {
        let mut c=OrbitCamera::new(); let old=c.clone();
        assert!(!c.look_from(DVec3::splat(f64::NAN),DVec3::Y)); assert_eq!(c,old);
        assert!(!c.turn(f64::INFINITY,0.)); assert_eq!(c,old);
        assert!(!c.move_eye(DVec3::splat(f64::INFINITY))); assert_eq!(c,old);
    }
}
