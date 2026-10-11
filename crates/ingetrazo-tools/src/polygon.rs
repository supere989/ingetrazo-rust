// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Regular polygon tool.
//!
//! Center click + radius click.
//! Default sides = 6. Typing a number in VCB before center changes side count.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

pub struct PolygonTool {
    pub sides: usize,
    authored_draw_faces:Vec<usize>,
    center: Option<DVec3>,
    hover_point: Option<DVec3>,
}

impl Default for PolygonTool {
    fn default() -> Self {
        Self {
            sides: 6,
            authored_draw_faces:Vec::new(),
            center: None,
            hover_point: None,
        }
    }
}

impl Tool for PolygonTool {
    fn take_authored_draw_faces(&mut self)->Vec<usize>{std::mem::take(&mut self.authored_draw_faces)}
    fn tool_type(&self) -> ToolType {
        ToolType::Polygon
    }

    fn name(&self) -> &'static str {
        "Polygon"
    }

    fn description(&self) -> &'static str {
        "Draw a regular polygon from center to vertex."
    }

    fn vcb_label(&self) -> &'static str {
        if self.center.is_none() {
            "Sides"
        } else {
            "Radius"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.center.is_none() {
            self.center = Some(ctx.world);
        } else {
            let c = self.center.unwrap();
            let rim = ctx.world;
            let radius_vec = rim - c;
            let radius = radius_vec.length();

            if radius > 1e-6 && self.sides >= 3 {
                // Compute polygon in the plane defined by radius_vec and normal
                let normal = DVec3::Z;
                let u = radius_vec.normalize();
                let v = normal.cross(u).normalize();

                let step = std::f64::consts::TAU / (self.sides as f64);
                let mut vert_ids = Vec::with_capacity(self.sides);

                for i in 0..self.sides {
                    let angle = (i as f64) * step;
                    let pt = c + (u * angle.cos() + v * angle.sin()) * radius;
                    vert_ids.push(scene.mesh.add_vertex(pt));
                }

                if let Some(id)=scene.mesh.add_face(vert_ids, vec![]){self.authored_draw_faces.push(id);}
                scene.bump_version();
            }

            self.center = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.center = None;
        self.hover_point = None;
    }

    fn on_vcb_input(&mut self, _scene: &mut Scene, text: &str) -> Result<(), String> {
        let clean = text.trim().trim_end_matches('s').trim_end_matches('S');
        if let Ok(n) = clean.parse::<usize>() {
            if n >= 3 {
                self.sides = n;
                return Ok(());
            }
        }
        Err("Enter side count >= 3 (e.g. 8 or 8s)".to_string())
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(c), Some(h)) = (self.center, self.hover_point) {
            Some((c, h))
        } else {
            None
        }
    }
}
