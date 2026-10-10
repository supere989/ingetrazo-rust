// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::HashSet;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_edge, Tool, ToolContext, ToolType};

/// Interactive Eraser tool for removing edges/faces and softening/smoothing edges.
#[derive(Debug, Default, Clone)]
pub struct EraserTool {
    pub is_soften_mode: bool,
    pub is_dragging: bool,
    pub marked_edges: HashSet<usize>,
}

impl EraserTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_soften(soften: bool) -> Self {
        Self {
            is_soften_mode: soften,
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.is_dragging = false;
        self.marked_edges.clear();
    }

    /// Erase an edge and its incident faces.
    pub fn erase_edge(scene: &mut Scene, edge_id: usize) {
        if edge_id >= scene.mesh.edges.len() {
            return;
        }

        scene.mesh.edges[edge_id].hidden = true;
        let incident = scene.mesh.edges[edge_id].faces.clone();
        for fid in incident {
            if fid < scene.mesh.faces.len() {
                scene.mesh.faces[fid].vertices.clear();
            }
        }
        scene.bump_version();
    }

    /// Toggle or set soften/smooth flag on an edge.
    pub fn soften_edge(scene: &mut Scene, edge_id: usize) {
        if edge_id >= scene.mesh.edges.len() {
            return;
        }

        let curr = scene.mesh.edges[edge_id].soft;
        scene.mesh.edges[edge_id].soft = !curr;
        scene.bump_version();
    }

    pub fn apply_marked(&mut self, scene: &mut Scene) {
        for &eid in &self.marked_edges {
            if self.is_soften_mode {
                Self::soften_edge(scene, eid);
            } else {
                Self::erase_edge(scene, eid);
            }
        }
        self.reset();
    }
}

impl Tool for EraserTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Eraser
    }

    fn name(&self) -> &'static str {
        "Eraser"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("E")
    }

    fn description(&self) -> &'static str {
        "Click or drag over edges to erase them, together with the faces they bound."
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
        if ctx.ctrl_held || ctx.alt_held {
            self.is_soften_mode = true;
        }
        if self.is_dragging {
            if let Some(eid) = pick_edge(&scene.mesh, &ctx.ray, 0.25) {
                self.marked_edges.insert(eid);
            }
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let soften = self.is_soften_mode || ctx.ctrl_held || ctx.alt_held;
        if let Some(eid) = pick_edge(&scene.mesh, &ctx.ray, 0.25) {
            if soften {
                Self::soften_edge(scene, eid);
            } else {
                Self::erase_edge(scene, eid);
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_eraser_tool_erase_edge() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2], vec![]);

        let mut tool = EraserTool::new();
        let ray = Ray::new(DVec3::new(0.5, 0.0, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.0, 0.0), ray);

        tool.on_click(&mut scene, &ctx);

        let e0 = scene.mesh.find_edge(v0, v1).unwrap();
        assert!(scene.mesh.edges[e0].hidden);
        assert!(scene.mesh.faces[0].vertices.is_empty());
    }

    #[test]
    fn test_eraser_tool_soften_toggle() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let e = scene.mesh.add_edge(v0, v1).unwrap();

        let mut tool = EraserTool::with_soften(true);
        let ray = Ray::new(DVec3::new(0.5, 0.0, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.0, 0.0), ray);

        tool.on_click(&mut scene, &ctx);
        assert!(scene.mesh.edges[e].soft);
    }
}
