// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::math::plane_basis;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Rectangle tool for drawing 4-edge planar rectangles and faces.
#[derive(Debug, Clone)]
pub struct RectangleTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub from_center: bool,
}

impl Default for RectangleTool {
    fn default() -> Self {
        Self {
            start_point: None,
            hover_point: None,
            plane_normal: DVec3::Z, // Default XY ground plane
            from_center: false,
        }
    }
}

impl RectangleTool {
    pub fn new() -> Self {
        Self::default()
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
        self.from_center = false;
    }

    /// Calculate the 4 corners from start point and an opposite point.
    pub fn compute_corners(&self, start: DVec3, far: DVec3) -> Option<[DVec3; 4]> {
        let (u, v) = plane_basis(self.plane_normal);
        let delta = far - start;
        let du = delta.dot(u);
        let dv = delta.dot(v);

        if du.abs() < 1e-6 || dv.abs() < 1e-6 {
            return None;
        }

        if self.from_center {
            let half_u = u * (du.abs());
            let half_v = v * (dv.abs());
            Some([
                start - half_u - half_v,
                start + half_u - half_v,
                start + half_u + half_v,
                start - half_u + half_v,
            ])
        } else {
            Some([
                start,
                start + u * du,
                start + u * du + v * dv,
                start + v * dv,
            ])
        }
    }

    /// Preview the 4 corners during hover.
    pub fn preview_corners(&self) -> Option<[DVec3; 4]> {
        match (self.start_point, self.hover_point) {
            (Some(s), Some(h)) => self.compute_corners(s, h),
            _ => None,
        }
    }

    fn commit_corners(&mut self, corners: [DVec3; 4], scene: &mut Scene) {
        let v0 = scene.mesh.add_vertex(corners[0]);
        let v1 = scene.mesh.add_vertex(corners[1]);
        let v2 = scene.mesh.add_vertex(corners[2]);
        let v3 = scene.mesh.add_vertex(corners[3]);

        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
        scene.bump_version();
        self.reset();
    }
}

impl Tool for RectangleTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Rectangle
    }

    fn name(&self) -> &'static str {
        "Rectangle"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("R")
    }

    fn description(&self) -> &'static str {
        "Draw a rectangle from two opposite corners."
    }

    fn vcb_label(&self) -> &'static str {
        "Dimensions"
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
        if ctx.ctrl_held {
            self.from_center = true;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.from_center = ctx.ctrl_held;
        if self.start_point.is_none() {
            self.start_point = Some(ctx.world);
        } else {
            let start = self.start_point.unwrap();
            if let Some(corners) = self.compute_corners(start, ctx.world) {
                self.commit_corners(corners, scene);
            }
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let Some(start) = self.start_point else {
            return Err("No active start point for rectangle dimensions".into());
        };

        let trimmed = text.trim();
        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        if parts.len() != 2 {
            return Err("Rectangle dimensions format: 'width, height'".into());
        }

        let w: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid width: {e}"))?;
        let h: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid height: {e}"))?;

        if w.abs() < 1e-6 || h.abs() < 1e-6 {
            return Err("Dimensions must be non-zero".into());
        }

        let (u, v) = plane_basis(self.plane_normal);
        let (sign_u, sign_v) = if let Some(hover) = self.hover_point {
            let d = hover - start;
            (
                if d.dot(u) < 0.0 { -1.0 } else { 1.0 },
                if d.dot(v) < 0.0 { -1.0 } else { 1.0 },
            )
        } else {
            (1.0, 1.0)
        };

        let far = start + u * (w * sign_u) + v * (h * sign_v);
        if let Some(corners) = self.compute_corners(start, far) {
            self.commit_corners(corners, scene);
            Ok(())
        } else {
            Err("Failed to generate rectangle corners".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_rectangle_tool_two_clicks() {
        let mut tool = RectangleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx_hover = ToolContext::new(DVec3::new(3.0, 4.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);
        assert!(tool.preview_corners().is_some());

        let ctx1 = ToolContext::new(DVec3::new(3.0, 4.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 4);
        assert_eq!(scene.mesh.edges.len(), 4);
        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 12.0).abs() < 1e-4);
        assert!(tool.start_point.is_none());
    }

    #[test]
    fn test_rectangle_tool_vcb_input() {
        let mut tool = RectangleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx_hover = ToolContext::new(DVec3::new(2.0, 2.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);

        assert!(tool.on_vcb_input(&mut scene, "5.0, 2.0").is_ok());
        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_rectangle_tool_xz_plane() {
        let mut tool = RectangleTool::with_normal(DVec3::Y);
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Y));
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 0.0, 3.0), Ray::new(DVec3::ZERO, DVec3::Y));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 6.0).abs() < 1e-4);
    }
}
