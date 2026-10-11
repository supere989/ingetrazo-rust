// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::f64::consts::PI;
use glam::DVec3;
use ingetrazo_core::math::plane_basis;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Circle tool for drawing regular N-gon circles and circular faces.
#[derive(Debug, Clone)]
pub struct CircleTool {
    pub start_point: Option<DVec3>,
    authored_draw_faces:Vec<usize>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub segments: usize,
}

impl Default for CircleTool {
    fn default() -> Self {
        Self {
            start_point: None,
            authored_draw_faces:Vec::new(),
            hover_point: None,
            plane_normal: DVec3::Z,
            segments: 24,
        }
    }
}

impl CircleTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_segments(segments: usize) -> Self {
        Self {
            segments: segments.max(3),
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
        self.hover_point = None;
    }

    pub fn compute_points(&self, center: DVec3, rim: DVec3) -> Option<Vec<DVec3>> {
        let (u, v) = plane_basis(self.plane_normal);
        let delta = rim - center;
        let du = delta.dot(u);
        let dv = delta.dot(v);
        let radius = (du * du + dv * dv).sqrt();

        if radius < 1e-6 {
            return None;
        }

        let a0 = dv.atan2(du);
        let n = self.segments.max(3);
        let mut pts = Vec::with_capacity(n);

        for k in 0..n {
            let angle = a0 + 2.0 * PI * (k as f64) / (n as f64);
            pts.push(center + (u * angle.cos() + v * angle.sin()) * radius);
        }

        Some(pts)
    }

    pub fn preview_points(&self) -> Option<Vec<DVec3>> {
        match (self.start_point, self.hover_point) {
            (Some(c), Some(r)) => self.compute_points(c, r),
            _ => None,
        }
    }

    fn commit_points(&mut self, pts: Vec<DVec3>, scene: &mut Scene) {
        let vert_ids: Vec<usize> = pts.into_iter().map(|p| scene.mesh.add_vertex(p)).collect();
        if let Some(id)=scene.mesh.add_face(vert_ids, vec![]){self.authored_draw_faces.push(id);}
        scene.bump_version();
        self.reset();
    }
}

impl Tool for CircleTool {
    fn take_authored_draw_faces(&mut self)->Vec<usize>{std::mem::take(&mut self.authored_draw_faces)}
    fn tool_type(&self) -> ToolType {
        ToolType::Circle
    }

    fn name(&self) -> &'static str {
        "Circle"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("C")
    }

    fn description(&self) -> &'static str {
        "Draw a circle from its centre and radius."
    }

    fn vcb_label(&self) -> &'static str {
        if self.start_point.is_some() {
            "Radius"
        } else {
            "Sides"
        }
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
        } else {
            let center = self.start_point.unwrap();
            if let Some(pts) = self.compute_points(center, ctx.world) {
                self.commit_points(pts, scene);
            }
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();

        // Check for segment count suffix e.g. "16s" or "16S"
        if trimmed.ends_with('s') || trimmed.ends_with('S') {
            let num_str = &trimmed[..trimmed.len() - 1];
            let segs: usize = num_str.trim().parse().map_err(|e| format!("Invalid segments: {e}"))?;
            if segs < 3 {
                return Err("Circle must have at least 3 segments".into());
            }
            self.segments = segs;
            return Ok(());
        }

        // Before center is placed, integer sets segment count
        if self.start_point.is_none() {
            if let Ok(value) = ingetrazo_core::units::vcb::parse_single(trimmed,1.) {
                let segs=value.round_ties_even() as usize;
                if segs >= 3 {
                    self.segments = segs;
                    return Ok(());
                }
            }
            return Err("Provide side count before placing centre point".into());
        }

        // Radius input
        let radius=ingetrazo_core::units::vcb::parse_single(trimmed,ingetrazo_core::units::model_scale(scene))?;
        if radius <= 0.0 {
            return Err("Radius must be positive".into());
        }

        let center = self.start_point.unwrap();
        let (u, _) = plane_basis(self.plane_normal);
        let rim = center + u * radius;

        if let Some(pts) = self.compute_points(center, rim) {
            self.commit_points(pts, scene);
            Ok(())
        } else {
            Err("Failed to generate circle".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_circle_tool_two_clicks() {
        let mut tool = CircleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx_hover = ToolContext::new(DVec3::new(2.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);
        assert_eq!(tool.preview_points().unwrap().len(), 24);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 24);
        assert_eq!(scene.mesh.edges.len(), 24);
        assert_eq!(scene.mesh.faces.len(), 1);
        // Area of regular 24-gon with radius 2: 0.5 * 24 * 2^2 * sin(2pi/24) ~ 12.423
        assert!((scene.mesh.face_area(0) - 12.423).abs() < 0.1);
        assert!(tool.start_point.is_none());
    }

    #[test]
    fn test_circle_tool_segments_and_vcb() {
        let mut tool = CircleTool::new();
        let mut scene = Scene::new();

        // Change segments via VCB
        assert!(tool.on_vcb_input(&mut scene, "8s").is_ok());
        assert_eq!(tool.segments, 8);

        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // VCB radius
        assert!(tool.on_vcb_input(&mut scene, "5.0").is_ok());
        assert_eq!(scene.mesh.vertices.len(), 8);
        assert_eq!(scene.mesh.faces.len(), 1);
    }
}
