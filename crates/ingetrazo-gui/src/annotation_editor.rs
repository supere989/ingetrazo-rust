//! Source Select double-click annotation text commands and buffered native dialog.
use crate::IngeTrazoApp;
use ingetrazo_tools::base::{AnnotationEditTarget, TextCreationRequest};
#[derive(Clone, Debug)]
pub enum Target {
    Existing(AnnotationEditTarget),
    Create(TextCreationRequest),
}
#[derive(Clone, Debug)]
pub struct Editor {
    pub target: Target,
    pub text: String,
    pub measured: Option<String>,
    focus_pending: bool,
}
impl IngeTrazoApp {
    pub(crate) fn format_text_suggestion(
        &self,
        suggestion: ingetrazo_tools::base::TextSuggestion,
        point: glam::DVec3,
    ) -> Result<String, String> {
        // Model units are global even while an edit exposes a local Scene.
        let units_scene = self
            .component_edit_stack
            .first()
            .map(|s| &s.original_scene)
            .unwrap_or(&self.scene);
        let (unit, precision) = crate::annotation_plan::label_units(units_scene)?;
        match suggestion {
            ingetrazo_tools::base::TextSuggestion::Length(length) => {
                ingetrazo_render::annotations::format_length(length, unit, precision)
            }
            ingetrazo_tools::base::TextSuggestion::Area(area) => {
                let (scaled, suffix) = match unit {
                    "m" => (area, "m"),
                    "cm" => (area * 1e4, "cm"),
                    "mm" => (area * 1e6, "mm"),
                    u if u.starts_with("ft") => (area / (0.3048 * 0.3048), "ft"),
                    _ => (area / (0.0254 * 0.0254), "in"),
                };
                let precision =
                    usize::try_from(precision.max(0)).map_err(|_| "Text precision overflow")?;
                if precision > 100 {
                    return Err("Text precision exceeds supported admission".into());
                }
                Ok(format!("{:.*} {}²", precision, scaled, suffix))
            }
            ingetrazo_tools::base::TextSuggestion::Coordinates => {
                Ok(format!("({:.2}, {:.2}, {:.2})", point.x, point.y, point.z))
            }
        }
    }

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
            target: Target::Existing(target),
            text,
            measured,
            focus_pending: true,
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
        let target = editor.target.clone();
        let entered = editor
            .text
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
            .to_owned();
        match target {
            Target::Create(draft) => {
                if entered.is_empty() {
                    self.cancel_annotation_edit();
                    return Ok(false);
                }
                if !draft.anchor.is_finite() || !draft.offset.is_finite() {
                    return Err("Text placement must be finite".into());
                }
                let label =
                    ingetrazo_core::textlabel::TextLabel::new(draft.anchor, draft.offset, entered);
                self.save_undo_state();
                self.undo_stack
                    .last_mut()
                    .unwrap()
                    .history_annotation_create = Some(label.history_id);
                self.scene.text_labels.push(label);
                self.active_tool.finish_text_creation();
            }
            Target::Existing(AnnotationEditTarget::Label(token)) => {
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
            Target::Existing(AnnotationEditTarget::Dimension(token)) => {
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
    pub fn begin_text_creation(&mut self, draft: TextCreationRequest) {
        self.annotation_editor = Some(Editor {
            text: draft.suggestion.clone(),
            target: Target::Create(draft),
            measured: None,
            focus_pending: true,
        });
    }
    pub fn cancel_annotation_edit(&mut self) {
        if self
            .annotation_editor
            .as_ref()
            .is_some_and(|e| matches!(e.target, Target::Create(_)))
        {
            self.active_tool.finish_text_creation();
        }
        self.annotation_editor = None;
    }
}
pub(crate) fn render(app: &mut IngeTrazoApp, ctx: &egui::Context) {
    let Some(editor) = app.annotation_editor.as_mut() else {
        return;
    };
    let label = matches!(
        editor.target,
        Target::Existing(AnnotationEditTarget::Label(_)) | Target::Create(_)
    );
    let mut accept = false;
    let mut cancel = false;
    egui::Modal::new(egui::Id::new("annotation_text_editor")).show(ctx, |ui| {
        ui.heading(if label { "Text" } else { "Dimension" });
        ui.label(if label {
            "Label text:"
        } else {
            "Dimension text (<> = measured value):"
        });
        let edit = if label {
            egui::TextEdit::multiline(&mut editor.text).desired_rows(4)
        } else {
            egui::TextEdit::singleline(&mut editor.text)
        };
        let response = ui.add(
            edit.id(egui::Id::new("annotation_editor_text"))
                .desired_width(320.),
        );
        if editor.focus_pending {
            response.request_focus();
            editor.focus_pending = false;
        }
        if !label
            && (response.has_focus() || response.lost_focus())
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
        {
            accept = true;
        }
        ui.horizontal(|ui| {
            accept |= ui.button("OK").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
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
    fn text_creation_real_modal_accept_cancel_and_live_selection_history() {
        use ingetrazo_core::{math::Ray, scene::SelectionItem, textlabel::TextLabel};
        use ingetrazo_tools::base::{ToolContext, ToolType};
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.text_labels.push(TextLabel::new(
            glam::DVec3::ZERO,
            glam::DVec3::Y,
            "existing",
        ));
        a.scene.select(SelectionItem::TextLabel(0));
        a.set_tool(ToolType::Text);
        let input = |p| ToolContext::new(p, Ray::new(p + glam::DVec3::Z, -glam::DVec3::Z));
        let open = |a: &mut IngeTrazoApp| {
            a.dispatch_tool_click(&input(glam::DVec3::ZERO));
            a.dispatch_tool_move(&input(glam::DVec3::X));
            a.dispatch_tool_click(&input(glam::DVec3::X));
        };
        let original = a.scene.clone();
        open(&mut a);
        assert_eq!(a.scene, original);
        assert!(a.undo_stack.is_empty());
        assert_eq!(a.active_tool.snap_anchor(), Some(glam::DVec3::ZERO));
        let ctx = egui::Context::default();
        let frame = |a: &mut IngeTrazoApp, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| crate::viewport::render_viewport(a, ui));
                    render(a, ctx);
                },
            )
        };
        frame(&mut a, vec![]);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::new("annotation_editor_text"))
        );
        frame(
            &mut a,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(a.annotation_editor.is_none());
        assert!(a.active_tool.snap_anchor().is_none());
        assert_eq!(a.scene, original);
        assert!(a.undo_stack.is_empty());
        open(&mut a);
        a.annotation_editor.as_mut().unwrap().text = "authored".into();
        frame(&mut a, vec![]);
        frame(
            &mut a,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(a.annotation_editor.is_some());
        assert!(a.annotation_editor.as_ref().unwrap().text.contains('\n'));
        a.annotation_editor.as_mut().unwrap().text = "  authored\nlabel  ".into();
        let out = frame(&mut a, vec![]);
        let ok = out
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Text(t) = &s.shape {
                    (t.galley.job.text == "OK").then_some(t.pos + t.galley.rect.center().to_vec2())
                } else {
                    None
                }
            })
            .expect("actual OK button text");
        let pointer = |pressed| egui::Event::PointerButton {
            pos: ok,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut a, vec![egui::Event::PointerMoved(ok), pointer(true)]);
        frame(&mut a, vec![pointer(false)]);
        assert!(a.annotation_editor.is_none());
        assert!(a.active_tool.snap_anchor().is_none());
        assert_eq!(a.scene.text_labels.len(), 2);
        assert_eq!(a.scene.text_labels[1].text, "authored\nlabel");
        assert_eq!(a.undo_stack.len(), 1);
        assert!(a.scene.selection.contains(&SelectionItem::TextLabel(0)));
        a.scene.select(SelectionItem::TextLabel(1));
        a.undo();
        assert_eq!(a.scene.text_labels.len(), 1);
        assert_eq!(a.scene.selection.len(), 1);
        assert!(a.scene.selection.contains(&SelectionItem::TextLabel(0)));
        a.redo();
        assert_eq!(a.scene.text_labels.len(), 2);
        assert_eq!(a.scene.selection.len(), 1);
    }

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
    #[test]
    fn actual_modal_owns_viewport_escape_navigation_and_dimension_enter() {
        use ingetrazo_tools::base::{Tool, ToolContext, ToolType};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        #[derive(Debug)]
        struct Count(Arc<AtomicUsize>);
        impl Tool for Count {
            fn tool_type(&self) -> ToolType {
                ToolType::Move
            }
            fn name(&self) -> &'static str {
                "modal witness"
            }
            fn on_move(&mut self, _: &mut ingetrazo_core::scene::Scene, _: &ToolContext) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_click(&mut self, _: &mut ingetrazo_core::scene::Scene, _: &ToolContext) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_release(
                &mut self,
                _: &mut ingetrazo_core::scene::Scene,
                _: &ToolContext,
                _: f64,
            ) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_cancel(&mut self, _: &mut ingetrazo_core::scene::Scene) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_control_state(&mut self, _: &mut ingetrazo_core::scene::Scene, _: bool) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene
            .dimensions
            .push(ingetrazo_core::dimensions::Dimension::new(
                glam::DVec3::ZERO,
                glam::DVec3::X,
                glam::DVec3::Y,
            ));
        a.begin_annotation_edit(AnnotationEditTarget::Dimension(a.scene.dimensions[0].id))
            .unwrap();
        a.annotation_editor.as_mut().unwrap().text = "authored".into();
        let count = Arc::new(AtomicUsize::new(0));
        a.active_tool = Box::new(Count(count.clone()));
        a.active_tool_type = ToolType::Move;
        a.navigation.mode = None;
        assert!(a.navigation_suspended());
        let eye = a.camera.eye();
        let ctx = egui::Context::default();
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let frame = |a: &mut IngeTrazoApp, events: Vec<egui::Event>, editor: bool| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| crate::viewport::render_viewport(a, ui));
                    if editor {
                        render(a, ctx);
                    }
                },
            );
        };
        frame(
            &mut a,
            vec![
                egui::Event::PointerMoved(egui::pos2(100., 100.)),
                egui::Event::PointerButton {
                    pos: egui::pos2(100., 100.),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos: egui::pos2(120., 120.),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                key(egui::Key::W),
                key(egui::Key::Escape),
            ],
            false,
        );
        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert_eq!(a.camera.eye(), eye);
        assert!(a.annotation_editor.is_some());
        assert!(a.undo_stack.is_empty());
        a.navigation.activate(crate::navigation::Mode::FirstPerson);
        frame(&mut a, vec![key(egui::Key::W)], false);
        frame(&mut a, vec![], false);
        assert_eq!(a.camera.eye(), eye);
        assert_eq!(count.load(Ordering::Relaxed), 0);
        a.navigation.mode = None;
        frame(&mut a, vec![], true);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(egui::Id::new("annotation_editor_text"))
        );
        frame(&mut a, vec![key(egui::Key::Enter)], true);
        assert!(a.annotation_editor.is_none());
        assert_eq!(
            a.scene.dimensions[0].text_override.as_deref(),
            Some("authored")
        );
        assert_eq!(a.undo_stack.len(), 1);
        assert_eq!(count.load(Ordering::Relaxed), 0);
        a.scene
            .text_labels
            .push(ingetrazo_core::textlabel::TextLabel::new(
                glam::DVec3::ZERO,
                glam::DVec3::Y,
                "old",
            ));
        a.begin_annotation_edit(AnnotationEditTarget::Label(
            a.scene.text_labels[0].history_id,
        ))
        .unwrap();
        frame(&mut a, vec![], true);
        frame(&mut a, vec![key(egui::Key::Enter)], true);
        assert!(
            a.annotation_editor.is_some(),
            "multiline Enter must not accept"
        );
        assert_eq!(a.undo_stack.len(), 1);
        frame(&mut a, vec![key(egui::Key::Escape)], true);
        assert!(a.annotation_editor.is_none());
        assert_eq!(a.scene.text_labels[0].text, "old");
        assert_eq!(a.undo_stack.len(), 1);
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn actual_select_double_click_and_escape_same_frame_never_cancel_background_tool() {
        use ingetrazo_tools::{
            base::{AnnotationHits, Tool, ToolContext, ToolType},
            select::SelectTool,
        };
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        #[derive(Debug)]
        struct Observed {
            select: SelectTool,
            cancel: Arc<AtomicUsize>,
        }
        impl Tool for Observed {
            fn tool_type(&self) -> ToolType {
                ToolType::Select
            }
            fn name(&self) -> &'static str {
                "Select"
            }
            fn wants_annotation_hits(&self) -> bool {
                true
            }
            fn note_annotation_hits(&mut self, h: AnnotationHits) {
                self.select.note_annotation_hits(h);
            }
            fn note_annotation_leader_hit(&mut self, h: Option<u64>) {
                self.select.note_annotation_leader_hit(h);
            }
            fn note_pick_viewport(
                &mut self,
                v: ingetrazo_core::screen_pick::PixelViewport,
                e: glam::DVec3,
                r: u64,
            ) {
                self.select.note_pick_viewport(v, e, r);
            }
            fn annotation_edit_target(
                &mut self,
                s: &ingetrazo_core::scene::Scene,
                c: &ToolContext,
            ) -> Option<AnnotationEditTarget> {
                self.select.annotation_edit_target(s, c)
            }
            fn on_click(&mut self, s: &mut ingetrazo_core::scene::Scene, c: &ToolContext) {
                self.select.on_click(s, c);
            }
            fn on_double_click(&mut self, s: &mut ingetrazo_core::scene::Scene, c: &ToolContext) {
                self.select.on_double_click(s, c);
            }
            fn on_cancel(&mut self, s: &mut ingetrazo_core::scene::Scene) {
                self.cancel.fetch_add(1, Ordering::Relaxed);
                self.select.on_cancel(s);
            }
        }
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.camera.target = glam::DVec3::ZERO;
        a.camera.pitch = std::f64::consts::FRAC_PI_2;
        a.camera.yaw = 0.;
        a.camera.distance = 10.;
        a.camera.perspective = false;
        a.scene
            .text_labels
            .push(ingetrazo_core::textlabel::TextLabel::new(
                glam::DVec3::ZERO,
                glam::DVec3::ZERO,
                "source glyph",
            ));
        let count = Arc::new(AtomicUsize::new(0));
        a.active_tool = Box::new(Observed {
            select: SelectTool::new(),
            cancel: count.clone(),
        });
        a.active_tool_type = ToolType::Select;
        let ctx = egui::Context::default();
        let input = |time, events| egui::RawInput {
            time: Some(time),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input(0., vec![]), |ctx| {
            egui::CentralPanel::default()
                .show(ctx, |ui| crate::viewport::render_viewport(&mut a, ui));
        });
        let rect = a.native_gpu_viewport_rect.unwrap();
        let pixel = rect.center() + egui::vec2(8., -8.);
        let pointer = |pressed| egui::Event::PointerButton {
            pos: pixel,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = ctx.run(
            input(
                0.1,
                vec![
                    egui::Event::PointerMoved(pixel),
                    pointer(true),
                    pointer(false),
                ],
            ),
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| crate::viewport::render_viewport(&mut a, ui));
            },
        );
        assert!(a.annotation_editor.is_none());
        assert!(a.undo_stack.is_empty());
        let _ = ctx.run(
            input(
                0.2,
                vec![
                    pointer(true),
                    pointer(false),
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            ),
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| crate::viewport::render_viewport(&mut a, ui));
                assert!(
                    a.annotation_editor.is_some(),
                    "actual Select double-click must open the editor before modal Escape"
                );
                render(&mut a, ctx);
            },
        );
        assert!(a.annotation_editor.is_none());
        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert!(a.undo_stack.is_empty());
        assert!(a.redo_stack.is_empty());
        assert_eq!(a.scene.text_labels[0].text, "source glyph");
    }
}

