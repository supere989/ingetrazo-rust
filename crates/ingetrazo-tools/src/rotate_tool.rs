// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::HashSet;
use std::f64::consts::PI;
use glam::{DMat4, DQuat, DVec3};
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{pick_edge, pick_face, pick_vertex, Tool, ToolContext, ToolType};

/// Rotate tool with 3-click protractor workflow around plane normal.
#[derive(Debug, Default, Clone)]
pub struct RotateTool {
    pub center: Option<DVec3>,
    pub ref_arm: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub is_copy_mode: bool,
    pub target_vertices: Vec<usize>,
    pub target_groups: Vec<usize>,
    pub last_angle_rad: Option<f64>,
}

impl RotateTool {
    pub fn new() -> Self {
        Self {
            plane_normal: DVec3::Z,
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
        self.center = None;
        self.ref_arm = None;
        self.hover_point = None;
        self.is_copy_mode = false;
        self.target_vertices.clear();
        self.target_groups.clear();
    }

    pub fn gather_targets(&mut self, scene: &Scene, ctx: &ToolContext) {
        let mut vert_set = HashSet::new();
        let mut group_set = HashSet::new();

        if !scene.selection.is_empty() {
            for item in &scene.selection {
                match item {
                    SelectionItem::Vertex(vid) => {
                        vert_set.insert(*vid);
                    }
                    SelectionItem::Edge(eid) => {
                        if *eid < scene.mesh.edges.len() {
                            let e = &scene.mesh.edges[*eid];
                            vert_set.insert(e.v0);
                            vert_set.insert(e.v1);
                        }
                    }
                    SelectionItem::Face(fid) => {
                        if *fid < scene.mesh.faces.len() {
                            for &v in &scene.mesh.faces[*fid].vertices {
                                vert_set.insert(v);
                            }
                        }
                    }
                    SelectionItem::Group(gid) => {
                        group_set.insert(*gid);
                    }
                    SelectionItem::GeoPoint(_) | SelectionItem::GeoPath(_) => {}
                }
            }
        } else if let Some(vid) = pick_vertex(&scene.mesh, &ctx.ray, 0.2) {
            vert_set.insert(vid);
        } else if let Some(eid) = pick_edge(&scene.mesh, &ctx.ray, 0.2) {
            let e = &scene.mesh.edges[eid];
            vert_set.insert(e.v0);
            vert_set.insert(e.v1);
        } else if let Some((fid, _, _)) = pick_face(&scene.mesh, &ctx.ray) {
            for &v in &scene.mesh.faces[fid].vertices {
                vert_set.insert(v);
            }
        }

        self.target_vertices = vert_set.into_iter().collect();
        self.target_groups = group_set.into_iter().collect();
    }

    /// Calculate signed angle in radians around normal between ref_arm and current arm.
    pub fn compute_angle(&self, arm_point: DVec3) -> f64 {
        let (Some(c), Some(r)) = (self.center, self.ref_arm) else {
            return 0.0;
        };

        let v1 = (r - c).normalize_or_zero();
        let v2 = (arm_point - c).normalize_or_zero();

        let dot = v1.dot(v2).clamp(-1.0, 1.0);
        let cross = v1.cross(v2);
        let sign = if cross.dot(self.plane_normal) >= 0.0 {
            1.0
        } else {
            -1.0
        };

        sign * dot.acos()
    }

    pub fn execute_rotation(&mut self, angle_rad: f64, scene: &mut Scene) {
        let Some(center) = self.center else { return };
        let quat = DQuat::from_axis_angle(self.plane_normal, angle_rad);

        if self.is_copy_mode {
            for &vid in &self.target_vertices {
                let old_pos = scene.mesh.vertices[vid].position;
                let rel = old_pos - center;
                let new_pos = center + quat * rel;
                scene.mesh.add_vertex(new_pos);
            }
        } else {
            for &vid in &self.target_vertices {
                let old_pos = scene.mesh.vertices[vid].position;
                let rel = old_pos - center;
                scene.mesh.vertices[vid].position = center + quat * rel;
            }
            let rot_mat = DMat4::from_translation(center)
                * DMat4::from_quat(quat)
                * DMat4::from_translation(-center);
            for &gid in &self.target_groups {
                if gid < scene.groups.len() {
                    scene.groups[gid].transform = rot_mat * scene.groups[gid].transform;
                }
            }
        }

        self.last_angle_rad = Some(angle_rad);
        scene.bump_version();
    }
}

impl Tool for RotateTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Rotate
    }

    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("Q")
    }

    fn description(&self) -> &'static str {
        "Turn the selection around a centre with the protractor; Ctrl leaves a copy behind."
    }

    fn vcb_label(&self) -> &'static str {
        "Angle"
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
            self.is_copy_mode = true;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if ctx.ctrl_held {
            self.is_copy_mode = true;
        }

        if self.center.is_none() {
            self.gather_targets(scene, ctx);
            self.center = Some(ctx.world);
        } else if self.ref_arm.is_none() {
            self.ref_arm = Some(ctx.world);
        } else {
            let angle = self.compute_angle(ctx.world);
            self.execute_rotation(angle, scene);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let deg: f64 = text.trim().parse().map_err(|e| format!("Invalid angle in degrees: {e}"))?;
        let rad = deg * PI / 180.0;
        self.execute_rotation(rad, scene);
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_rotate_tool_90_degrees() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = RotateTool::new();
        // Click 1: center at origin
        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Click 2: ref arm along X
        let ctx1 = ToolContext::new(DVec3::new(1.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Click 3: target arm along Y (90 degrees)
        let ctx2 = ToolContext::new(DVec3::new(0.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx2);

        let final_pos = scene.mesh.vertices[v0].position;
        assert!((final_pos - DVec3::new(0.0, 1.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn test_rotate_tool_vcb_degrees() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = RotateTool::new();
        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Rotate 180 deg via VCB
        assert!(tool.on_vcb_input(&mut scene, "180.0").is_ok());

        let final_pos = scene.mesh.vertices[v0].position;
        assert!((final_pos - DVec3::new(-2.0, 0.0, 0.0)).length() < 1e-4);
    }
}
