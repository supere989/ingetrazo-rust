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
mod preview;

pub struct DimensionTool {
    first_point: Option<DVec3>,
    second_point: Option<DVec3>,
    hover_point: Option<DVec3>,
    forward: DVec3,
    proposed: Option<Dimension>,
}

impl Default for DimensionTool {
    fn default() -> Self {
        Self {
            first_point: None,
            second_point: None,
            hover_point: None,
            forward: DVec3::ZERO,
            proposed: None,
        }
    }
}

impl DimensionTool {
    fn frontal(&self, scene: &Scene, p: DVec3) -> DVec3 {
        let Some(refpoint) = self.first_point else {
            return p;
        };
        let n = preview::length(self.forward.as_vec3());
        if n < 1e-9 {
            return p;
        }
        let f = preview::normalized(self.forward.as_vec3());
        let axes = scene.drawing_axes();
        let vectors = scene
            .drawing_frame
            .filter(|m| m.to_cols_array().iter().all(|x| (*x as f32) as f64 == *x))
            .map(|m| preview::axes(m).map(|a| a.as_dvec3()))
            .unwrap_or([axes.x, axes.y, axes.z]);
        if scene.drawing_frame.is_some() {
            let mut best = (0f32, 0usize);
            for (i, a) in vectors.into_iter().enumerate() {
                let v = preview::dot(f, a.as_vec3()).abs();
                if v > best.0 || (v == best.0 && i > best.1) {
                    best = (v, i);
                }
            }
            if best.0 < 0.97 {
                return p;
            }
            let a = vectors[best.1];
            if p.as_vec3().as_dvec3() == p
                && refpoint.as_vec3().as_dvec3() == refpoint
                && a.as_vec3().as_dvec3() == a
            {
                return (p.as_vec3()
                    + a.as_vec3() * preview::dot(refpoint.as_vec3() - p.as_vec3(), a.as_vec3()))
                .as_dvec3();
            }
            return p + a * (refpoint - p).dot(a);
        }
        let v = f.abs();
        if v.max_element() < 0.97 {
            return p;
        }
        if v.x >= v.y && v.x >= v.z {
            DVec3::new(refpoint.x, p.y, p.z)
        } else if v.y >= v.z {
            DVec3::new(p.x, refpoint.y, p.z)
        } else {
            DVec3::new(p.x, p.y, refpoint.z)
        }
    }
    fn proposal(&self, scene: &Scene, p: DVec3) -> Option<Dimension> {
        let (a, b) = (self.first_point?, self.second_point?);
        let qt = [a, b, p].iter().all(|p| p.as_vec3().as_dvec3() == *p)
            && scene.dimension_frame == glam::DMat4::IDENTITY;
        let (offset, axis) = if qt {
            preview::proposed(a, b, p)
        } else {
            Dimension::placement_in_frame(a, b, p, scene.dimension_frame)
        };
        let dim = Dimension {
            measurement_frame: scene.dimension_frame,
            anchor_a: None,
            anchor_b: None,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            layer: None,
            axis,
            id: uuid::Uuid::nil(),
            start: a,
            end: b,
            offset,
            text_override: None,
        };
        Some(dim)
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

    fn note_camera_forward(&mut self, forward: DVec3) {
        self.forward = forward;
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !ctx.world.is_finite() {
            return;
        }
        if self.first_point.is_none() {
            self.first_point = Some(ctx.world);
        } else if self.second_point.is_none() {
            let p = self.frontal(scene, ctx.world);
            if let Some(a) = self.first_point {
                let distance = if [p, a].iter().all(|p| p.as_vec3().as_dvec3() == *p) {
                    f64::from(preview::length(p.as_vec3() - a.as_vec3()))
                } else {
                    (p - a).length()
                };
                if distance >= 1e-6 {
                    self.second_point = Some(p);
                    self.proposed = self.hover_point.and_then(|h| self.proposal(scene, h));
                }
            }
        } else if let Some(mut dim) = self.proposal(scene, ctx.world) {
            if !dim.offset.is_finite() {
                return;
            }
            dim.id = uuid::Uuid::new_v4();
            dim.anchor_a = scene.resolve_dimension_anchor(dim.start);
            dim.anchor_b = scene.resolve_dimension_anchor(dim.end);
            scene.dimensions.push(dim);
            scene.bump_version();
            self.first_point = None;
            self.second_point = None;
            self.proposed = None;
        }
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !ctx.world.is_finite() {
            return;
        }
        let p = self.frontal(scene, ctx.world);
        if self.hover_point == Some(p)
            && self
                .proposed
                .as_ref()
                .map(|d| d.measurement_frame == scene.dimension_frame)
                .unwrap_or(self.second_point.is_none())
        {
            return;
        }
        self.hover_point = Some(p);
        self.proposed = self.proposal(scene, p);
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.first_point = None;
        self.second_point = None;
        self.proposed = None;
    }

    fn distance_label(&self) -> Option<(f64, DVec3)> {
        let a = self.first_point?;
        let h = self.hover_point?;
        if self.second_point.is_none() {
            let qt = [a, h].iter().all(|p| p.as_vec3().as_dvec3() == *p);
            return Some(if qt {
                (
                    f64::from(preview::length(h.as_vec3() - a.as_vec3())),
                    ((a.as_vec3() + h.as_vec3()) * 0.5).as_dvec3(),
                )
            } else {
                ((h - a).length(), (a + h) * 0.5)
            });
        }
        let d = self.proposed.as_ref()?;
        let qt = [d.start, d.end, d.offset]
            .iter()
            .all(|p| p.as_vec3().as_dvec3() == *p)
            && d.measurement_frame == glam::DMat4::IDENTITY;
        if qt {
            let (ap, bp, value) = preview::lines_value(d.start, d.end, d.offset, d.axis.as_deref());
            Some((value, ((ap.as_vec3() + bp.as_vec3()) * 0.5).as_dvec3()))
        } else {
            let (ap, bp) = d.line_points();
            Some((d.length(), (ap + bp) * 0.5))
        }
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        let Some(a) = self.first_point else {
            return Vec::new();
        };
        let Some(h) = self.hover_point else {
            return Vec::new();
        };
        if self.second_point.is_none() {
            return vec![(a, h)];
        }
        let Some(d) = self.proposed.as_ref() else {
            return Vec::new();
        };
        let qt = [d.start, d.end, d.offset]
            .iter()
            .all(|p| p.as_vec3().as_dvec3() == *p)
            && d.measurement_frame == glam::DMat4::IDENTITY;
        let (ap, bp) = if qt {
            let (ap, bp, _) = preview::lines_value(d.start, d.end, d.offset, d.axis.as_deref());
            (ap, bp)
        } else {
            d.line_points()
        };
        vec![(d.start, ap), (d.end, bp), (ap, bp)]
    }
    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.rubber_band_lines().first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;
    fn ctx(p: DVec3) -> ToolContext {
        ToolContext::new(p, Ray::new(DVec3::Z, -DVec3::Z))
    }
    #[test]
    fn native_precision_preview_is_transient_warm_and_cancelable() {
        let mut scene = Scene::new();
        let before = scene.clone();
        let mut tool = DimensionTool::default();
        tool.note_camera_forward(-DVec3::Z);
        let a = DVec3::new(1. + 2f64.powi(-40), 2., 3.);
        let b = DVec3::new(4., 6., 12.);
        let p = DVec3::new(7., 8., 20.);
        tool.on_move(&mut scene, &ctx(a));
        tool.on_click(&mut scene, &ctx(a));
        tool.on_move(&mut scene, &ctx(b));
        tool.on_click(&mut scene, &ctx(b));
        tool.on_move(&mut scene, &ctx(p));
        assert_eq!(scene, before);
        let id = tool.proposed.as_ref().unwrap().id;
        let lines = tool.rubber_band_lines();
        let label = tool.distance_label();
        tool.on_move(&mut scene, &ctx(p));
        assert_eq!(tool.proposed.as_ref().unwrap().id, id);
        assert_eq!(tool.rubber_band_lines(), lines);
        assert_eq!(tool.distance_label(), label);
        tool.on_click(&mut scene, &ctx(p));
        assert_eq!(scene.dimensions[0].start.x, a.x);
        assert_eq!(scene.dimensions[0].end.z, a.z);
        assert!(scene.dimensions[0].offset.z != 0.);
        assert!(tool.distance_label().is_none());
        tool.on_click(&mut scene, &ctx(a));
        tool.on_cancel(&mut scene);
        assert!(tool.rubber_band_lines().is_empty());
        assert!(tool.distance_label().is_none());
    }
}