/// AddTextLabel owns only membership of its label, not the user's live selection.
pub(crate) fn preserve_creation_selection(
    current: &ingetrazo_core::scene::Scene,
    target: &mut ingetrazo_core::scene::Scene,
) {
    let (selection, order) = creation_selection(current, &target.text_labels);
    target.selection = selection;
    target.selection_order = order;
}
pub(crate) fn creation_selection(
    current: &ingetrazo_core::scene::Scene,
    labels: &[ingetrazo_core::textlabel::TextLabel],
) -> (
    std::collections::HashSet<ingetrazo_core::scene::SelectionItem>,
    Vec<ingetrazo_core::scene::SelectionItem>,
) {
    use ingetrazo_core::scene::SelectionItem;
    let mut fallback = None;
    let mut remap = |item: &SelectionItem| match item {
        SelectionItem::TextLabel(index) => {
            let token = current.text_labels.get(*index)?.history_id;
            if labels.get(*index).is_some_and(|t| t.history_id == token) {
                Some(SelectionItem::TextLabel(*index))
            } else {
                let index = fallback.get_or_insert_with(|| {
                    labels
                        .iter()
                        .enumerate()
                        .map(|(i, t)| (t.history_id, i))
                        .collect::<std::collections::HashMap<_, _>>()
                });
                index.get(&token).copied().map(SelectionItem::TextLabel)
            }
        }
        other => Some(other.clone()),
    };
    let selection = current.selection.iter().filter_map(&mut remap).collect();
    let order = current.selection_order.iter().filter_map(remap).collect();
    (selection, order)
}

/// Global label membership can cross an edit context; geometry selection cannot.
pub(crate) fn preserve_creation_label_selection(
    current: &ingetrazo_core::scene::Scene,
    target: &mut ingetrazo_core::scene::Scene,
) {
    use ingetrazo_core::scene::SelectionItem;
    let (selection, order) = creation_selection(current, &target.text_labels);
    target
        .selection
        .retain(|s| !matches!(s, SelectionItem::TextLabel(_)));
    target
        .selection_order
        .retain(|s| !matches!(s, SelectionItem::TextLabel(_)));
    target.selection.extend(
        selection
            .into_iter()
            .filter(|s| matches!(s, SelectionItem::TextLabel(_))),
    );
    target.selection_order.extend(
        order
            .into_iter()
            .filter(|s| matches!(s, SelectionItem::TextLabel(_))),
    );
}
