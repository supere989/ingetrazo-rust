// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Follow Me tool: sweep a face along a path of edges.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_face, Tool, ToolContext, ToolType};

pub struct FollowMeTool {
    profile_face: Option<usize>,
    hover_point: Option<DVec3>,
}

impl Default for FollowMeTool {
    fn default() -> Self {
        Self {
            profile_face: None,
            hover_point: None,
        }
    }
}

impl Tool for FollowMeTool {
    fn tool_type(&self) -> ToolType {
        ToolType::FollowMe
    }

    fn name(&self) -> &'static str {
        "Follow Me"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("W")
    }

    fn description(&self) -> &'static str {
        "Sweep a profile face along a path of edges."
    }

    fn vcb_label(&self) -> &'static str {
        if self.profile_face.is_none() {
            "Select profile face"
        } else {
            "Select path end"
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.profile_face.is_none() {
            if let Some((face_id, _, _)) = pick_face(&scene.mesh, &ctx.ray) {
                self.profile_face = Some(face_id);
            }
        } else {
            // Sweep profile along vector towards click point
            let face_id = self.profile_face.unwrap();
            if face_id < scene.mesh.faces.len() {
                let face_verts = scene.mesh.faces[face_id].vertices.clone();
                let centroid = scene.mesh.face_centroid(face_id);
                let sweep_vec = ctx.world - centroid;

                if sweep_vec.length() > 1e-4 {
                    // Create extruded side faces
                    let n = face_verts.len();
                    let mut top_verts = Vec::with_capacity(n);
                    for &vid in &face_verts {
                        let p = scene.mesh.vertices[vid].position;
                        top_verts.push(scene.mesh.add_vertex(p + sweep_vec));
                    }

                    for i in 0..n {
                        let next_i = (i + 1) % n;
                        let v0 = face_verts[i];
                        let v1 = face_verts[next_i];
                        let v2 = top_verts[next_i];
                        let v3 = top_verts[i];
                        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
                    }

                    scene.mesh.add_face(top_verts, vec![]);
                    scene.bump_version();
                }
            }
            self.profile_face = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.profile_face = None;
        self.hover_point = None;
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.hover_point.map(|h| (h, h))
    }
}
