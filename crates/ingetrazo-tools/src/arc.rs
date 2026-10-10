// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::f64::consts::PI;
use glam::DVec3;
use ingetrazo_core::math::plane_basis;
use ingetrazo_core::scene::Scene;
use ingetrazo_core::topology::find_smallest_cycle_through;

use crate::base::{Tool, ToolContext, ToolType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcMode {
    TwoPoint,
    ThreePoint,
}

/// Interactive Arc tool supporting 2-point (chord + bulge) and 3-point arcs.
#[derive(Debug, Clone)]
pub struct ArcTool {
    pub mode: ArcMode,
    pub start_point: Option<DVec3>,
    pub second_point: Option<DVec3>, // End point in 2-point mode; intermediate through-point in 3-point mode
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub segments: usize,
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
        }
    }
}

impl ArcTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn three_point() -> Self {
        Self {
            mode: ArcMode::ThreePoint,
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
        Self {
            plane_normal: normal.normalize_or_zero(),
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.second_point = None;
        self.hover_point = None;
    }

    /// Compute 2D circumcenter of 3 points (in local basis u, v).
    fn circumcenter_2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64)> {
        let (ax, ay) = a;
        let (bx, by) = b;
        let (cx, cy) = c;
        let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
        if d.abs() < 1e-12 {
            return None;
        }
        let a2 = ax * ax + ay * ay;
        let b2 = bx * bx + by * by;
        let c2 = cx * cx + cy * cy;
        let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
        let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
        Some((ux, uy))
    }

    fn wrap_angle(a: f64) -> f64 {
        let mut angle = a;
        while angle <= -PI {
            angle += 2.0 * PI;
        }
        while angle > PI {
            angle -= 2.0 * PI;
        }
        angle
    }

    /// Compute 3-point arc polyline vertices from start, intermediate, end.
    pub fn compute_3pt_arc(&self, p0: DVec3, p1: DVec3, p2: DVec3) -> Vec<DVec3> {
        let (u, v) = plane_basis(self.plane_normal);
        let to_2d = |p: DVec3| (p.dot(u), p.dot(v));

        let a = to_2d(p0);
        let m = to_2d(p1);
        let b = to_2d(p2);

        let Some((cx, cy)) = Self::circumcenter_2d(a, m, b) else {
            return vec![p0, p2];
        };

        let radius = (a.0 - cx).hypot(a.1 - cy);
        let a0 = (a.1 - cy).atan2(a.0 - cx);
        let a1 = (b.1 - cy).atan2(b.0 - cx);
        let am = (m.1 - cy).atan2(m.0 - cx);

        let mut delta = Self::wrap_angle(a1 - a0);
        let da = Self::wrap_angle(am - a0);

        if delta >= 0.0 && !(0.0..=delta).contains(&da) {
            delta -= 2.0 * PI;
        } else if delta < 0.0 && !(delta..=0.0).contains(&da) {
            delta += 2.0 * PI;
        }

        let n = self.segments.max(2);
        let mut out = Vec::with_capacity(n + 1);

        for k in 0..=n {
            let theta = a0 + delta * (k as f64 / n as f64);
            let px = cx + radius * theta.cos();
            let py = cy + radius * theta.sin();
            let z_avg = (p0.dot(self.plane_normal) + p2.dot(self.plane_normal)) * 0.5;
            out.push(u * px + v * py + self.plane_normal * z_avg);
        }

        out
    }

    /// Compute 2-point arc polyline from start, end, and bulge point.
    pub fn compute_2pt_arc(&self, start: DVec3, end: DVec3, bulge_pt: DVec3) -> Vec<DVec3> {
        let chord = end - start;
        let chord_len = chord.length();
        if chord_len < 1e-6 {
            return vec![start, end];
        }

        let mid = (start + end) * 0.5;
        let chord_dir = chord.normalize();
        let perp = self.plane_normal.cross(chord_dir).normalize_or_zero();

        let h = (bulge_pt - mid).dot(perp);
        if h.abs() < 1e-6 {
            return vec![start, end];
        }

        let intermediate = mid + perp * h;
        self.compute_3pt_arc(start, intermediate, end)
    }

    pub fn compute_2pt_arc_with_bulge(&self, start: DVec3, end: DVec3, h: f64) -> Vec<DVec3> {
        let chord = end - start;
        let chord_dir = chord.normalize_or_zero();
        let perp = self.plane_normal.cross(chord_dir).normalize_or_zero();
        let mid = (start + end) * 0.5;
        let intermediate = mid + perp * h;
        self.compute_3pt_arc(start, intermediate, end)
    }

    pub fn preview_points(&self) -> Option<Vec<DVec3>> {
        match (self.mode, self.start_point, self.second_point, self.hover_point) {
            (ArcMode::TwoPoint, Some(s), Some(e), Some(h)) => Some(self.compute_2pt_arc(s, e, h)),
            (ArcMode::ThreePoint, Some(s), Some(m), Some(e)) => Some(self.compute_3pt_arc(s, m, e)),
            _ => None,
        }
    }

    fn commit_arc_polyline(&mut self, pts: Vec<DVec3>, scene: &mut Scene) {
        if pts.len() < 2 {
            self.reset();
            return;
        }

        let first = scene.mesh.add_vertex(*pts.first().unwrap());
        let last = scene.mesh.add_vertex(*pts.last().unwrap());
        let existing_cycle = find_smallest_cycle_through(&scene.mesh, first, last, 64);

        let vert_ids: Vec<usize> = pts.into_iter().map(|p| scene.mesh.add_vertex(p)).collect();
        for i in 0..vert_ids.len() - 1 {
            let v0 = vert_ids[i];
            let v1 = vert_ids[i + 1];
            scene.mesh.add_edge(v0, v1);
        }

        if let Some(mut cycle) = existing_cycle {
            cycle.extend(vert_ids.iter().skip(1).copied());
            scene.mesh.add_face(cycle, vec![]);
        }

        scene.bump_version();
        self.reset();
    }
}

impl Tool for ArcTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Arc
    }

    fn name(&self) -> &'static str {
        match self.mode {
            ArcMode::TwoPoint => "2-Point Arc",
            ArcMode::ThreePoint => "3-Point Arc",
        }
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("A")
    }

    fn description(&self) -> &'static str {
        "Draw an arc through points or with a bulge."
    }

    fn vcb_label(&self) -> &'static str {
        "Bulge"
    }

    fn on_activate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.start_point.is_none() {
            self.start_point = Some(ctx.world);
        } else if self.second_point.is_none() {
            self.second_point = Some(ctx.world);
        } else {
            let p0 = self.start_point.unwrap();
            let p1 = self.second_point.unwrap();
            let pts = match self.mode {
                ArcMode::TwoPoint => self.compute_2pt_arc(p0, p1, ctx.world),
                ArcMode::ThreePoint => self.compute_3pt_arc(p0, p1, ctx.world),
            };
            self.commit_arc_polyline(pts, scene);
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.ends_with('s') || trimmed.ends_with('S') {
            let segs: usize = trimmed[..trimmed.len() - 1].trim().parse().map_err(|e| format!("Invalid segments: {e}"))?;
            self.segments = segs.max(2);
            return Ok(());
        }

        if let (Some(s), Some(e)) = (self.start_point, self.second_point) {
            let bulge: f64 = trimmed.parse().map_err(|e| format!("Invalid bulge: {e}"))?;
            let pts = self.compute_2pt_arc_with_bulge(s, e, bulge);
            self.commit_arc_polyline(pts, scene);
            Ok(())
        } else {
            Err("Arc requires start and end points before bulge input".into())
        }
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
