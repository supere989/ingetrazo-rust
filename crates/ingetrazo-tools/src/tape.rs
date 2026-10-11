// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::guides::Guide;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapeMode {
    MeasureOnly,
    GuidePoint,
    GuideLine,
}

/// Interactive Tape Measure tool for distance readouts and construction guides.
#[derive(Debug, Clone)]
pub struct TapeTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub measured_distance: Option<f64>,
    pub mode: TapeMode,
}

impl Default for TapeTool {
    fn default() -> Self {
        Self {
            start_point: None,
            hover_point: None,
            measured_distance: None,
            mode: TapeMode::GuideLine,
        }
    }
}

impl TapeTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mode(mode: TapeMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.hover_point = None;
        self.measured_distance = None;
    }

    pub fn commit_guide(&mut self, end: DVec3, scene: &mut Scene) {
        let Some(start) = self.start_point else { return };
        let delta = end - start;
        let dist = delta.length();
        self.measured_distance = Some(dist);

        if dist > 1e-6 {
            match self.mode {
                TapeMode::MeasureOnly => {}
                TapeMode::GuidePoint => {
                    scene.guides.push(Guide::point(end));
                    scene.guides.push(Guide::segment(start, end));
                    scene.bump_version();
                }
                TapeMode::GuideLine => {
                    scene.guides.push(Guide::infinite_line(start, delta));
                    scene.bump_version();
                }
            }
        }

        self.start_point = None;
    }
}

impl Tool for TapeTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Tape
    }

    fn name(&self) -> &'static str {
        "Tape Measure"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("T")
    }

    fn description(&self) -> &'static str {
        "Measure distances, or pull guide lines off edges to draw against."
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
        if let Some(start) = self.start_point {
            self.measured_distance = Some((ctx.world - start).length());
        }
        if ctx.ctrl_held {
            self.mode = match self.mode {
                TapeMode::GuideLine => TapeMode::GuidePoint,
                TapeMode::GuidePoint => TapeMode::MeasureOnly,
                TapeMode::MeasureOnly => TapeMode::GuideLine,
            };
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.start_point.is_none() {
            self.start_point = Some(ctx.world);
        } else {
            self.commit_guide(ctx.world, scene);
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let dist=ingetrazo_core::units::vcb::parse_single(text,ingetrazo_core::units::model_scale(scene))?;
        let Some(start) = self.start_point else {
            return Err("No active start point for tape measure".into());
        };

        let dir = self
            .hover_point
            .map(|h| (h - start).normalize_or_zero())
            .unwrap_or(DVec3::X);

        let end = start + dir * dist;
        self.commit_guide(end, scene);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_tape_tool_measurement_and_guides() {
        let mut scene = Scene::new();
        let mut tool = TapeTool::with_mode(TapeMode::GuidePoint);

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(5.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(tool.measured_distance, Some(5.0));
        assert_eq!(scene.guides.len(), 2); // point and segment
    }

    #[test]
    fn test_tape_tool_vcb() {
        let mut scene = Scene::new();
        let mut tool = TapeTool::with_mode(TapeMode::GuideLine);

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx_h = ToolContext::new(DVec3::new(1.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_h);

        assert!(tool.on_vcb_input(&mut scene, "12.5").is_ok());
        assert_eq!(tool.measured_distance, Some(12.5));
        assert_eq!(scene.guides.len(), 1); // infinite line
    }
}
