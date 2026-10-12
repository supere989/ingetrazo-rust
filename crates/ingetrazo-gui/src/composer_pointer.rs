//! Source Select pointer transactions. No model snapshot until actual movement.
use super::IngeTrazoApp;
use egui::{Pos2, Rect};
use ingetrazo_core::composition::{Sheet, SheetItem};
#[derive(Clone, Debug)]
pub(super) struct Targets {
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
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
    pub(super) fn capture_picture(sheet: &Sheet, exclude: usize) -> Result<Self, String> {
        let (w, h) = sheet.dimensions_mm();
        let m = sheet.margin_mm;
        if [w, h, m].iter().any(|v| !v.is_finite())
            || w <= 0.
            || h <= 0.
            || sheet.items.len() > 100_000
        {
            return Err("Invalid/budget-exceeding item snap geometry".into());
        }
        let mut x = vec![0., m, w / 2., w - m, w];
        let mut y = vec![0., m, h / 2., h - m, h];
        for (name, targets) in [("guides_v", &mut x), ("guides_h", &mut y)] {
            let axis = if name == "guides_v" { "v" } else { "h" };
            let guides = sheet
                .native_document
                .get("guides")
                .and_then(|v| v.get(axis))
                .or_else(|| sheet.native_document.get(name));
            if let Some(guides) = guides.and_then(|v| v.as_array()) {
                if guides.len() > 100_000 {
                    return Err("Guide snap budget exceeded".into());
                }
                for v in guides {
                    let n = v
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .ok_or("Invalid guide target")?;
                    targets.push(n);
                }
            }
        }
        let mut ordered = Vec::new();
        ordered
            .try_reserve_exact(sheet.items.len())
            .map_err(|_| "Snap target allocation failed")?;
        for (index, item) in sheet.items.iter().enumerate() {
            let z = match item {
                SheetItem::Shape(v) => v.shape.z,
                _ => sheet
                    .native_items
                    .get(&item.id())
                    .and_then(|v| v.get("z"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.),
            };
            let category = match item {
                SheetItem::ViewFrame(_) => 0,
                SheetItem::Text(_) => 1,
                SheetItem::GraphicScale(_) => 3,
                SheetItem::NorthArrow(_) => 4,
                SheetItem::Shape(_) => 6,
                SheetItem::TitleBlock(_) => 14,
            };
            let mut r = item.rect();
            if let SheetItem::Text(t) = item {
                r.w = sheet
                    .native_items
                    .get(&t.id)
                    .and_then(|v| v.get("w_mm"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(80.);
                r.h = (t.font_size_pt
                    * (25.4 / 72.)
                    * 1.4
                    * (t.text.matches('\n').count() + 1) as f64)
                    .max(6.);
            }
            if let SheetItem::GraphicScale(t) = item {
                let divisions = sheet
                    .native_items
                    .get(&t.id)
                    .and_then(|v| v.get("segments"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(t.divisions as u64);
                if divisions == 0
                    || divisions > 100_000
                    || !t.scale_n.is_finite()
                    || t.scale_n <= 0.
                {
                    return Err("Invalid scale-bar snap size".into());
                }
                let raw = (100. / divisions as f64) * t.scale_n / 1000.;
                let mut segment = 0.1;
                for n in [
                    0.1, 0.2, 0.25, 0.5, 1., 2., 2.5, 5., 10., 20., 25., 50., 100., 200., 250.,
                    500., 1000., 2000., 5000.,
                ] {
                    if n <= raw {
                        segment = n;
                    }
                }
                r.w = (segment * 1000. / t.scale_n) * divisions as f64;
                r.h = 8.;
            }
            ordered.push((z, category, index, r));
        }
        if let Some(images) = sheet
            .native_document
            .get("images")
            .and_then(|v| v.as_array())
        {
            if images.len().saturating_add(ordered.len()) > 100_000 {
                return Err("Item snap budget exceeded".into());
            }
            for (index, _) in images.iter().enumerate() {
                if index != exclude {
                    let r = super::picture_gestures::rect(sheet, index)?;
                    let z = images[index]
                        .get("z")
                        .map_or(Ok(0.), |v| v.as_f64().ok_or("Invalid snap stacking order"))?;
                    ordered.push((z, 2, index, r));
                }
            }
        }
        for (key, category) in [
            ("leyendas", 5),
            ("cotas", 7),
            ("cotas_ang", 8),
            ("cotas_rad", 9),
            ("perfiles", 11),
            ("llamadas", 13),
        ] {
            if let Some(rows) = sheet.native_document.get(key).and_then(|v| v.as_array()) {
                if rows.len().saturating_add(ordered.len()) > 100_000 {
                    return Err("Item snap budget exceeded".into());
                }
                for (index, v) in rows.iter().enumerate() {
                    let n = |k: &str, default: f64| -> Result<f64, String> {
                        v.get(k).map_or(Ok(default), |v| {
                            v.as_f64()
                                .filter(|n| n.is_finite())
                                .ok_or_else(|| format!("Invalid snap furniture {k}"))
                        })
                    };
                    let (w, h) = match key {
                        "leyendas" => (
                            n("w_mm", 55.)?,
                            7.5 + 5.5
                                * v.get("rows")
                                    .and_then(|v| v.as_array())
                                    .map_or(1, |v| v.len().max(1))
                                    as f64,
                        ),
                        "cotas" => (
                            n("dx_mm", 40.)?.abs().max(2.),
                            n("dy_mm", 0.)?.abs().max(2.),
                        ),
                        "cotas_ang" => (
                            n("ax_mm", 30.)?
                                .abs()
                                .max(n("bx_mm", 0.)?.abs())
                                .max(n("radius_mm", 15.)?)
                                .max(2.),
                            n("ay_mm", 0.)?
                                .abs()
                                .max(n("by_mm", -30.)?.abs())
                                .max(n("radius_mm", 15.)?)
                                .max(2.),
                        ),
                        "cotas_rad" => {
                            let d = (2. * n("radius_mm", 15.)?).max(2.);
                            (d, d)
                        }
                        _ => (
                            n("w_mm", if key == "perfiles" { 180. } else { 30. })?,
                            n("h_mm", if key == "perfiles" { 70. } else { 20. })?,
                        ),
                    };
                    ordered.push((
                        n("z", 0.)?,
                        category,
                        index,
                        ingetrazo_core::composition::RectMm::new(
                            n(
                                "x_mm",
                                if matches!(key, "cotas_ang" | "cotas_rad") {
                                    0.
                                } else {
                                    20.
                                },
                            )?,
                            n(
                                "y_mm",
                                if matches!(key, "cotas_ang" | "cotas_rad") {
                                    0.
                                } else {
                                    20.
                                },
                            )?,
                            w,
                            h,
                        ),
                    ));
                }
            }
        }
        if ordered.iter().any(|(z, _, _, r)| {
            !z.is_finite() || [r.x, r.y, r.w, r.h].iter().any(|v| !v.is_finite())
        }) {
            return Err("Invalid item snap geometry".into());
        }
        ordered.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        x.try_reserve(ordered.len() * 2)
            .map_err(|_| "Snap target allocation failed")?;
        y.try_reserve(ordered.len() * 2)
            .map_err(|_| "Snap target allocation failed")?;
        for (_, _, _, r) in ordered.into_iter().rev() {
            x.extend([r.x, r.x + r.w]);
            y.extend([r.y, r.y + r.h]);
        }
        Ok(Self { x, y })
    }
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
    pub(super) fn snap(value: f64, targets: &[f64]) -> f64 {
        let mut best = None;
        for &target in targets {
            let d = (value - target).abs();
            if d <= 2. && best.is_none_or(|b: f64| d < (value - b).abs()) {
                best = Some(target);
            }
        }
        best.unwrap_or(value)
    }
    pub(super) fn position(&self, x: f64, y: f64, w: f64, h: f64) -> (f64, f64) {
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
    s.picture_grab = None;
    s.picture_ctrl_toggle = false;
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
    order: &[(bool, usize)],
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
    let mut release_ownership = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => Some((*pos, false)),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    for (pos, owned) in &mut release_ownership {
        *owned = ui.ctx().layer_id_at(*pos) == Some(response.layer_id)
            && response.rect.contains(*pos)
            && !toolbar.contains(*pos);
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
        if (app.composer_state.dragging_item_idx.is_some()
            || app.composer_state.picture_grab.is_some())
            && !input.pointer.primary_down()
            && !release
            && !input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
        {
            app.finish_composer_edit();
        }
        for event in &input.events {
            match event {
                egui::Event::PointerGone => {
                    if app.composer_state.dragging_item_idx.is_some()
                        || app.composer_state.picture_grab.is_some()
                    {
                        app.finish_composer_edit();
                    }
                    app.composer_state.picture_ctrl_toggle = false;
                }

                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if !space && !modifiers.alt => {
                    if !press_ownership.iter().any(|(p, owned)| p == pos && *owned)
                        || !response.rect.contains(*pos)
                        || toolbar.contains(*pos)
                    {
                        continue;
                    }
                    let sheet = &app.scene.sheets[sheet_idx];
                    let hit = order.iter().rev().copied().find(|&(picture, index)| {
                        let r = if picture {
                            if super::picture_gestures::locked(sheet, index) {
                                return false;
                            }
                            let Ok(r) = super::picture_gestures::rect(sheet, index) else {
                                return false;
                            };
                            ingetrazo_core::composition::RectMm::new(
                                r.x - 0.5,
                                r.y - 0.5,
                                r.w + 3.5,
                                r.h + 3.5,
                            )
                        } else {
                            if locked(sheet, index) {
                                return false;
                            }
                            sheet.items[index].rect()
                        };
                        let a = paper.min + egui::vec2(r.x as f32 * zoom, r.y as f32 * zoom);
                        Rect::from_min_size(a, egui::vec2(r.w as f32 * zoom, r.h as f32 * zoom))
                            .contains(*pos)
                    });
                    if let Some((true, index)) = hit {
                        clear(app);
                        if modifiers.ctrl {
                            let selected = app
                                .composer_picture_selection()
                                .is_some_and(|v| v.1 == index);
                            if let Err(e) =
                                app.composer_select_picture((!selected).then_some(index))
                            {
                                app.status_message = e;
                            }
                            app.composer_state.picture_ctrl_toggle = true;
                        } else if let Err(e) = super::picture_gestures::acquire(
                            app, sheet_idx, index, *pos, paper, zoom,
                        ) {
                            app.status_message = e;
                        }
                        continue;
                    }
                    // Existing typed item semantics retain their modifier admission.
                    if modifiers.ctrl || modifiers.shift {
                        continue;
                    }
                    clear(app);
                    app.composer_state.picture_authoring.selected = None;
                    if let Some((false, index)) = hit {
                        app.scene.sheets[sheet_idx].selected_item = Some(index);
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
                egui::Event::PointerMoved(pos) => {
                    if app.composer_state.picture_grab.is_some() {
                        if let Err(e) =
                            super::picture_gestures::motion(app, sheet_idx, *pos, paper, zoom)
                        {
                            app.status_message = e;
                            app.finish_composer_edit();
                        }
                    } else {
                        motion(app, sheet_idx, *pos, zoom)
                    }
                }
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    pos,
                    ..
                } => {
                    if app.composer_state.picture_ctrl_toggle {
                        app.composer_state.picture_ctrl_toggle = false;
                        continue;
                    }
                    if app.composer_state.picture_grab.is_some() {
                        app.finish_composer_edit();
                        continue;
                    }
                    let acquired = app.composer_state.dragging_item_idx.is_some();
                    if !acquired
                        && clicked
                        && release_ownership
                            .iter()
                            .any(|(p, owned)| p == pos && *owned)
                    {
                        app.scene.sheets[sheet_idx].selected_item = None;
                        app.composer_state.picture_authoring.selected = None;
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
                    // Release is the transaction boundary even when another
                    // press follows in this same event packet. Unrelated
                    // property-editor releases do not finish their draft.
                    if acquired {
                        app.finish_composer_edit();
                    } else {
                        clear(app);
                    }
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    app.scene.sheets[sheet_idx].selected_item = None;
                    app.composer_state.picture_authoring.selected = None;
                }
                _ => {}
            }
        }
    });
}
