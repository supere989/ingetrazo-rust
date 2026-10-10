// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_face, Tool, ToolContext, ToolType};

/// Interactive Offset tool for parallel offsets of face perimeters.
#[derive(Debug, Default, Clone)]
pub struct OffsetTool {
    pub hovered_face: Option<usize>,
    pub selected_face: Option<usize>,
    pub start_point: Option<DVec3>,
    pub offset_distance: f64,
    pub is_dragging: bool,
}

impl OffsetTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.hovered_face = None;
        self.selected_face = None;
        self.start_point = None;
        self.offset_distance = 0.0;
        self.is_dragging = false;
    }

    /// Parallel offset of a planar polygon loop by signed distance `dist` (positive = inward).
    pub fn offset_polygon(points: &[DVec3], normal: DVec3, dist: f64) -> Option<Vec<DVec3>> {
        let n = points.len();
        if n < 3 || dist.abs() < 1e-9 {
            return None;
        }

        // Edge directions and in-plane inward normals
        let mut edge_dirs = Vec::with_capacity(n);
        let mut edge_inwards = Vec::with_capacity(n);

        for i in 0..n {
            let next_i = (i + 1) % n;
            let dir = (points[next_i] - points[i]).normalize_or_zero();
            let inward = normal.cross(dir).normalize_or_zero();
            edge_dirs.push(dir);
            edge_inwards.push(inward);
        }

        let mut offset_pts = Vec::with_capacity(n);

        for i in 0..n {
            let prev_i = (i + n - 1) % n;
            // Shifted line 1: passes through p_prev_shifted with dir1
            let p1 = points[prev_i] + edge_inwards[prev_i] * dist;
            let d1 = edge_dirs[prev_i];

            // Shifted line 2: passes through p2 with dir2
            let p2 = points[i] + edge_inwards[i] * dist;
            let d2 = edge_dirs[i];

            // Intersect 2 lines in plane: p1 + s * d1 = p2 + t * d2
            let perp2 = d2.cross(normal);
            let denom = d1.dot(perp2);

            if denom.abs() < 1e-6 {
                offset_pts.push(points[i] + edge_inwards[i] * dist);
            } else {
                let s = (p2 - p1).dot(perp2) / denom;
                offset_pts.push(p1 + d1 * s);
            }
        }

        Some(offset_pts)
    }

    /// Apply offset loop to the selected face in `scene`.
    pub fn apply_offset(scene: &mut Scene, face_id: usize, dist: f64) -> Result<(), String> {
        if face_id >= scene.mesh.faces.len() {
            return Err("Invalid face ID".into());
        }

        let normal = scene.mesh.face_normal(face_id);
        let positions: Vec<DVec3> = scene.mesh.faces[face_id]
            .vertices
            .iter()
            .map(|&v| scene.mesh.vertices[v].position)
            .collect();

        let Some(offset_positions) = Self::offset_polygon(&positions, normal, dist) else {
            return Err("Failed to compute offset polygon".into());
        };

        let offset_verts: Vec<usize> = offset_positions
            .into_iter()
            .map(|p| scene.mesh.add_vertex(p))
            .collect();

        // Add inner face
        scene.mesh.add_face(offset_verts, vec![]);
        scene.bump_version();
        Ok(())
    }
}

impl Tool for OffsetTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Offset
    }

    fn name(&self) -> &'static str {
        "Offset"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("F")
    }

    fn description(&self) -> &'static str {
        "Draw a copy of a face's outline at an even distance inside or outside it."
    }

    fn vcb_label(&self) -> &'static str {
        "Offset"
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

    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !self.is_dragging {
            self.hovered_face = pick_face(&scene.mesh, &ctx.ray).map(|(id, _, _)| id);
        } else if let (Some(face_id), Some(start)) = (self.selected_face, self.start_point) {
            let in_plane_delta = ctx.world - start;
            let dist = in_plane_delta.length();
            let centroid = scene.mesh.face_centroid(face_id);
            let sign = if (ctx.world - centroid).length() < (start - centroid).length() {
                1.0
            } else {
                -1.0
            };
            self.offset_distance = dist * sign;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !self.is_dragging {
            let face_pick = self
                .hovered_face
                .or_else(|| pick_face(&scene.mesh, &ctx.ray).map(|(id, _, _)| id));

            if let Some(face_id) = face_pick {
                self.selected_face = Some(face_id);
                self.start_point = Some(ctx.world);
                self.is_dragging = true;
                self.offset_distance = 0.0;
            }
        } else if let Some(face_id) = self.selected_face {
            let dist = self.offset_distance;
            let _ = Self::apply_offset(scene, face_id, dist);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let dist: f64 = text.trim().parse().map_err(|e| format!("Invalid offset distance: {e}"))?;

        if let Some(face_id) = self.selected_face {
            Self::apply_offset(scene, face_id, dist)?;
            self.reset();
            Ok(())
        } else if let Some(face_id) = self.hovered_face {
            Self::apply_offset(scene, face_id, dist)?;
            self.reset();
            Ok(())
        } else {
            Err("No face selected for Offset".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_offset_tool_inward() {
        let mut scene = Scene::new();
        // 10x10 square
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(10.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(10.0, 10.0, 0.0));
        let v3 = scene.mesh.add_vertex(DVec3::new(0.0, 10.0, 0.0));
        let f0 = scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]).unwrap();

        let mut tool = OffsetTool::new();
        let ray = Ray::new(DVec3::new(5.0, 5.0, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(5.0, 5.0, 0.0), ray);

        tool.on_click(&mut scene, &ctx);
        assert_eq!(tool.selected_face, Some(f0));

        // Offset inward by 1.0 via VCB
        assert!(tool.on_vcb_input(&mut scene, "1.0").is_ok());

        // Should have added an 8x8 inner face
        assert_eq!(scene.mesh.faces.len(), 2);
        let inner_face_id = 1;
        let inner_area = scene.mesh.face_area(inner_face_id);
        assert!((inner_area - 64.0).abs() < 1e-4);
    }
}
