// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Section Plane tool: place cutting planes to slice models open.

use glam::DVec3;
use ingetrazo_core::section::{next_symbol, SectionPlane};
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

pub struct SectionPlaneTool {
    hover_point: Option<DVec3>,
    hover_normal: DVec3,
}

impl Default for SectionPlaneTool {
    fn default() -> Self {
        Self {
            hover_point: None,
            hover_normal: DVec3::Z,
        }
    }
}

impl Tool for SectionPlaneTool {
    fn tool_type(&self) -> ToolType {
        ToolType::SectionPlane
    }

    fn name(&self) -> &'static str {
        "Section Plane"
    }

    fn description(&self) -> &'static str {
        "Place a plane that cuts the model open to show its interior."
    }

    fn vcb_label(&self) -> &'static str {
        "Position"
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let sym = next_symbol(&scene.section_planes);
        let count = scene.section_planes.len() + 1;
        let name = format!("Section {}", count);

        let plane = SectionPlane::new(ctx.world, self.hover_normal, name, sym);
        for existing in &mut scene.section_planes{existing.active=false;}
        scene.section_planes.push(plane);
        scene.show_section_cuts = true;
        scene.show_section_planes = true;
        scene.bump_version();
    }

    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
        if let Some((face_id, _, _)) = crate::base::pick_scene_face(scene, &ctx.ray) {
            self.hover_normal = scene.mesh.face_normal(face_id);
        } else {
            self.hover_normal = DVec3::Z;
        }
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.hover_point = None;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.hover_point.map(|p| (p, p + self.hover_normal * 1.0))
    }
}
