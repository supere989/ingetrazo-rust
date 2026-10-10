// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Inference snap marker glyphs (Track 3).
//!
//! Generates 2D/3D markers for snapping:
//! - Endpoint (green square)
//! - Midpoint (cyan circle)
//! - Center (cyan circle)
//! - Intersection (dark cross)
//! - On Edge (red diamond)
//! - On Face (blue square)
//! - Origin (orange target)
//! - Axis Lock & Inference (colored axis dashed ray)
//! - Guide (blue cross)
//! - From Point & Two-Point From Point (tracking lines)
//! - Reference Edge (magenta parallel / perpendicular line)

use glam::DVec3;
use ingetrazo_core::snap::{SnapKind, SnapResult};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlyphShape {
    Square,
    Circle,
    Diamond,
    Cross,
    Triangle,
    Target,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapGlyph {
    pub point: DVec3,
    pub shape: GlyphShape,
    pub color: [f32; 4],
    pub size_px: f32,
    pub label: String,
    pub axis_line: Option<(DVec3, DVec3, [f32; 4])>,
    pub guides: Vec<(DVec3, DVec3, [f32; 4])>,
}

impl SnapGlyph {
    pub fn from_snap_result(snap: &SnapResult, start_point: Option<DVec3>) -> Option<Self> {
        if snap.kind == SnapKind::None {
            return None;
        }

        let (shape, color, size_px) = match snap.kind {
            SnapKind::Endpoint => (GlyphShape::Square, [0.16, 0.62, 0.36, 1.0], 8.0),
            SnapKind::Midpoint => (GlyphShape::Circle, [0.20, 0.66, 0.74, 1.0], 8.0),
            SnapKind::Center => (GlyphShape::Circle, [0.20, 0.66, 0.74, 1.0], 8.0),
            SnapKind::Intersection => (GlyphShape::Cross, [0.2, 0.2, 0.2, 1.0], 10.0),
            SnapKind::OnEdge => (GlyphShape::Diamond, [0.86, 0.22, 0.27, 1.0], 8.0),
            SnapKind::OnFace => (GlyphShape::Square, [0.42, 0.46, 0.92, 1.0], 7.0),
            SnapKind::Origin => (GlyphShape::Target, [0.95, 0.45, 0.16, 1.0], 10.0),
            SnapKind::AxisLock => (GlyphShape::Circle, [0.9, 0.2, 0.2, 1.0], 7.0),
            SnapKind::AxisInference => (GlyphShape::Circle, [0.8, 0.3, 0.3, 1.0], 7.0),
            SnapKind::Reference => (GlyphShape::Diamond, [0.85, 0.30, 0.80, 1.0], 8.0),
            SnapKind::FromPoint => (GlyphShape::Square, [0.16, 0.62, 0.36, 1.0], 8.0),
            SnapKind::TwoPointFromPoint => (GlyphShape::Cross, [0.16, 0.62, 0.36, 1.0], 10.0),
            SnapKind::Close => (GlyphShape::Triangle, [0.20, 0.40, 0.78, 1.0], 9.0),
            SnapKind::Guide => (GlyphShape::Cross, [0.25, 0.50, 0.90, 1.0], 8.0),
            SnapKind::None => return None,
        };

        let axis_line = if let (Some(ax), Some(start)) = (snap.axis, start_point) {
            let (axis_color, axis_dir) = match ax {
                'x' | 'X' => ([0.9, 0.2, 0.2, 0.8], DVec3::X),
                'y' | 'Y' => ([0.2, 0.8, 0.2, 0.8], DVec3::Y),
                'z' | 'Z' => ([0.2, 0.4, 0.9, 0.8], DVec3::Z),
                _ => ([0.7, 0.7, 0.7, 0.8], DVec3::Z),
            };
            let p0 = start - axis_dir * 100.0;
            let p1 = start + axis_dir * 100.0;
            Some((p0, p1, axis_color))
        } else {
            None
        };

        let guides: Vec<(DVec3, DVec3, [f32; 4])> = snap
            .guides
            .iter()
            .map(|(p0, p1, rgb)| (*p0, *p1, [rgb[0], rgb[1], rgb[2], 0.85]))
            .collect();

        Some(Self {
            point: snap.point,
            shape,
            color,
            size_px,
            label: snap.label.clone(),
            axis_line,
            guides,
        })
    }
}
