// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Rotated Rectangle tool: 3-click oriented rectangle on arbitrary planes.
//!
//! 1. First click: start corner.
//! 2. Second click: baseline endpoint (length & orientation).
//! 3. Third click: width endpoint (width offset perpendicular to baseline).

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

pub struct RotatedRectangleTool {
    first_corner: Option<DVec3>,
    second_corner: Option<DVec3>,
    hover_point: Option<DVec3>,
}

impl Default for RotatedRectangleTool {
    fn default() -> Self {
        Self {
            first_corner: None,
            second_corner: None,
            hover_point: None,
        }
    }
}

impl Tool for RotatedRectangleTool {
    fn tool_type(&self) -> ToolType {
        ToolType::RotatedRectangle
    }

    fn name(&self) -> &'static str {
        "Rotated Rectangle"
    }

    fn description(&self) -> &'static str {
        "Draw an oriented rectangle: first corner, baseline, then width."
    }

    fn vcb_label(&self) -> &'static str {
        if self.first_corner.is_none() {
            "First corner"
        } else if self.second_corner.is_none() {
            "Length; Angle"
        } else {
            "Width"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.first_corner.is_none() {
            self.first_corner = Some(ctx.world);
        } else if self.second_corner.is_none() {
            if let Some(p0) = self.first_corner {
                if (ctx.world - p0).length() > 1e-6 {
                    self.second_corner = Some(ctx.world);
                }
            }
        } else {
            // Third click: width
            let p0 = self.first_corner.unwrap();
            let p1 = self.second_corner.unwrap();
            let p_width = ctx.world;

            let base = p1 - p0;
            let base_len = base.length();
            if base_len > 1e-6 {
                let u = base / base_len;
                // Vector from p1 towards p_width projected perpendicular to u
                let d = p_width - p1;
                let perp = d - u * d.dot(u);
                if perp.length() > 1e-6 {
                    let p2 = p1 + perp;
                    let p3 = p0 + perp;

                    let v0 = scene.mesh.add_vertex(p0);
                    let v1 = scene.mesh.add_vertex(p1);
                    let v2 = scene.mesh.add_vertex(p2);
                    let v3 = scene.mesh.add_vertex(p3);

                    scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
                    scene.bump_version();
                }
            }

            self.first_corner = None;
            self.second_corner = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.first_corner = None;
        self.second_corner = None;
        self.hover_point = None;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(p0), None) = (self.first_corner, self.second_corner) {
            self.hover_point.map(|h| (p0, h))
        } else if let (Some(_), Some(p1)) = (self.first_corner, self.second_corner) {
            self.hover_point.map(|h| (p1, h))
        } else {
            None
        }
    }
}
