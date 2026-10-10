// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::HashSet;
use glam::DVec3;
use ingetrazo_core::math::BoundingBox;
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Scale tool for scaling selected geometry via bounding box grips.
#[derive(Debug, Default, Clone)]
pub struct ScaleTool {
    pub bbox: Option<BoundingBox>,
    pub anchor: Option<DVec3>,
    pub grip: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub about_center: bool,
    pub target_vertices: Vec<usize>,
}

impl ScaleTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.bbox = None;
        self.anchor = None;
        self.grip = None;
        self.hover_point = None;
        self.about_center = false;
        self.target_vertices.clear();
    }

    pub fn gather_targets(&mut self, scene: &Scene, ctx: &ToolContext) {
        let mut vert_set = HashSet::new();

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
                    SelectionItem::Group(_) => {}
                    SelectionItem::GeoPoint(_) | SelectionItem::GeoPath(_) | SelectionItem::SectionPlane(_) => {}
                }
            }
        } else if let Some(vid) = crate::base::pick_scene_vertex(scene, &ctx.ray, 0.2) {
            vert_set.insert(vid);
        } else if let Some(eid) = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2) {
            let e = &scene.mesh.edges[eid];
            vert_set.insert(e.v0);
            vert_set.insert(e.v1);
        } else if let Some((fid, _, _)) = crate::base::pick_scene_face(scene, &ctx.ray) {
            for &v in &scene.mesh.faces[fid].vertices {
                vert_set.insert(v);
            }
        }

        self.target_vertices = vert_set.into_iter().collect();

        if !self.target_vertices.is_empty() {
            let positions: Vec<DVec3> = self
                .target_vertices
                .iter()
                .map(|&v| scene.mesh.vertices[v].position)
                .collect();
            self.bbox = Some(BoundingBox::from_points(&positions));
        }
    }

    pub fn execute_scale(&mut self, factors: DVec3, scene: &mut Scene) {
        let Some(bbox) = self.bbox else { return };

        let anchor = if self.about_center {
            bbox.center()
        } else {
            self.anchor.unwrap_or(bbox.min)
        };

        for &vid in &self.target_vertices {
            let p = scene.mesh.vertices[vid].position;
            let rel = p - anchor;
            scene.mesh.vertices[vid].position = anchor + rel * factors;
        }

        scene.bump_version();
    }
}

impl Tool for ScaleTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Scale
    }

    fn name(&self) -> &'static str {
        "Scale"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("S")
    }

    fn description(&self) -> &'static str {
        "Resize the selection by dragging the grips of its box; the corners keep its proportions."
    }

    fn vcb_label(&self) -> &'static str {
        "Scale"
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
            self.about_center = true;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if ctx.ctrl_held {
            self.about_center = true;
        }

        if self.grip.is_none() {
            self.gather_targets(scene, ctx);
            if let Some(bbox) = self.bbox {
                self.grip = Some(bbox.max);
                self.anchor = Some(bbox.min);
            }
        } else {
            let grip = self.grip.unwrap();
            let anchor = self.anchor.unwrap_or(DVec3::ZERO);
            let orig_diag = grip - anchor;
            let curr_diag = ctx.world - anchor;

            let fx = if orig_diag.x.abs() > 1e-6 { curr_diag.x / orig_diag.x } else { 1.0 };
            let fy = if orig_diag.y.abs() > 1e-6 { curr_diag.y / orig_diag.y } else { 1.0 };
            let fz = if orig_diag.z.abs() > 1e-6 { curr_diag.z / orig_diag.z } else { 1.0 };

            self.execute_scale(DVec3::new(fx, fy, fz), scene);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();
        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        let factors = if parts.len() == 3 {
            let fx: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid fx: {e}"))?;
            let fy: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid fy: {e}"))?;
            let fz: f64 = parts[2].trim().parse().map_err(|e| format!("Invalid fz: {e}"))?;
            DVec3::new(fx, fy, fz)
        } else {
            let f: f64 = trimmed.parse().map_err(|e| format!("Invalid factor: {e}"))?;
            DVec3::splat(f)
        };

        self.execute_scale(factors, scene);
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_scale_tool_uniform() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(2.0, 2.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));
        scene.select(SelectionItem::Vertex(v1));

        let mut tool = ScaleTool::new();
        let ctx0 = ToolContext::new(DVec3::new(2.0, 2.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Scale by 2.0 via VCB
        assert!(tool.on_vcb_input(&mut scene, "2.0").is_ok());

        assert_eq!(scene.mesh.vertices[v0].position, DVec3::ZERO);
        assert_eq!(scene.mesh.vertices[v1].position, DVec3::new(4.0, 4.0, 0.0));
    }
}
