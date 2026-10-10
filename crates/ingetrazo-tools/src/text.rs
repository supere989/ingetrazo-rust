// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Leader text note tool (X).
//!
//! 1. Anchor on geometry.
//! 2. Label placement offset.

use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_core::textlabel::TextLabel;

use crate::base::{Tool, ToolContext, ToolType};

pub struct TextTool {
    anchor: Option<DVec3>,
    hover_point: Option<DVec3>,
    default_text: String,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            anchor: None,
            hover_point: None,
            default_text: String::new(),
        }
    }
}

impl Tool for TextTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Text
    }

    fn name(&self) -> &'static str {
        "Text"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("X")
    }

    fn description(&self) -> &'static str {
        "Place a text note with a leader pointing to the model."
    }

    fn vcb_label(&self) -> &'static str {
        if self.anchor.is_none() {
            "Anchor point"
        } else {
            "Label text"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.anchor.is_none() {
            self.anchor = Some(ctx.world);
            self.default_text = format!("({:.2}, {:.2}, {:.2})", ctx.world.x, ctx.world.y, ctx.world.z);
        } else {
            let anc = self.anchor.unwrap();
            let offset = ctx.world - anc;
            let text = if self.default_text.is_empty() {
                format!("Note ({:.2}, {:.2})", anc.x, anc.y)
            } else {
                self.default_text.clone()
            };

            let label = TextLabel::new(anc, offset, text);
            scene.text_labels.push(label);
            scene.bump_version();

            self.anchor = None;
            self.hover_point = None;
            self.default_text.clear();
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.anchor = None;
        self.hover_point = None;
        self.default_text.clear();
    }

    fn on_vcb_input(&mut self, _scene: &mut Scene, text: &str) -> Result<(), String> {
        self.default_text = text.to_string();
        Ok(())
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(a), Some(h)) = (self.anchor, self.hover_point) {
            Some((a, h))
        } else {
            None
        }
    }
}
