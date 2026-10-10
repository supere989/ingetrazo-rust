// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_face, Tool, ToolContext, ToolType};

/// Interactive Push/Pull tool for extruding faces into solids and recessing geometry.
#[derive(Debug, Default, Clone)]
pub struct PushPullTool {
    pub hovered_face: Option<usize>,
    pub selected_face: Option<usize>,
    pub start_point: Option<DVec3>,
    pub extrusion_distance: f64,
    pub is_dragging: bool,
    pub last_distance: Option<f64>,
}

impl PushPullTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.hovered_face = None;
        self.selected_face = None;
        self.start_point = None;
        self.extrusion_distance = 0.0;
        self.is_dragging = false;
    }

    /// Perform extrusion of a face by signed distance along its normal.
    pub fn extrude_face(scene: &mut Scene, face_id: usize, distance: f64) -> Result<(), String> {
        if face_id >= scene.mesh.faces.len() {
            return Err("Invalid face ID".into());
        }
        if distance.abs() < 1e-6 {
            return Ok(()); // no-op
        }

        let normal = scene.mesh.face_normal(face_id);
        let trans = normal * distance;
        let base_verts = scene.mesh.faces[face_id].vertices.clone();
        let n = base_verts.len();
        if n < 3 {
            return Err("Face has fewer than 3 vertices".into());
        }

        // Add cap vertices
        let mut cap_verts = Vec::with_capacity(n);
        for &bv in &base_verts {
            let pos = scene.mesh.vertices[bv].position + trans;
            let cv = scene.mesh.add_vertex(pos);
            cap_verts.push(cv);
        }

        // Add side quad faces
        for i in 0..n {
            let next_i = (i + 1) % n;
            let v0 = base_verts[i];
            let v1 = base_verts[next_i];
            let c1 = cap_verts[next_i];
            let c0 = cap_verts[i];

            if distance > 0.0 {
                scene.mesh.add_face(vec![v0, v1, c1, c0], vec![]);
            } else {
                scene.mesh.add_face(vec![v0, c0, c1, v1], vec![]);
            }
        }

        // Add top cap face
        if distance > 0.0 {
            scene.mesh.add_face(cap_verts, vec![]);
        } else {
            let mut rev_cap = cap_verts;
            rev_cap.reverse();
            scene.mesh.add_face(rev_cap, vec![]);
        }

        // If the original base face was oriented upwards, reverse its vertex winding
        // so its normal points outward from the newly formed solid
        if distance > 0.0 {
            let base_face = &mut scene.mesh.faces[face_id];
            base_face.vertices.reverse();
        }

        scene.bump_version();
        Ok(())
    }
}

impl Tool for PushPullTool {
    fn tool_type(&self) -> ToolType {
        ToolType::PushPull
    }

    fn name(&self) -> &'static str {
        "Push / Pull"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("P")
    }

    fn description(&self) -> &'static str {
        "Push or pull a face to extrude it into a solid or cut it back."
    }

    fn vcb_label(&self) -> &'static str {
        "Distance"
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
            let normal = scene.mesh.face_normal(face_id);
            self.extrusion_distance = (ctx.world - start).dot(normal);
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
                self.extrusion_distance = 0.0;
            }
        } else if let Some(face_id) = self.selected_face {
            let dist = self.extrusion_distance;
            let _ = Self::extrude_face(scene, face_id, dist);
            self.last_distance = Some(dist);
            self.reset();
        }
    }

    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if let Some(last_d) = self.last_distance {
            let face_pick = self
                .hovered_face
                .or_else(|| pick_face(&scene.mesh, &ctx.ray).map(|(id, _, _)| id));

            if let Some(face_id) = face_pick {
                let _ = Self::extrude_face(scene, face_id, last_d);
                self.reset();
                return;
            }
        }
        self.on_click(scene, ctx);
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let dist: f64 = text.trim().parse().map_err(|e| format!("Invalid distance: {e}"))?;

        if let Some(face_id) = self.selected_face {
            Self::extrude_face(scene, face_id, dist)?;
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
        } else if let Some(face_id) = self.hovered_face {
            Self::extrude_face(scene, face_id, dist)?;
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
        } else {
            Err("No face selected for Push/Pull".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    fn make_test_square_scene() -> Scene {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
        scene
    }

    #[test]
    fn test_pushpull_extrude_cube() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        // Hover & click face 0
        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_move(&mut scene, &ctx0);
        assert_eq!(tool.hovered_face, Some(0));

        tool.on_click(&mut scene, &ctx0);
        assert!(tool.is_dragging);

        // Move up by 1.0
        let ctx1 = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);
        tool.on_move(&mut scene, &ctx1);
        assert!((tool.extrusion_distance - 1.0).abs() < 1e-4);

        // Commit click
        tool.on_click(&mut scene, &ctx1);
        assert!(!tool.is_dragging);
        assert_eq!(tool.last_distance, Some(1.0));

        // Result is a 6-sided solid cube!
        assert_eq!(scene.mesh.faces.len(), 6);
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_pushpull_vcb_and_repeat() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_click(&mut scene, &ctx0);

        // Extrude 2.5 via VCB
        assert!(tool.on_vcb_input(&mut scene, "2.5").is_ok());
        assert_eq!(tool.last_distance, Some(2.5));
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 2.5).abs() < 1e-3);
    }
}
