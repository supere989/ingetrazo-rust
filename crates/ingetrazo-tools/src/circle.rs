// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    base::{Tool, ToolContext, ToolType},
    shape_plane::{self as qt, ShapePlane},
};
use glam::DVec3;
use ingetrazo_core::scene::Scene;
#[derive(Debug, Clone)]
pub struct CircleTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub segments: usize,
    preview_color: Option<[f32; 3]>,
    plane: ShapePlane,
    error: Option<String>,
}
impl Default for CircleTool {
    fn default() -> Self {
        Self {
            start_point: None,
            hover_point: None,
            plane_normal: DVec3::Z,
            segments: 24,
            preview_color: None,
            plane: Default::default(),
            error: None,
        }
    }
}
impl CircleTool {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_segments(segments: usize) -> Self {
        Self {
            segments: segments.max(3),
            ..Self::default()
        }
    }
    pub fn with_normal(normal: DVec3) -> Self {
        let mut t = Self::default();
        t.plane_normal = qt::norm(normal);
        t.plane.note(DVec3::ZERO, t.plane_normal);
        t
    }
    pub fn reset(&mut self) {
        self.preview_color = None;
        self.start_point = None;
        self.plane.reset();
        self.error = None;
    }
    fn normal_for(&self, center: DVec3, rim: DVec3) -> DVec3 {
        let mut p = self.plane.clone();
        if p.hover.is_none() && p.captured.is_none() {
            p.note(center, self.plane_normal);
        }
        p.drawing(&[center, rim]).1
    }
    pub fn compute_points(&self, center: DVec3, rim: DVec3) -> Option<Vec<DVec3>> {
        if !center.is_finite() || !rim.is_finite() || self.segments < 3 {
            return None;
        }
        let (u, v) = self.plane.axes_for(self.normal_for(center, rim));
        let d = qt::sub(rim, center);
        let du = qt::dot(d, u);
        let dv = qt::dot(d, v);
        let r = du.hypot(dv);
        if r < 1e-6 {
            return None;
        }
        let a0 = dv.atan2(du);
        let mut pts = Vec::new();
        pts.try_reserve_exact(self.segments).ok()?;
        for k in 0..self.segments {
            let a = a0 + std::f64::consts::TAU * k as f64 / self.segments as f64;
            pts.push(qt::add(
                center,
                qt::mul(qt::add(qt::mul(u, a.cos()), qt::mul(v, a.sin())), r),
            ));
        }
        Some(pts)
    }
    pub fn preview_points(&self) -> Option<Vec<DVec3>> {
        self.compute_points(self.start_point?, self.hover_point?)
    }
    fn commit_points(&mut self, pts: Vec<DVec3>, scene: &mut Scene) -> Result<(), String> {
        let segments = pts
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, pts[(i + 1) % pts.len()]))
            .collect::<Vec<_>>();
        let curve = scene
            .mesh
            .edges
            .iter()
            .filter_map(|e| e.curve_id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Curve identity exhausted")?;
        let mesh = ingetrazo_core::drafting::add_segments_tagged_with_id(
            &scene.mesh,
            &segments,
            Some(&pts),
            Some(curve),
            scene.active_ifc.as_ref(),
            scene.bim_draw_id.as_ref(),
        )?;
        scene.mesh = mesh;
        scene.bump_version();
        self.reset();
        Ok(())
    }
}
impl Tool for CircleTool {
    fn shape_segments(&self) -> Option<usize> {
        Some(self.segments)
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Circle
    }
    fn name(&self) -> &'static str {
        "Circle"
    }
    fn shortcut(&self) -> Option<&'static str> {
        Some("C")
    }
    fn description(&self) -> &'static str {
        "Draw a circle from its centre and radius."
    }
    fn vcb_label(&self) -> &'static str {
        if self.start_point.is_some() {
            "Radius"
        } else {
            "Sides"
        }
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn magnetic_axis_deg(&self) -> Option<f64> {
        Some(3.)
    }
    fn screen_axis_px(&self) -> Option<f64> {
        Some(9.)
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.start_point
    }
    fn note_pick_viewport(
        &mut self,
        v: ingetrazo_core::screen_pick::PixelViewport,
        _: DVec3,
        _: u64,
    ) {
        self.plane.viewport = Some(v);
    }
    fn note_cursor_plane(&mut self, p: DVec3, n: DVec3) {
        self.plane.note(p, n);
        self.plane_normal = n;
    }
    fn lock_drawing_plane(&mut self, n: DVec3) -> bool {
        self.start_point.is_none() && self.plane.lock_axis(n)
    }
    fn lock_drawing_reference(
        &mut self,
        r: Option<ingetrazo_core::screen_pick::HoverReference>,
    ) -> bool {
        self.start_point.is_none() && self.plane.lock_reference(r)
    }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> {
        let Some(p) = self.start_point else {
            return self.plane.locked(self.hover_point);
        };
        Some(self.plane.drawing(&[p, self.hover_point.unwrap_or(p)]))
    }
    fn on_activate(&mut self, s: &mut Scene) {
        self.plane.axes = s.drawing_axes();
        self.reset();
        self.hover_point = None;
    }
    fn on_deactivate(&mut self, _: &mut Scene) {
        self.reset();
        self.hover_point = None;
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_move(&mut self, s: &mut Scene, c: &ToolContext) {
        self.preview_color = self.plane.lock_color();
        self.plane.axes = s.drawing_axes();
        self.hover_point = Some(qt::q(c.world));
    }
    fn on_click(&mut self, s: &mut Scene, c: &ToolContext) {
        self.plane.axes = s.drawing_axes();
        if let Some(start) = self.start_point {
            if let Some(pts) = self.compute_points(start, c.world) {
                if let Err(e) = self.commit_points(pts, s) {
                    self.error = Some(e);
                }
            }
        } else {
            let p = qt::q(c.world);
            self.start_point = Some(p);
            self.plane.start(p);
        }
    }
    fn on_vcb_input(&mut self, s: &mut Scene, text: &str) -> Result<(), String> {
        let text = text.trim();
        let suffix = text.ends_with('s') || text.ends_with('S');
        if suffix || self.start_point.is_none() {
            let raw = if suffix {
                &text[..text.len() - 1]
            } else {
                text
            };
            let n = ingetrazo_core::units::vcb::parse_single(raw, 1.)?;
            let n = if suffix {
                if n != n.trunc() {
                    return Err("Sides suffix requires an integer".into());
                }
                n
            } else {
                n.round_ties_even()
            };
            if !n.is_finite() || n < 3. || n > usize::MAX as f64 {
                return Err("Provide side count >=3".into());
            }
            self.segments = n.round_ties_even() as usize;
            return Ok(());
        }
        let r =
            ingetrazo_core::units::vcb::parse_single(text, ingetrazo_core::units::model_scale(s))?;
        if !r.is_finite() || r <= 0. {
            return Err("Radius must be positive and finite".into());
        }
        let center = self.start_point.unwrap();
        let hover = self
            .hover_point
            .ok_or("Move to set the radius direction first")?;
        let (u, v) = self.plane.axes_for(self.normal_for(center, hover));
        let d = qt::sub(hover, center);
        let a = qt::dot(d, v).atan2(qt::dot(d, u));
        let rim = qt::add(
            center,
            qt::mul(qt::add(qt::mul(u, a.cos()), qt::mul(v, a.sin())), r),
        );
        let pts = self
            .compute_points(center, rim)
            .ok_or("Failed to generate circle")?;
        self.commit_points(pts, s)
    }
    fn preview_color(&self) -> Option<[f32; 3]> {
        self.preview_color
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        if self.start_point.is_none() {
            return self
                .hover_point
                .map(|p| self.plane.cursor_ring(p, self.segments))
                .unwrap_or_default();
        }
        let Some(p) = self.preview_points() else {
            return vec![];
        };
        (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])).collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_circle_tool_two_clicks() {
        let mut tool = CircleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx_hover =
            ToolContext::new(DVec3::new(2.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);
        assert_eq!(tool.preview_points().unwrap().len(), 24);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 24);
        assert_eq!(scene.mesh.edges.len(), 24);
        assert_eq!(scene.mesh.faces.len(), 1);
        // Area of regular 24-gon with radius 2: 0.5 * 24 * 2^2 * sin(2pi/24) ~ 12.423
        assert!((scene.mesh.face_area(0) - 12.423).abs() < 0.1);
        assert!(tool.start_point.is_none());
    }

    #[test]
    fn test_circle_tool_segments_and_vcb() {
        let mut tool = CircleTool::new();
        let mut scene = Scene::new();

        // Change segments via VCB
        assert!(tool.on_vcb_input(&mut scene, "8s").is_ok());
        assert_eq!(tool.segments, 8);

        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Source radius input requires an actual hover direction.
        assert!(tool.on_vcb_input(&mut scene, "5.0").is_err());
        tool.on_move(
            &mut scene,
            &ToolContext::new(DVec3::new(2., 1., 0.), Ray::new(DVec3::Z, -DVec3::Z)),
        );
        assert!(tool.on_vcb_input(&mut scene, "5.0").is_ok());
        assert_eq!(scene.mesh.vertices.len(), 8);
        assert_eq!(scene.mesh.faces.len(), 1);
    }
}
