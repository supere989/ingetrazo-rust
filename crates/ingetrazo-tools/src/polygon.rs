// SPDX-License-Identifier: GPL-3.0-or-later
//! Polygon shares the original radial controller, including curve identity.
use crate::{
    base::{Tool, ToolContext, ToolType},
    circle::CircleTool,
};
use glam::DVec3;
use ingetrazo_core::scene::Scene;
#[derive(Debug, Clone)]
pub struct PolygonTool {
    pub sides: usize,
    radial: CircleTool,
}
impl Default for PolygonTool {
    fn default() -> Self {
        Self {
            sides: 6,
            radial: CircleTool::with_segments(6),
        }
    }
}
impl Tool for PolygonTool {
    fn shape_segments(&self) -> Option<usize> {
        Some(self.sides)
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Polygon
    }
    fn name(&self) -> &'static str {
        "Polygon"
    }
    fn description(&self) -> &'static str {
        "Draw a regular polygon from centre to vertex."
    }
    fn vcb_label(&self) -> &'static str {
        self.radial.vcb_label()
    }
    fn error_message(&self) -> Option<&str> {
        self.radial.error_message()
    }
    fn magnetic_axis_deg(&self) -> Option<f64> {
        self.radial.magnetic_axis_deg()
    }
    fn screen_axis_px(&self) -> Option<f64> {
        self.radial.screen_axis_px()
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.radial.snap_anchor()
    }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> {
        self.radial.drawing_plane()
    }
    fn note_pick_viewport(
        &mut self,
        v: ingetrazo_core::screen_pick::PixelViewport,
        e: DVec3,
        r: u64,
    ) {
        self.radial.note_pick_viewport(v, e, r);
    }
    fn note_cursor_plane(&mut self, p: DVec3, n: DVec3) {
        self.radial.note_cursor_plane(p, n);
    }
    fn lock_drawing_plane(&mut self, n: DVec3) -> bool {
        self.radial.lock_drawing_plane(n)
    }
    fn lock_drawing_reference(
        &mut self,
        r: Option<ingetrazo_core::screen_pick::HoverReference>,
    ) -> bool {
        self.radial.lock_drawing_reference(r)
    }
    fn on_activate(&mut self, s: &mut Scene) {
        self.radial.segments = self.sides;
        self.radial.on_activate(s);
    }
    fn on_deactivate(&mut self, s: &mut Scene) {
        self.radial.on_deactivate(s);
    }
    fn on_cancel(&mut self, s: &mut Scene) {
        self.radial.on_cancel(s);
    }
    fn on_move(&mut self, s: &mut Scene, c: &ToolContext) {
        self.radial.segments = self.sides;
        self.radial.on_move(s, c);
    }
    fn on_click(&mut self, s: &mut Scene, c: &ToolContext) {
        self.radial.segments = self.sides;
        self.radial.on_click(s, c);
    }
    fn on_vcb_input(&mut self, s: &mut Scene, t: &str) -> Result<(), String> {
        self.radial.segments = self.sides;
        let r = self.radial.on_vcb_input(s, t);
        self.sides = self.radial.segments;
        r
    }
    fn preview_color(&self) -> Option<[f32; 3]> {
        self.radial.preview_color()
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        self.radial.rubber_band_lines()
    }
}
