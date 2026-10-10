// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::HashSet;
use glam::{DMat4, DVec3};
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{pick_edge, pick_face, pick_vertex, Tool, ToolContext, ToolType};

/// Move and Copy tool supporting vector translations, Ctrl-copy, and linear array repeats.
#[derive(Debug, Default, Clone)]
pub struct MoveTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub is_copy_mode: bool,
    pub target_vertices: Vec<usize>,
    pub target_groups: Vec<usize>,
    pub last_targets: Vec<usize>,
    pub last_delta: Option<DVec3>,
    pub last_moved_verts: Vec<usize>,
}

impl MoveTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.hover_point = None;
        self.is_copy_mode = false;
        self.target_vertices.clear();
        self.target_groups.clear();
    }

    /// Resolve targets from selection or mouse click.
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
        } else {
            // Pick under cursor
            if let Some(vid) = pick_vertex(&scene.mesh, &ctx.ray, 0.2) {
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
        }

        self.target_vertices = vert_set.into_iter().collect();
        self.target_groups = group_set.into_iter().collect();
    }

    /// Execute translation or copy.
    pub fn execute_move(&mut self, delta: DVec3, scene: &mut Scene) {
        if delta.length_squared() < 1e-12 {
            return;
        }

        if self.is_copy_mode {
            // Copy mode: clone target vertices
            let mut new_verts = Vec::with_capacity(self.target_vertices.len());
            for &vid in &self.target_vertices {
                let old_pos = scene.mesh.vertices[vid].position;
                let new_vid = scene.mesh.add_vertex(old_pos + delta);
                new_verts.push(new_vid);
            }
            self.last_moved_verts = new_verts;
        } else {
            // Translate original vertices
            for &vid in &self.target_vertices {
                scene.mesh.vertices[vid].position += delta;
            }
            for &gid in &self.target_groups {
                if gid < scene.groups.len() {
                    scene.groups[gid].transform =
                        DMat4::from_translation(delta) * scene.groups[gid].transform;
                }
            }
            self.last_moved_verts = self.target_vertices.clone();
        }

        self.last_targets = self.target_vertices.clone();
        self.last_delta = Some(delta);
        scene.bump_version();
    }
}

impl Tool for MoveTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Move
    }

    fn name(&self) -> &'static str {
        "Move"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("M")
    }

    fn description(&self) -> &'static str {
        "Move the selection, or what you click, to a new place; Ctrl leaves a copy behind."
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

        if self.start_point.is_none() {
            self.gather_targets(scene, ctx);
            if !self.target_vertices.is_empty() || !self.target_groups.is_empty() {
                self.start_point = Some(ctx.world);
            }
        } else {
            let start = self.start_point.unwrap();
            let delta = ctx.world - start;
            self.execute_move(delta, scene);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();

        // 1. Array repeat: e.g. "5x", "*5", "/5", "5/"
        if (trimmed.ends_with('x') || trimmed.ends_with('X') || trimmed.starts_with('*'))
            && self.last_delta.is_some()
        {
            let num_str = trimmed.trim_matches(|c| c == 'x' || c == 'X' || c == '*');
            let count: usize = num_str.trim().parse().map_err(|e| format!("Invalid count: {e}"))?;
            let delta = self.last_delta.unwrap();

            // Create (count - 1) additional copies along delta
            for k in 2..=count {
                for &vid in &self.last_targets {
                    let old_pos = scene.mesh.vertices[vid].position;
                    scene.mesh.add_vertex(old_pos + delta * (k as f64));
                }
            }
            scene.bump_version();
            return Ok(());
        }

        if (trimmed.starts_with('/') || trimmed.ends_with('/')) && self.last_delta.is_some() {
            let num_str = trimmed.trim_matches('/');
            let count: usize = num_str.trim().parse().map_err(|e| format!("Invalid count: {e}"))?;
            if count == 0 {
                return Err("Count must be greater than zero".into());
            }
            let delta = self.last_delta.unwrap();

            // Internal division copies
            for k in 1..count {
                let frac = k as f64 / count as f64;
                for &vid in &self.last_targets {
                    let old_pos = scene.mesh.vertices[vid].position;
                    scene.mesh.add_vertex(old_pos + delta * frac);
                }
            }
            scene.bump_version();
            return Ok(());
        }

        // 2. 3D delta or single distance during active move
        let Some(start) = self.start_point else {
            return Err("No active move drag in progress".into());
        };

        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        let delta = if parts.len() == 3 {
            let dx: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid dx: {e}"))?;
            let dy: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid dy: {e}"))?;
            let dz: f64 = parts[2].trim().parse().map_err(|e| format!("Invalid dz: {e}"))?;
            DVec3::new(dx, dy, dz)
        } else {
            let dist: f64 = trimmed.parse().map_err(|e| format!("Invalid distance: {e}"))?;
            let dir = self
                .hover_point
                .map(|h| (h - start).normalize_or_zero())
                .unwrap_or(DVec3::X);
            dir * dist
        };

        self.execute_move(delta, scene);
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_move_tool_translation() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.target_vertices, vec![v0]);

        let ctx1 = ToolContext::new(DVec3::new(4.0, 5.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices[v0].position, DVec3::new(4.0, 5.0, 0.0));
    }

    #[test]
    fn test_move_tool_copy_mode() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let mut ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        ctx0.ctrl_held = true;
        tool.on_click(&mut scene, &ctx0);
        assert!(tool.is_copy_mode);

        let ctx1 = ToolContext::new(DVec3::new(3.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Original stays at (0,0,0); new copy at (3,0,0)
        assert_eq!(scene.mesh.vertices.len(), 2);
        assert_eq!(scene.mesh.vertices[0].position, DVec3::ZERO);
        assert_eq!(scene.mesh.vertices[1].position, DVec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn test_move_tool_array_repeat() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::ZERO);
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let mut ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        ctx0.ctrl_held = true;
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(10.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Type "3x" array repeat
        assert!(tool.on_vcb_input(&mut scene, "3x").is_ok());
        // Original + 1st copy + 2nd & 3rd copy from array repeat
        assert_eq!(scene.mesh.vertices.len(), 4);
    }
}
