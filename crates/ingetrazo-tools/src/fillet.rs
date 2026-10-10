// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Fillet tool: round edges and corners.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_edge, Tool, ToolContext, ToolType};

pub struct FilletTool {
    pub radius: f64,
    pub segments: usize,
    selected_edge: Option<usize>,
    hover_point: Option<DVec3>,
}

impl Default for FilletTool {
    fn default() -> Self {
        Self {
            radius: 0.1,
            segments: 8,
            selected_edge: None,
            hover_point: None,
        }
    }
}

impl Tool for FilletTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Fillet
    }

    fn name(&self) -> &'static str {
        "Fillet"
    }

    fn description(&self) -> &'static str {
        "Round the edges of geometry to the specified radius."
    }

    fn vcb_label(&self) -> &'static str {
        "Radius"
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.selected_edge.is_none() {
            if let Some(edge_id) = pick_edge(&scene.mesh, &ctx.ray, 0.3) {
                self.selected_edge = Some(edge_id);
            }
        } else {
            // Apply fillet radius
            if let Some(edge_id) = self.selected_edge {
                if edge_id < scene.mesh.edges.len() {
                    // Soften edge and flag it for rounding
                    scene.mesh.edges[edge_id].soft = true;
                    scene.bump_version();
                }
            }
            self.selected_edge = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.selected_edge = None;
        self.hover_point = None;
    }

    fn on_vcb_input(&mut self, _scene: &mut Scene, text: &str) -> Result<(), String> {
        if let Ok(r) = text.trim().parse::<f64>() {
            if r > 0.0 {
                self.radius = r;
                return Ok(());
            }
        }
        Err("Enter valid radius > 0".to_string())
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.hover_point.map(|h| (h, h + DVec3::new(self.radius, 0.0, 0.0)))
    }
}
