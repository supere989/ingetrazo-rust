use super::*;
use ingetrazo_core::render_blender::BlenderDocument;
use ingetrazo_tools::base::{ToolContext, ToolType};
#[derive(Clone, Copy, Debug)]
pub(super) enum Pick {
    New(LightKind),
    Move(usize),
    Aim(usize),
}
impl IngeTrazoApp {
    fn blender_document_raw(&self) -> serde_json::Value {
        let scene = self
            .component_edit_stack
            .first()
            .map_or(&self.scene, |s| &s.original_scene);
        scene
            .plugin_data
            .get("render_blender")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
    pub fn sync_blender_document(&mut self) {
        let raw = self.blender_document_raw();
        if self.blender_state.cached_document.as_ref() == Some(&raw)
            && self.blender_state.cached_document_id == Some(self.clipboard_document)
        {
            return;
        }
        let d = BlenderDocument::from_json(&raw);
        let s = &mut self.blender_state;
        s.ambience = d.ambience;
        s.sun_scale = d.sun_scale;
        s.lights = d.lights;
        s.cached_document = Some(raw);
        s.cached_document_id = Some(self.clipboard_document);
        s.pending_document_controls = false;
        s.pick = None;
        s.picked_light = None;
        s.selected_light = s.selected_light.filter(|i| *i < s.lights.len());
    }
    pub fn blender_document_controls(&self) -> BlenderDocument {
        let s = &self.blender_state;
        BlenderDocument {
            ambience: s.ambience,
            sun_scale: s.sun_scale,
            lights: s.lights.clone(),
        }
    }
    pub fn set_blender_document(&mut self, value: serde_json::Value) -> Result<(), String> {
        let current = self.blender_document_raw();
        if current == value {
            return Ok(());
        }
        let target = self
            .component_edit_stack
            .first()
            .map_or(&self.scene, |s| &s.original_scene);
        if !(target.plugin_data.is_null() || target.plugin_data.is_object()) {
            return Err("Document plugin data is not an object".into());
        }
        self.undo_stack
            .try_reserve(1)
            .map_err(|_| "Cannot reserve document history")?;
        let mut before = target.clone();
        before.camera = self.camera.clone();
        self.push_prepared_undo_snapshot(before);
        let target = if let Some(s) = self.component_edit_stack.first_mut() {
            &mut s.original_scene
        } else {
            &mut self.scene
        };
        if target.plugin_data.is_null() {
            target.plugin_data = serde_json::json!({});
        }
        target.plugin_data["render_blender"] = value;
        target.bump_view_version();
        self.bump_image_presentation_revision();
        self.blender_state.cached_document = None;
        self.sync_blender_document();
        Ok(())
    }
    pub(super) fn commit_blender_document_controls(&mut self) -> bool {
        self.blender_state.pending_document_controls = false;
        let value = self.blender_document_controls().to_json();
        if let Err(e) = self.set_blender_document(value) {
            self.status_message = e;
            self.blender_state.cached_document = None;
            self.sync_blender_document();
            return false;
        }
        true
    }
    pub(super) fn begin_blender_pick(&mut self, pick: Pick) {
        self.set_tool(ToolType::Select);
        self.blender_state.pick = Some(pick);
        self.blender_state.picked_light = None;
        self.status_message = match pick {
            Pick::New(_) => "Click where the light goes — Esc cancels",
            Pick::Move(_) => "Click the light's new place — Esc cancels",
            Pick::Aim(_) => "Click the point the spot looks at — Esc cancels",
        }
        .into();
    }
    pub fn add_blender_light_at(
        &mut self,
        kind: LightKind,
        point: glam::DVec3,
        on_face: bool,
    ) -> Result<(), String> {
        if !point.is_finite() {
            return Err("Light position must be finite".into());
        }
        self.sync_blender_document();
        let mut d = self.blender_document_controls();
        let mut lt = BlenderLight::default();
        lt.kind = kind;
        lt.pos = [
            point.x,
            point.y,
            point.z
                + if on_face && kind == LightKind::Point {
                    0.1
                } else {
                    0.
                },
        ];
        lt.power = if kind == LightKind::Spot { 1500. } else { 400. };
        lt.name.clear();
        d.lights.push(lt);
        self.set_blender_document(d.to_json())?;
        self.blender_state.selected_light = Some(d.lights.len() - 1);
        Ok(())
    }
    /// Cancel rolls back the editing mesh; document-global light commands survive.
    pub(crate) fn preserve_blender_cancel_state(&mut self) {
        let raw = self.blender_document_raw();
        let Some(session) = self.component_edit_stack.last_mut() else {
            return;
        };
        if session
            .cancel_scene
            .plugin_data
            .get("render_blender")
            .cloned()
            .unwrap_or(serde_json::Value::Null)
            == raw
        {
            return;
        }
        if session.cancel_scene.plugin_data.is_null() {
            session.cancel_scene.plugin_data = serde_json::json!({});
        }
        session.cancel_scene.plugin_data["render_blender"] = raw;
        let mut path = session.original_scene.history_context.clone();
        path.push(session.group_id);
        session.original_undo = self
            .undo_stack
            .iter()
            .filter(|e| !e.history_context.starts_with(&path))
            .cloned()
            .collect();
        session.original_redo = self
            .redo_stack
            .iter()
            .filter(|e| !e.history_context.starts_with(&path))
            .cloned()
            .collect();
    }
    pub(crate) fn blender_is_picking(&self) -> bool {
        self.blender_state.pick.is_some()
    }
    pub fn cancel_blender_pick(&mut self) -> bool {
        if self.blender_state.pick.take().is_some() {
            self.status_message.clear();
            true
        } else {
            false
        }
    }
    pub(crate) fn handle_blender_click(
        &mut self,
        ctx: &ToolContext,
        mouse: egui::Pos2,
        rect: egui::Rect,
        on_face: bool,
    ) -> bool {
        if let Some(pick) = self.blender_state.pick.take() {
            let point = ctx.world.as_vec3().as_dvec3();
            let result = match pick {
                Pick::New(kind) => self.add_blender_light_at(kind, point, on_face),
                Pick::Move(i) | Pick::Aim(i) => {
                    let mut d = self.blender_document_controls();
                    if let Some(lt) = d.lights.get_mut(i) {
                        if matches!(pick, Pick::Move(_)) {
                            lt.pos = point.to_array();
                        } else {
                            let dir = point - glam::DVec3::from_array(lt.pos);
                            if dir.length() > 1e-6 {
                                lt.dir = dir.to_array();
                            }
                        }
                        self.set_blender_document(d.to_json())
                    } else {
                        Err("The selected light no longer exists".into())
                    }
                }
            };
            self.status_message = result.err().unwrap_or_default();
            return true;
        }
        if self.active_tool_type != ToolType::Select {
            return false;
        }
        let mut best = None;
        let mut distance = 12.;
        for (i, lt) in self.blender_state.lights.iter().enumerate() {
            if let Some((p, _)) = ingetrazo_render::pipeline::project_point(
                glam::DVec3::from_array(lt.pos),
                &(self.camera.projection_matrix() * self.camera.view_matrix()),
                rect,
            ) {
                let d = p.distance(mouse);
                if d <= distance {
                    distance = d;
                    best = Some(i);
                }
            }
        }
        self.blender_state.picked_light = best.map(|i| (i, self.blender_document_raw()));
        self.blender_state.selected_light = best;
        if best.is_some() {
            self.scene.clear_selection();
            true
        } else {
            false
        }
    }
    pub(crate) fn delete_picked_blender_light(&mut self) -> bool {
        let Some((index, raw)) = self.blender_state.picked_light.take() else {
            return false;
        };
        if raw != self.blender_document_raw() {
            return true;
        }
        let mut d = self.blender_document_controls();
        if index < d.lights.len() {
            d.lights.remove(index);
            if let Err(e) = self.set_blender_document(d.to_json()) {
                self.status_message = e;
            }
        }
        true
    }
    pub(crate) fn draw_blender_lights(&self, painter: &egui::Painter, rect: egui::Rect) {
        for (i, lt) in self.blender_state.lights.iter().enumerate() {
            if let Some((p, _)) = ingetrazo_render::pipeline::project_point(
                glam::DVec3::from_array(lt.pos),
                &(self.camera.projection_matrix() * self.camera.view_matrix()),
                rect,
            ) {
                let color = if lt.on {
                    Color32::from_rgb(
                        (lt.color[0] * 255.).round() as u8,
                        (lt.color[1] * 255.).round() as u8,
                        (lt.color[2] * 255.).round() as u8,
                    )
                } else {
                    Color32::from_gray(150)
                };
                if lt.kind == LightKind::Spot {
                    let tip = glam::DVec3::from_array(lt.pos)
                        + glam::DVec3::from_array(lt.dir).normalize_or_zero() * 0.8;
                    if let Some((tip, _)) = ingetrazo_render::pipeline::project_point(
                        tip,
                        &(self.camera.projection_matrix() * self.camera.view_matrix()),
                        rect,
                    ) {
                        painter.extend(egui::Shape::dashed_line(
                            &[p, tip],
                            egui::Stroke::new(1.6, color.gamma_multiply(1. / 1.3)),
                            4.,
                            3.,
                        ));
                    }
                }
                painter.circle_filled(p, 6., color);
                painter.circle_stroke(p, 6., egui::Stroke::new(1.2, Color32::from_gray(40)));
                for j in 0..8 {
                    let a = j as f32 * std::f32::consts::FRAC_PI_4;
                    let d = egui::vec2(a.cos(), a.sin());
                    painter.line_segment(
                        [p + d * 8., p + d * 11.],
                        egui::Stroke::new(1.4, color.gamma_multiply(1. / 1.4)),
                    );
                }
                if self.blender_state.selected_light == Some(i) {
                    painter.circle_stroke(
                        p,
                        14.,
                        egui::Stroke::new(2., Color32::from_rgb(54, 137, 230)),
                    );
                    let name = if lt.name.is_empty() {
                        if lt.kind == LightKind::Spot {
                            "Spot"
                        } else {
                            "Point light"
                        }
                    } else {
                        &lt.name
                    };
                    painter.text(
                        p + egui::vec2(16., -10.),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{name} · {} W", lt.power as u32),
                        egui::FontId::proportional(11.),
                        Color32::from_gray(20),
                    );
                }
                if self
                    .blender_state
                    .picked_light
                    .as_ref()
                    .is_some_and(|(n, _)| *n == i)
                {
                    painter.circle_stroke(
                        p,
                        15.,
                        egui::Stroke::new(3., Color32::from_rgb(242, 115, 41)),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_lights_preserve_other_plugins_and_root_context_history() {
        use ingetrazo_core::{groups::Group, scene::SelectionItem};
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.plugin_data = serde_json::json!({"other":{"opaque":71}});
        a.scene.groups.push(Group::new("edit"));
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        let mesh = a.scene.mesh.clone();
        a.add_blender_light_at(LightKind::Point, glam::DVec3::Z, true)
            .unwrap();
        assert_eq!(a.scene.mesh, mesh);
        assert_eq!(
            a.component_edit_stack[0].original_scene.plugin_data["other"]["opaque"],
            71
        );
        assert_eq!(a.blender_state.lights.len(), 1);
        a.undo();
        a.sync_blender_document();
        assert!(a.blender_state.lights.is_empty());
        assert_eq!(a.scene.mesh, mesh);
        a.redo();
        a.sync_blender_document();
        assert_eq!(a.blender_state.lights.len(), 1);
        a.finish_component_edit();
        assert!(a.scene.plugin_data["render_blender"]["lights"].is_array());
    }
    #[test]
    fn picked_light_identity_and_invalid_plugin_refuse_without_mutation() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.add_blender_light_at(LightKind::Point, glam::DVec3::ZERO, false)
            .unwrap();
        a.blender_state.picked_light = Some((0, a.blender_document_raw()));
        a.scene.plugin_data["render_blender"]["lights"][0]["name"] = "changed".into();
        let before = a.scene.clone();
        assert!(a.delete_picked_blender_light());
        assert_eq!(a.scene, before);
        a.scene.plugin_data = 17.into();
        let before = a.scene.clone();
        let undo = a.undo_stack.clone();
        assert!(a.set_blender_document(serde_json::json!({})).is_err());
        assert_eq!(a.scene, before);
        assert_eq!(a.undo_stack, undo);
    }
}

#[cfg(test)]
mod cancel_tests {
    use super::*;
    #[test]
    fn cancel_keeps_global_lights_and_their_undo_without_working_geometry() {
        use ingetrazo_core::{groups::Group, scene::SelectionItem};
        let mut a = IngeTrazoApp::new().without_startup_figure_for_test();
        a.system_clipboard = None;
        a.scene.groups.push(Group::new("edit"));
        a.scene.select(SelectionItem::Group(0));
        a.begin_component_edit();
        a.save_undo_state();
        a.scene.mesh.add_vertex(glam::DVec3::X);
        a.add_blender_light_at(LightKind::Point, glam::DVec3::Z, false)
            .unwrap();
        a.cancel_component_edit();
        a.sync_blender_document();
        assert_eq!(a.blender_state.lights.len(), 1);
        assert!(a.scene.groups[0].mesh.vertices.is_empty());
        assert_eq!(a.undo_stack.len(), 1);
        a.undo();
        a.sync_blender_document();
        assert!(a.blender_state.lights.is_empty());
        a.redo();
        a.sync_blender_document();
        assert_eq!(a.blender_state.lights.len(), 1);
    }
    #[test]
    fn new_document_retires_same_payload_picks_and_pending_controls() {
        let mut a = IngeTrazoApp::new();
        a.sync_blender_document();
        a.blender_state.pick = Some(Pick::New(LightKind::Point));
        a.blender_state.pending_document_controls = true;
        a.clipboard_document = uuid::Uuid::new_v4();
        a.sync_blender_document();
        assert!(!a.blender_is_picking());
        assert!(!a.blender_state.pending_document_controls);
    }
}
