//! Source Select double-click annotation text commands and buffered native dialog.
use crate::IngeTrazoApp;
use ingetrazo_tools::base::AnnotationEditTarget;
#[derive(Clone, Debug)]
pub struct Editor {
    pub target: AnnotationEditTarget,
    pub text: String,
    pub measured: Option<String>,
}
impl IngeTrazoApp {
    pub fn begin_annotation_edit(&mut self, target: AnnotationEditTarget) -> Result<(), String> {
        let (text, measured) = match target {
            AnnotationEditTarget::Label(id) => {
                let t = self
                    .scene
                    .text_labels
                    .iter()
                    .find(|t| t.history_id == id)
                    .ok_or("Text label no longer resolves")?;
                if !self.scene.is_layer_selectable(t.layer.as_deref()) {
                    return Err("Text label is hidden or locked".into());
                }
                (t.text.clone(), None)
            }
            AnnotationEditTarget::Dimension(id) => {
                let d = self
                    .scene
                    .dimensions
                    .iter()
                    .find(|d| d.id == id)
                    .ok_or("Dimension no longer resolves")?;
                if !self.scene.is_layer_selectable(d.layer.as_deref()) {
                    return Err("Dimension is hidden or locked".into());
                }
                let measured = self.formatted_dimension_value(d)?;
                (
                    d.text_override
                        .clone()
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| measured.clone()),
                    Some(measured),
                )
            }
        };
        self.annotation_editor = Some(Editor {
            target,
            text,
            measured,
        });
        Ok(())
    }
    pub fn formatted_dimension_value(
        &self,
        d: &ingetrazo_core::dimensions::Dimension,
    ) -> Result<String, String> {
        let raw = self.scene.dimension_style();
        let unit = raw["units"].as_str().unwrap_or("m");
        let decimals = i32::try_from(raw["decimals"].as_i64().unwrap_or(2))
            .map_err(|_| "Dimension precision overflow")?;
        let mut live = d.clone();
        (live.start, live.end) = self.scene.dimension_points(d);
        ingetrazo_render::annotations::format_length(live.length(), unit, decimals)
    }
    /// The native dialog's OK callback: preflight identity/layer before one command.
    pub fn accept_annotation_edit(&mut self) -> Result<bool, String> {
        let editor = self
            .annotation_editor
            .as_ref()
            .ok_or("No annotation edit is pending")?;
        let target = editor.target;
        let entered = editor
            .text
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            .to_owned();
        match target {
            AnnotationEditTarget::Label(token) => {
                let id = self
                    .scene
                    .text_labels
                    .iter()
                    .position(|t| t.history_id == token)
                    .ok_or("Text label no longer resolves")?;
                let t = &self.scene.text_labels[id];
                if !self.scene.is_layer_selectable(t.layer.as_deref()) {
                    return Err("Text label is hidden or locked".into());
                }
                if entered.is_empty() || entered == t.text {
                    self.annotation_editor = None;
                    return Ok(false);
                }
                self.save_undo_state();
                self.undo_stack.last_mut().unwrap().history_annotation_edit = true;
                self.scene.text_labels[id].text = entered;
            }
            AnnotationEditTarget::Dimension(token) => {
                let id = self
                    .scene
                    .dimensions
                    .iter()
                    .position(|d| d.id == token)
                    .ok_or("Dimension no longer resolves")?;
                let d = &self.scene.dimensions[id];
                if !self.scene.is_layer_selectable(d.layer.as_deref()) {
                    return Err("Dimension is hidden or locked".into());
                }
                let new = if entered.is_empty()
                    || entered == "<>"
                    || editor.measured.as_deref() == Some(entered.as_str())
                {
                    None
                } else {
                    Some(entered)
                };
                if new == d.text_override {
                    self.annotation_editor = None;
                    return Ok(false);
                }
                self.save_undo_state();
                self.undo_stack.last_mut().unwrap().history_annotation_edit = true;
                self.scene.dimensions[id].text_override = new;
            }
        }
        self.scene.bump_version();
        self.bump_model_revision();
        self.annotation_editor = None;
        Ok(true)
    }
    pub fn cancel_annotation_edit(&mut self) {
        self.annotation_editor = None;
    }
}
pub(crate) fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(editor) = app.annotation_editor.as_mut() else {
        return;
    };
    let label = matches!(editor.target, AnnotationEditTarget::Label(_));
    let mut open = true;
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new(if label { "Text" } else { "Dimension" })
        .id(egui::Id::new("annotation_text_editor"))
        .collapsible(false)
        .resizable(label)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(if label {
                "Label text:"
            } else {
                "Dimension text (<> = measured value):"
            });
            if label {
                ui.text_edit_multiline(&mut editor.text);
            } else {
                ui.text_edit_singleline(&mut editor.text);
            }
            ui.horizontal(|ui| {
                accept = ui.button("OK").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
    if cancel || !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.cancel_annotation_edit();
    } else if accept {
        if let Err(error) = app.accept_annotation_edit() {
            app.status_message = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locked_target_refuses_before_history_and_preserves_redo() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        let t = ingetrazo_core::textlabel::TextLabel::new(glam::DVec3::ZERO, glam::DVec3::Y, "old");
        a.scene.text_labels.push(t);
        let id = a.scene.text_labels[0].history_id;
        a.begin_annotation_edit(AnnotationEditTarget::Label(id))
            .unwrap();
        a.annotation_editor.as_mut().unwrap().text = "new".into();
        a.scene.text_labels[0].layer = Some("locked".into());
        let mut layer = ingetrazo_core::layers::Layer::new("locked");
        layer.locked = true;
        a.scene.layers.push(layer);
        a.redo_stack.push(a.scene.clone());
        let before = a.scene.clone();
        assert!(a.accept_annotation_edit().is_err());
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
        assert_eq!(a.redo_stack.len(), 1);
    }
    #[test]
    fn style_is_nonhistory_dirty_document_data_and_refusal_is_atomic() {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.mark_document_clean();
        assert!(!a.document_dirty());
        let before = a.scene.clone();
        let mut c = a.dimension_style_controls();
        c.font_size = 17;
        a.apply_dimension_style(c.clone()).unwrap();
        assert!(a.undo_stack.is_empty());
        assert!(a.document_dirty());
        assert!(a.scene.native_document.is_null());
        assert_eq!(a.scene.dimension_style()["font_size"], 17);
        let accepted = a.scene.clone();
        c.base_step_mm = f64::INFINITY;
        assert!(a.apply_dimension_style(c).is_err());
        assert_eq!(a.scene, accepted);
        let mut restored = before;
        crate::document_presentation::preserve(&a.scene, &mut restored);
        assert_eq!(restored.dimension_style(), a.scene.dimension_style());
        let p = std::env::temp_dir().join(format!("annotation-style-{}.igz", uuid::Uuid::new_v4()));
        ingetrazo_formats::igz::save_igz(&a.scene, &p).unwrap();
        let loaded = ingetrazo_formats::igz::load_igz(&p).unwrap();
        std::fs::remove_file(p).unwrap();
        assert_eq!(loaded.dimension_style(), a.scene.dimension_style());
    }
}
