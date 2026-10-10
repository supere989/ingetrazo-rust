// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use ingetrazo_core::materials::Material;
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{pick_face, Tool, ToolContext, ToolType};

/// Interactive Paint tool for applying materials and colours to faces and groups.
#[derive(Debug, Clone)]
pub struct PaintTool {
    pub active_material: String,
    pub active_color: [f32; 4],
    pub eyedropper_mode: bool,
}

impl Default for PaintTool {
    fn default() -> Self {
        Self {
            active_material: "Default".to_string(),
            active_color: [0.8, 0.8, 0.8, 1.0],
            eyedropper_mode: false,
        }
    }
}

impl PaintTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_material(name: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            active_material: name.into(),
            active_color: color,
            eyedropper_mode: false,
        }
    }

    /// Apply active material to a face in scene.
    pub fn paint_face(&self, scene: &mut Scene, face_id: usize) {
        if face_id >= scene.mesh.faces.len() {
            return;
        }

        // Ensure material is in scene.materials
        if !scene.materials.contains_key(&self.active_material) {
            scene.materials.insert(
                self.active_material.clone(),
                Material::new(self.active_material.clone(), self.active_color),
            );
        }

        let face = &mut scene.mesh.faces[face_id];
        face.attrs.insert(
            "material".to_string(),
            serde_json::Value::String(self.active_material.clone()),
        );
        face.attrs.insert(
            "color".to_string(),
            serde_json::to_value(self.active_color).unwrap(),
        );

        scene.bump_version();
    }

    /// Eyedropper sample material from a face.
    pub fn sample_face(&mut self, scene: &Scene, face_id: usize) {
        if face_id >= scene.mesh.faces.len() {
            return;
        }

        let face = &scene.mesh.faces[face_id];
        if let Some(serde_json::Value::String(mat_name)) = face.attrs.get("material") {
            self.active_material = mat_name.clone();
            if let Some(mat) = scene.materials.get(mat_name) {
                self.active_color = mat.color;
            }
        }
        if let Some(color_val) = face.attrs.get("color") {
            if let Ok(col) = serde_json::from_value::<[f32; 4]>(color_val.clone()) {
                self.active_color = col;
            }
        }
    }
}

impl Tool for PaintTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Paint
    }

    fn name(&self) -> &'static str {
        "Paint"
    }


    fn shortcut(&self) -> Option<&'static str> {
        Some("B")
    }

    fn description(&self) -> &'static str {
        "Assign a material or colour to a face or group."
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.eyedropper_mode = ctx.alt_held;
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.eyedropper_mode = ctx.alt_held;

        let hit_face = pick_face(&scene.mesh, &ctx.ray).map(|(id, _, _)| id);
        let Some(fid) = hit_face else { return };

        if self.eyedropper_mode {
            self.sample_face(scene, fid);
            self.eyedropper_mode = false;
        } else {
            // If the face is selected, paint all selected faces
            let is_in_selection = scene.is_selected(&SelectionItem::Face(fid));
            if is_in_selection {
                let selected_faces: Vec<usize> = scene
                    .selection
                    .iter()
                    .filter_map(|item| match item {
                        SelectionItem::Face(id) => Some(*id),
                        _ => None,
                    })
                    .collect();

                for f in selected_faces {
                    self.paint_face(scene, f);
                }
            } else {
                self.paint_face(scene, fid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_paint_tool_apply_and_sample() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let f0 = scene.mesh.add_face(vec![v0, v1, v2], vec![]).unwrap();

        let mut tool = PaintTool::with_material("Brick", [0.8, 0.2, 0.2, 1.0]);
        let ray = Ray::new(DVec3::new(0.5, 0.2, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.2, 0.0), ray);

        // Click to paint face
        tool.on_click(&mut scene, &ctx);

        assert!(scene.materials.contains_key("Brick"));
        let face_mat = scene.mesh.faces[f0].attrs.get("material").unwrap().as_str().unwrap();
        assert_eq!(face_mat, "Brick");

        // Now change tool material, then use eyedropper to sample f0
        tool.active_material = "Wood".to_string();
        let mut ctx_alt = ctx;
        ctx_alt.alt_held = true;
        tool.on_click(&mut scene, &ctx_alt);

        assert_eq!(tool.active_material, "Brick");
        assert_eq!(tool.active_color, [0.8, 0.2, 0.2, 1.0]);
    }
}

