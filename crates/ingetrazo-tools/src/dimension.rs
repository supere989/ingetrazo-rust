// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! 3D Linear Dimension annotation tool (D).
//!
//! 1. First endpoint (point A).
//! 2. Second endpoint (point B).
//! 3. Offset click to place dimension line and extension lines.

use glam::DVec3;
use ingetrazo_core::dimensions::Dimension;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

pub struct DimensionTool {
    first_point: Option<DVec3>,
    second_point: Option<DVec3>,
    hover_point: Option<DVec3>,
}

impl Default for DimensionTool {
    fn default() -> Self {
        Self {
            first_point: None,
            second_point: None,
            hover_point: None,
        }
    }
}

impl Tool for DimensionTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Dimension
    }

    fn name(&self) -> &'static str {
        "Dimension"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("D")
    }

    fn description(&self) -> &'static str {
        "Place a linear dimension showing the distance between two points."
    }

    fn vcb_label(&self) -> &'static str {
        if self.first_point.is_none() {
            "First point"
        } else if self.second_point.is_none() {
            "Second point"
        } else {
            "Offset placement"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.first_point.is_none() {
            self.first_point = Some(ctx.world);
        } else if self.second_point.is_none() {
            if let Some(p0) = self.first_point {
                if (ctx.world - p0).length() > 1e-6 {
                    self.second_point = Some(ctx.world);
                }
            }
        } else {
            let p0 = self.first_point.unwrap();
            let p1 = self.second_point.unwrap();
            let p_off = ctx.world;

            // Offset vector is vector from midpoint of segment (p0..p1) to cursor
            let (offset,axis) = Dimension::placement_for_cursor(p0,p1,p_off);

            let mut dim = Dimension::new(p0, p1, offset);dim.axis=axis;
            dim.anchor_a=scene.resolve_dimension_anchor(p0);dim.anchor_b=scene.resolve_dimension_anchor(p1);
            scene.dimensions.push(dim);
            scene.bump_version();

            self.first_point = None;
            self.second_point = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.first_point = None;
        self.second_point = None;
        self.hover_point = None;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(p0), None) = (self.first_point, self.second_point) {
            self.hover_point.map(|h| (p0, h))
        } else if let (Some(p0), Some(p1)) = (self.first_point, self.second_point) {
            self.hover_point.map(|h| ((p0 + p1) * 0.5, h))
        } else {
            None
        }
    }
}
