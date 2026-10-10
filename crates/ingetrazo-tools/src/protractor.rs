// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Protractor tool: measure angles and place angled guide lines.
//!
//! 1. Center vertex.
//! 2. Base reference direction (0° baseline).
//! 3. Sweep direction: commits infinite guide line along angled direction.

use glam::DVec3;
use ingetrazo_core::guides::Guide;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

pub struct ProtractorTool {
    center: Option<DVec3>,
    baseline: Option<DVec3>,
    hover_point: Option<DVec3>,
}

impl Default for ProtractorTool {
    fn default() -> Self {
        Self {
            center: None,
            baseline: None,
            hover_point: None,
        }
    }
}

impl Tool for ProtractorTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Protractor
    }

    fn name(&self) -> &'static str {
        "Protractor"
    }

    fn description(&self) -> &'static str {
        "Measure angles and create angled construction guide lines."
    }

    fn vcb_label(&self) -> &'static str {
        if self.center.is_none() {
            "Vertex"
        } else if self.baseline.is_none() {
            "Reference direction"
        } else {
            "Angle"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.center.is_none() {
            self.center = Some(ctx.world);
        } else if self.baseline.is_none() {
            if let Some(c) = self.center {
                if (ctx.world - c).length() > 1e-6 {
                    self.baseline = Some(ctx.world);
                }
            }
        } else {
            let c = self.center.unwrap();
            let sweep = ctx.world;
            let dir = sweep - c;

            if dir.length() > 1e-6 {
                let guide = Guide::infinite_line(c, dir.normalize());
                scene.guides.push(guide);
                scene.bump_version();
            }

            self.center = None;
            self.baseline = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.center = None;
        self.baseline = None;
        self.hover_point = None;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(c), None) = (self.center, self.baseline) {
            self.hover_point.map(|h| (c, h))
        } else if let (Some(c), Some(_)) = (self.center, self.baseline) {
            self.hover_point.map(|h| (c, h))
        } else {
            None
        }
    }
}
