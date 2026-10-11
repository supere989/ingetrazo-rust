// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Leader text note tool (X).
//!
//! 1. Anchor on geometry.
//! 2. Label placement offset.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{TextCreationRequest, TextSuggestion, Tool, ToolContext, ToolType};

pub struct TextTool {
    anchor: Option<DVec3>,
    hover_point: Option<DVec3>,
    default_text: String,
    request: Option<TextCreationRequest>,
    prompting: bool,
    viewport: Option<ingetrazo_core::screen_pick::PixelViewport>,
    suggestion: Option<TextSuggestion>,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            anchor: None,
            hover_point: None,
            default_text: String::new(),
            request: None,
            prompting: false,
            viewport: None,
            suggestion: None,
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
        let point = ctx.world.as_vec3();
        if !point.is_finite() || self.prompting {
            return;
        }
        if let Some(anchor) = self.anchor {
            let offset = point - anchor.as_vec3();
            if !offset.is_finite() {
                return;
            }
            self.request = Some(TextCreationRequest {
                anchor,
                offset: offset.as_dvec3(),
                suggestion: self.default_text.clone(),
            });
            self.prompting = true;
        } else {
            self.suggestion = Some(describe(scene, ctx, self.viewport));
            self.anchor = Some(point.as_dvec3());
            self.default_text = format!("({:.2}, {:.2}, {:.2})", point.x, point.y, point.z);
        }
    }

    fn note_pick_viewport(
        &mut self,
        viewport: ingetrazo_core::screen_pick::PixelViewport,
        _eye: DVec3,
        _revision: u64,
    ) {
        self.viewport = Some(viewport);
    }

    fn take_text_suggestion(&mut self) -> Option<TextSuggestion> {
        self.suggestion.take()
    }

    fn text_creation_request(&mut self) -> Option<TextCreationRequest> {
        self.request.take()
    }

    fn set_text_suggestion(&mut self, text: String) {
        if self.anchor.is_some() && !self.prompting {
            self.default_text = text;
        }
    }

    fn finish_text_creation(&mut self) {
        self.anchor = None;
        self.default_text.clear();
        self.request = None;
        self.prompting = false;
        self.suggestion = None;
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        let point = ctx.world.as_vec3();
        if point.is_finite() {
            self.hover_point = Some(point.as_dvec3());
        }
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.finish_text_creation();
    }

    fn value_label(&self) -> Option<(String, DVec3)> {
        self.anchor?;
        Some((
            if self.default_text.is_empty() {
                "…".into()
            } else {
                self.default_text.clone()
            },
            self.hover_point?,
        ))
    }

    fn snap_anchor(&self) -> Option<DVec3> {
        self.anchor
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if let (Some(a), Some(h)) = (self.anchor, self.hover_point) {
            Some((a, h))
        } else {
            None
        }
    }
}

// Source pick_edge is a reference pick: soft seams and occluded edges participate.
// Source edge_a is scene.edges only; group edges feed the separate pick_group index.
// This cold first-click traversal deliberately differs from Select's visible index.
fn describe(
    scene: &Scene,
    ctx: &ToolContext,
    viewport: Option<ingetrazo_core::screen_pick::PixelViewport>,
) -> TextSuggestion {
    use glam::{DMat4, DVec2};
    use ingetrazo_core::mesh::Mesh;
    fn edges(
        scene: &Scene,
        mesh: &Mesh,
        matrix: DMat4,
        loose: bool,
        viewport: ingetrazo_core::screen_pick::PixelViewport,
        cursor: DVec2,
        best: &mut (f64, Option<f64>),
    ) {
        for (id, edge) in mesh.edges.iter().enumerate() {
            if !(if loose {
                scene.mesh_edge_selectable(mesh, id)
            } else {
                scene.mesh_edge_visible(mesh, id)
            }) {
                continue;
            }
            let (Some(a), Some(b)) = (mesh.vertices.get(edge.v0), mesh.vertices.get(edge.v1))
            else {
                continue;
            };
            let a = ingetrazo_core::change_axes::map(matrix, a.position);
            let b = ingetrazo_core::change_axes::map(matrix, b.position);
            if !scene.section_point_visible(a) && !scene.section_point_visible(b) {
                continue;
            }
            let (Some(pa), Some(pb)) = (viewport.project(a), viewport.project(b)) else {
                continue;
            };
            let d = pb - pa;
            let t = if d.length_squared() > 0. {
                ((cursor - pa).dot(d) / d.length_squared()).clamp(0., 1.)
            } else {
                0.
            };
            let distance = (cursor - (pa + d * t)).length();
            if distance < best.0 {
                *best = (distance, Some((b.as_vec3() - a.as_vec3()).length() as f64));
            }
        }
    }
    if let (Some(viewport), Some(cursor)) = (viewport, ctx.screen) {
        let mut best = (8., None);
        edges(
            scene,
            &scene.mesh,
            DMat4::IDENTITY,
            true,
            viewport,
            cursor,
            &mut best,
        );
        if let Some(length) = best.1 {
            return TextSuggestion::Length(length);
        }
    }
    if let Some(hit) = ingetrazo_core::screen_pick::hover_face_placement(scene, &ctx.ray) {
        let face = &hit.mesh.faces[hit.face];
        let mut normal = DVec3::ZERO;
        for i in 0..face.vertices.len() {
            // The pick index transforms triangles, but returns the prototype Face.
            let a = hit.mesh.vertices[face.vertices[i]]
                .position
                .as_vec3()
                .as_dvec3();
            let b = hit.mesh.vertices[face.vertices[(i + 1) % face.vertices.len()]]
                .position
                .as_vec3()
                .as_dvec3();
            normal.x += (a.y - b.y) * (a.z + b.z);
            normal.y += (a.z - b.z) * (a.x + b.x);
            normal.z += (a.x - b.x) * (a.y + b.y);
        }
        return TextSuggestion::Area(0.5 * normal.as_vec3().length() as f64);
    }
    TextSuggestion::Coordinates
}
