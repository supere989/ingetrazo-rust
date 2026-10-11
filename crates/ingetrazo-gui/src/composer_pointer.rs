//! Source Select pointer transactions. No model snapshot until actual movement.
use super::IngeTrazoApp;
use egui::{Pos2, Rect};
use ingetrazo_core::composition::{Sheet, SheetItem};
#[derive(Clone, Debug)]
pub(super) struct Targets {
    x: Vec<f64>,
    y: Vec<f64>,
}
fn locked(sheet: &Sheet, index: usize) -> bool {
    match &sheet.items[index] {
        SheetItem::Shape(shape) => shape.shape.locked,
        item => sheet
            .native_items
            .get(&item.id())
            .and_then(|v| v.get("locked"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}
impl Targets {
    fn capture(sheet: &Sheet, exclude: usize) -> Self {
        let (w, h) = sheet.dimensions_mm();
        let m = sheet.margin_mm;
        let mut x = vec![0., m, w / 2., w - m, w];
        let mut y = vec![0., m, h / 2., h - m, h];
        for (name, targets) in [("guides_v", &mut x), ("guides_h", &mut y)] {
            if let Some(guides) = sheet.native_document.get(name).and_then(|v| v.as_array()) {
                targets.extend(guides.iter().filter_map(|v| v.as_f64()));
            }
        }
        for index in sheet.stacked_item_indices().into_iter().rev() {
            if index != exclude {
                let r = sheet.items[index].rect();
                x.extend([r.x, r.x + r.w]);
                y.extend([r.y, r.y + r.h]);
            }
        }
        Self { x, y }
    }
    fn snap(value: f64, targets: &[f64]) -> f64 {
        let mut best = None;
        for &target in targets {
            let d = (value - target).abs();
            if d <= 2. && best.is_none_or(|b: f64| d < (value - b).abs()) {
                best = Some(target);
            }
        }
        best.unwrap_or(value)
    }
    fn position(&self, x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
        let x = Self::snap(x, &self.x);
        let y = Self::snap(y, &self.y);
        (
            Self::snap(x + w, &self.x) - w,
            Self::snap(y + h, &self.y) - h,
        )
    }
}
fn clear(app: &mut IngeTrazoApp) {
    let s = &mut app.composer_state;
    s.dragging_item_idx = None;
    s.drag_start_pos = None;
    s.drag_item_orig_pos = None;
    s.drag_item_id = None;
    s.drag_snap_targets = None;
}
fn motion(app: &mut IngeTrazoApp, sheet_idx: usize, pos: Pos2, zoom: f32) {
    let s = &app.composer_state;
    let (Some(index), Some(start), Some((x, y)), Some(id)) = (
        s.dragging_item_idx,
        s.drag_start_pos,
        s.drag_item_orig_pos,
        s.drag_item_id,
    ) else {
        return;
    };
    let sheet = &app.scene.sheets[sheet_idx];
    let Some(item) = sheet.items.get(index).filter(|i| i.id() == id) else {
        clear(app);
        return;
    };
    if locked(sheet, index)
        || !pos.x.is_finite()
        || !pos.y.is_finite()
        || !zoom.is_finite()
        || zoom <= 0.
    {
        return;
    }
    let r = item.rect();
    let d = pos - start;
    let target = (
        x + f64::from(d.x) / f64::from(zoom),
        y + f64::from(d.y) / f64::from(zoom),
    );
    let target = s
        .drag_snap_targets
        .as_ref()
        .map_or(target, |t| t.position(target.0, target.1, r.w, r.h));
    if !target.0.is_finite() || !target.1.is_finite() || (target.0 == r.x && target.1 == r.y) {
        return;
    }
    app.composer_state.snapshot_sheet(sheet);
    app.scene.sheets[sheet_idx].items[index].set_pos(target.0, target.1);
}
pub(super) fn input(
    app: &mut IngeTrazoApp,
    ui: &egui::Ui,
    sheet_idx: usize,
    response: &egui::Response,
    paper: Rect,
    toolbar: Rect,
    zoom: f32,
    order: &[usize],
    space: bool,
) {
    if !ui.is_enabled() || ui.ctx().output(|o| o.num_completed_passes) != 0 {
        return;
    }
    // Response/context queries must stay outside the borrowed input lock.
    let clicked = response.clicked();
    let mut press_ownership = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } => Some((*pos, false)),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    for (pos, owned) in &mut press_ownership {
        *owned = ui.ctx().layer_id_at(*pos) == Some(response.layer_id);
    }
    ui.input(|input| {
        let release = input.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                }
            )
        });
        if app.composer_state.dragging_item_idx.is_some()
            && !input.pointer.primary_down()
            && !release
        {
            app.finish_composer_edit();
        }
        for event in &input.events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if !space && !modifiers.ctrl && !modifiers.shift && !modifiers.alt => {
                    if !press_ownership.iter().any(|(p, owned)| p == pos && *owned)
                        || !response.rect.contains(*pos)
                        || toolbar.contains(*pos)
                    {
                        continue;
                    }
                    let sheet = &mut app.scene.sheets[sheet_idx];
                    let hit = order.iter().rev().copied().find(|&index| {
                        if locked(sheet, index) {
                            return false;
                        }
                        let r = sheet.items[index].rect();
                        let a = paper.min + egui::vec2(r.x as f32 * zoom, r.y as f32 * zoom);
                        Rect::from_min_size(a, egui::vec2(r.w as f32 * zoom, r.h as f32 * zoom))
                            .contains(*pos)
                    });
                    if hit.is_some() {
                        sheet.selected_item = hit;
                    }
                    clear(app);
                    if let Some(index) = hit {
                        let sheet = &app.scene.sheets[sheet_idx];
                        let r = sheet.items[index].rect();
                        let targets = Targets::capture(sheet, index);
                        let s = &mut app.composer_state;
                        s.dragging_item_idx = Some(index);
                        s.drag_item_id = Some(sheet.items[index].id());
                        s.drag_start_pos = Some(*pos);
                        s.drag_item_orig_pos = Some((r.x, r.y));
                        s.drag_snap_targets = Some(targets);
                    }
                }
                egui::Event::PointerMoved(pos) => motion(app, sheet_idx, *pos, zoom),
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    if app.composer_state.dragging_item_idx.is_none() && clicked {
                        app.scene.sheets[sheet_idx].selected_item = None;
                    }
                    if let Some(index) = app.composer_state.dragging_item_idx {
                        if app.scene.sheets[sheet_idx]
                            .items
                            .get(index)
                            .is_some_and(|i| Some(i.id()) == app.composer_state.drag_item_id)
                        {
                            app.scene.sheets[sheet_idx].selected_item = Some(index);
                        }
                    }
                    clear(app);
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    app.scene.sheets[sheet_idx].selected_item = None;
                }
                _ => {}
            }
        }
    });
}
