// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Freehand sketching tool.
//!
//! Click and drag to draw a continuous curve. The sampled stroke is simplified
//! via Ramer–Douglas–Peucker. If the end point lands close to the start,
//! the loop closes into a face.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

fn rdp_recursive(pts: &[DVec3], tol: f64, keep: &mut [bool], start: usize, end: usize) {
    if end <= start + 1 {
        return;
    }
    let p_start = pts[start];
    let p_end = pts[end];
    let line_vec = p_end - p_start;
    let line_len_sq = line_vec.length_squared();

    let mut max_dist = 0.0;
    let mut max_idx = start;

    for i in (start + 1)..end {
        let p = pts[i];
        let dist = if line_len_sq < 1e-12 {
            (p - p_start).length()
        } else {
            let t = ((p - p_start).dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
            let proj = p_start + line_vec * t;
            (p - proj).length()
        };
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > tol {
        keep[max_idx] = true;
        rdp_recursive(pts, tol, keep, start, max_idx);
        rdp_recursive(pts, tol, keep, max_idx, end);
    }
}

pub fn simplify_rdp(pts: &[DVec3], tolerance: f64) -> Vec<DVec3> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    rdp_recursive(pts, tolerance, &mut keep, 0, pts.len() - 1);
    pts.iter()
        .enumerate()
        .filter_map(|(i, &p)| if keep[i] { Some(p) } else { None })
        .collect()
}

pub struct FreehandTool {
    points: Vec<DVec3>,
    drawing: bool,
    tolerance: f64,
}

impl Default for FreehandTool {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            drawing: false,
            tolerance: 0.05,
        }
    }
}

impl Tool for FreehandTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Freehand
    }

    fn name(&self) -> &'static str {
        "Freehand"
    }

    fn description(&self) -> &'static str {
        "Drag to sketch a curve; a stroke that ends where it began closes into a face."
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !self.drawing {
            self.drawing = true;
            self.points.clear();
            self.points.push(ctx.world);
        } else {
            // Commit drawing on second click / release
            self.commit(scene);
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        if self.drawing {
            if let Some(&last) = self.points.last() {
                if (ctx.world - last).length() > 0.02 {
                    self.points.push(ctx.world);
                }
            }
        }
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.points.clear();
        self.drawing = false;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if self.points.len() >= 2 {
            let last = self.points[self.points.len() - 1];
            let prev = self.points[self.points.len() - 2];
            Some((prev, last))
        } else {
            None
        }
    }
}

impl FreehandTool {
    pub fn commit(&mut self, scene: &mut Scene) {
        if self.points.len() < 2 {
            self.points.clear();
            self.drawing = false;
            return;
        }

        let simplified = simplify_rdp(&self.points, self.tolerance);
        if simplified.len() < 2 {
            self.points.clear();
            self.drawing = false;
            return;
        }

        let start = simplified[0];
        let end = *simplified.last().unwrap();
        let is_closed = (end - start).length() < 0.2 && simplified.len() >= 3;

        let mut vert_ids = Vec::with_capacity(simplified.len());
        for &pt in &simplified {
            vert_ids.push(scene.mesh.add_vertex(pt));
        }

        if is_closed {
            // Weld start and end
            let first_id = vert_ids[0];
            if let Some(last_id) = vert_ids.last_mut() {
                *last_id = first_id;
            }
            vert_ids.pop(); // Remove duplicate last vertex for add_face
            if vert_ids.len() >= 3 {
                scene.mesh.add_face(vert_ids, vec![]);
            }
        } else {
            for i in 0..(vert_ids.len() - 1) {
                scene.mesh.add_edge(vert_ids[i], vert_ids[i + 1]);
            }
        }

        scene.bump_version();
        self.points.clear();
        self.drawing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_simplification() {
        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.01, 0.0),
            DVec3::new(2.0, -0.01, 0.0),
            DVec3::new(3.0, 0.0, 0.0),
        ];
        let simp = simplify_rdp(&pts, 0.05);
        assert_eq!(simp.len(), 2);
        assert_eq!(simp[0], pts[0]);
        assert_eq!(simp[1], pts[3]);
    }
}
