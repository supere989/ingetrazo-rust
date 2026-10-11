//! Selection-only release dispatcher: no model snapshot or history command.
use crate::IngeTrazoApp;
use ingetrazo_tools::base::{BoxSelection, ToolType};
impl IngeTrazoApp {
    pub fn dispatch_box_selection(&mut self, selection: &BoxSelection) -> Result<(), String> {
        if self.active_tool_type != ToolType::Select
            || self.paste_preview.is_some()
            || self.navigation.mode.is_some()
        {
            return Err("Box selection requires Select".into());
        }
        self.active_tool.box_select(selection, &mut self.scene)
    }
}
pub(crate) fn input(
    app: &mut IngeTrazoApp,
    ui: &egui::Ui,
    rect: egui::Rect,
    id: egui::Id,
    owns_pointer: bool,
) -> bool {
    let key = id.with("source_box_press");
    if !ui.is_enabled() || app.active_tool_type != ToolType::Select
        || app.paste_preview.is_some()
        || app.navigation.mode.is_some()
        || app.text3d_dialog.is_some()
    {
        ui.ctx().data_mut(|d| d.remove::<egui::Pos2>(key));
        return false;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.ctx().data_mut(|d| d.remove::<egui::Pos2>(key));
        return false;
    }
    let mut handled = false;
    // Release the egui input read lock before touching its temporary state.
    // Empty warm frames allocate nothing; only primary transitions are copied.
    let events = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| {
                if let egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                } = e
                {
                    Some((*pos, *pressed, *modifiers))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    });
    for (pos, pressed, modifiers) in events {
        if pressed {
            if owns_pointer && rect.contains(pos) && !modifiers.alt {
                ui.ctx().data_mut(|d| d.insert_temp(key, pos));
            }
        } else if let Some(start) = ui.ctx().data_mut(|d| d.remove_temp::<egui::Pos2>(key)) {
            if pos.distance(start) >= 4. {
                let local = |p: egui::Pos2| {
                    glam::DVec2::new(f64::from(p.x - rect.min.x), f64::from(p.y - rect.min.y))
                };
                let selection = BoxSelection {
                    viewport: ingetrazo_core::screen_pick::PixelViewport::new(
                        &app.camera,
                        f64::from(rect.width()),
                        f64::from(rect.height()),
                    ),
                    eye: app.camera.eye(),
                    start: local(start),
                    end: local(pos),
                    shift: modifiers.shift,
                    ctrl: modifiers.ctrl || modifiers.command,
                };
                if let Err(e) = app.dispatch_box_selection(&selection) {
                    app.status_message = e;
                }
                handled = true;
            }
        }
    }
    handled
}
pub(crate) fn paint(ui: &egui::Ui, rect: egui::Rect, id: egui::Id) {
    let Some(start) = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Pos2>(id.with("source_box_press")))
    else {
        return;
    };
    let Some(end) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if start.distance(end) < 4. {
        return;
    }
    let crossing = end.x < start.x;
    let color = if crossing {
        egui::Color32::from_rgb(60, 180, 90)
    } else {
        egui::Color32::from_rgb(60, 120, 220)
    };
    let box_rect = egui::Rect::from_two_pos(start, end);
    let painter = ui.painter_at(rect);
    painter.rect_filled(box_rect, 0., color.linear_multiply(0.12));
    painter.rect_stroke(box_rect, 0., egui::Stroke::new(1., color));
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DVec2, DVec3};
    fn app() -> IngeTrazoApp {
        let mut a = IngeTrazoApp::new();
        a.system_clipboard = None;
        a.scene.mesh.clear();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y]
            .map(|p| a.scene.mesh.add_vertex(p));
        a.scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        a.camera
            .set_standard_view(ingetrazo_core::camera::StandardView::Top);
        a.camera.perspective = false;
        a.camera.target = DVec3::new(0.5, 0.5, 0.);
        a.camera.distance = 10.;
        a
    }
    fn frame(ctx: &egui::Context, a: &mut IngeTrazoApp, events: Vec<egui::Event>) {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800., 600.),
        ));
        input.events = events;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |ui| crate::viewport::render_viewport(a, ui));
        });
    }
    fn event(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }
    #[test]
    fn actual_viewport_release_selects_without_model_history_and_filters_lock() {
        let mut a = app();
        let before = a.scene.mesh.clone();
        let ctx = egui::Context::default();
        frame(&ctx, &mut a, vec![]);
        let start = egui::pos2(100., 100.);
        let end = egui::pos2(700., 500.);
        frame(
            &ctx,
            &mut a,
            vec![egui::Event::PointerMoved(start), event(start, true)],
        );
        assert!(a.scene.selection.is_empty());
        frame(&ctx, &mut a, vec![egui::Event::PointerMoved(end)]);
        assert!(a.scene.selection.is_empty());
        frame(&ctx, &mut a, vec![event(end, false)]);
        assert!(a
            .scene
            .selection
            .contains(&ingetrazo_core::scene::SelectionItem::Face(0)));
        assert_eq!(a.scene.mesh, before);
        assert!(a.undo_stack.is_empty());
        a.set_layer_locked(0, true);
        frame(
            &ctx,
            &mut a,
            vec![egui::Event::PointerMoved(start), event(start, true)],
        );
        frame(
            &ctx,
            &mut a,
            vec![egui::Event::PointerMoved(end), event(end, false)],
        );
        assert!(a.scene.selection.is_empty());
        assert_eq!(a.scene.mesh, before);
        assert!(a.undo_stack.is_empty());
    }
    #[test]
    fn malformed_candidate_refuses_without_selection_or_geometry_change() {
        let mut a = app();
        a.scene
            .select(ingetrazo_core::scene::SelectionItem::Face(0));
        let before = a.scene.clone();
        let mut selection = BoxSelection {
            viewport: ingetrazo_core::screen_pick::PixelViewport::new(&a.camera, 800., 600.),
            eye: a.camera.eye(),
            start: DVec2::ZERO,
            end: DVec2::splat(800.),
            shift: false,
            ctrl: false,
        };
        selection.viewport.width = f64::NAN;
        assert!(a.dispatch_box_selection(&selection).is_err());
        assert_eq!(a.scene, before);
        selection.viewport.width = 800.;
        a.scene.mesh.edges[0].v0 = usize::MAX;
        let before = a.scene.clone();
        assert!(a.dispatch_box_selection(&selection).is_err());
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
    }
    #[test]
    fn obscured_press_and_escape_do_not_leave_a_box_gesture() {
        let ctx = egui::Context::default();
        let mut a = app();
        let id = egui::Id::new("owned-box");
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        let mut raw = egui::RawInput::default();
        raw.events = vec![event(egui::pos2(100., 100.), true)];
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!input(&mut a, ui, rect, id, false));
            });
        });
        assert!(ctx
            .data(|d| d.get_temp::<egui::Pos2>(id.with("source_box_press")))
            .is_none());
        ctx.data_mut(|d| d.insert_temp(id.with("source_box_press"), egui::pos2(100., 100.)));
        let mut raw = egui::RawInput::default();
        raw.events = vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        }];
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!input(&mut a, ui, rect, id, true));
            });
        });
        assert!(ctx
            .data(|d| d.get_temp::<egui::Pos2>(id.with("source_box_press")))
            .is_none());
        assert!(a.scene.selection.is_empty());
        assert!(a.undo_stack.is_empty());
    }
    #[test]
    fn annotation_delete_refuses_before_history_or_selection_mutation() {
        let mut a = app();
        a.scene
            .guides
            .push(ingetrazo_core::guides::Guide::point(DVec3::ZERO));
        a.scene
            .select(ingetrazo_core::scene::SelectionItem::Face(0));
        a.scene
            .select(ingetrazo_core::scene::SelectionItem::Guide(0));
        let before = a.scene.clone();
        a.delete_selection();
        assert_eq!(a.scene, before);
        assert!(a.undo_stack.is_empty());
        assert!(a.status_message.contains("not yet supported"));
    }
}
