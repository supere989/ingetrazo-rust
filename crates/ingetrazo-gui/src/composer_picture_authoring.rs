//! Picture authoring shares export pixels and sheet-owned transactions.
use crate::app::IngeTrazoApp;
use ingetrazo_core::i18n::tr;
use std::{path::Path, sync::Arc};
#[derive(Clone, Default)]
pub(super) struct State {
    pub selected: Option<(uuid::Uuid, usize)>,
    coalesce: Option<(uuid::Uuid, usize, Vec<String>, usize, u64)>,
    previews: std::collections::HashMap<usize, Preview>,
    seen: std::collections::HashMap<usize, serde_json::Value>,
    sheet: Option<uuid::Uuid>,
    uploads: u64,
    cold_previews: u64,
    sources: std::collections::HashMap<String, Option<Arc<ingetrazo_core::paper_picture::Pixels>>>,
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PictureAuthoring")
            .field("selected", &self.selected)
            .finish()
    }
}
#[derive(Clone)]
struct Preview {
    pixels: Arc<ingetrazo_core::paper_picture::Pixels>,
    texture: egui::TextureHandle,
}
impl State {
    pub(super) fn history_boundary(&mut self) {
        self.coalesce = None;
        self.previews.clear();
        self.seen.clear();
    }
    pub(super) fn clear(&mut self) {
        self.selected = None;
        self.coalesce = None;
        self.previews.clear();
        self.seen.clear();
        self.sheet = None;
        self.sources.clear();
    }
}
fn images(sheet: &ingetrazo_core::composition::Sheet) -> Result<&[serde_json::Value], String> {
    match sheet.native_document.get("images") {
        None | Some(serde_json::Value::Null) => Ok(&[]),
        Some(v) => v
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| "Invalid sheet images".into()),
    }
}
fn active(app: &IngeTrazoApp) -> Result<usize, String> {
    app.scene
        .active_sheet_index
        .filter(|&i| i < app.scene.sheets.len())
        .ok_or_else(|| "No active sheet".into())
}
impl IngeTrazoApp {
    fn admitted_composer_picture(
        &mut self,
        value: &serde_json::Value,
    ) -> Result<Option<ingetrazo_core::paper_picture::Picture>, String> {
        let s = active(self)?;
        let sid = self.scene.sheets[s].id;
        let state = &mut self.composer_state.picture_authoring;
        if state.sheet != Some(sid) {
            state.clear();
            state.sheet = Some(sid);
        }
        let path = value
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("Invalid picture path")?;
        if !state.sources.contains_key(path) {
            if state.sources.len() >= 4096 {
                return Err("Picture path owner limit exceeded".into());
            }
            let prior_texture_bytes = state.previews.values().try_fold(0usize, |n, p| {
                n.checked_add(p.pixels.rgba().len() * 12)
                    .ok_or("Picture preview budget overflow")
            })?;
            self.sheet_image.painters.pictures.admit_preview(
                prior_texture_bytes
                    .checked_add(80 * 1024 * 1024)
                    .ok_or("Picture preview budget overflow")?,
            )?;
            let plain = serde_json::json!({"path":path,"w_mm":60.,"h_mm":40.});
            let image = self
                .sheet_image
                .painters
                .pictures
                .prepare(&plain)?
                .map(|p| p.image);
            state
                .sources
                .try_reserve(1)
                .map_err(|_| "Picture path cache allocation failed")?;
            state.sources.insert(path.into(), image);
        }
        let prior_texture_bytes = state.previews.values().try_fold(0usize, |n, p| {
            n.checked_add(p.pixels.rgba().len() * 12)
                .ok_or("Picture preview budget overflow")
        })?;
        self.sheet_image.painters.pictures.admit_preview(
            prior_texture_bytes
                .checked_add(32 * 1024 * 1024)
                .ok_or("Picture preview budget overflow")?,
        )?;
        self.sheet_image
            .painters
            .pictures
            .prepare_preview(value, state.sources.get(path).cloned().flatten())
    }

    pub fn composer_picture_preview_stats(&self) -> [u64; 5] {
        let cache = &self.sheet_image.painters.pictures;
        let state = &self.composer_state.picture_authoring;
        [
            cache.prepare_calls,
            cache.source_reads,
            cache.encoded_read_bytes,
            state.uploads,
            state.cold_previews,
        ]
    }
    pub(super) fn revalidate_picture_selection(&mut self) {
        if self.composer_picture_selection().is_none() {
            self.composer_state.picture_authoring.selected = None;
        }
    }
    pub fn composer_picture_selection(&self) -> Option<(uuid::Uuid, usize)> {
        let selected = self.composer_state.picture_authoring.selected?;
        let sheet = self.scene.sheets.get(self.scene.active_sheet_index?)?;
        if sheet.id == selected.0
            && sheet.selected_item.is_none()
            && images(sheet).ok()?.get(selected.1).is_some()
        {
            Some(selected)
        } else {
            None
        }
    }
    pub fn composer_select_picture(&mut self, index: Option<usize>) -> Result<(), String> {
        let s = active(self)?;
        if index.is_some_and(|i| images(&self.scene.sheets[s]).map_or(true, |v| i >= v.len())) {
            return Err("Picture no longer exists".into());
        }
        self.finish_composer_edit();
        let sid = self.scene.sheets[s].id;
        if self.composer_state.picture_authoring.sheet != Some(sid) {
            self.composer_state.picture_authoring.clear();
            self.composer_state.picture_authoring.sheet = Some(sid);
        }
        self.composer_state.picture_authoring.selected =
            index.map(|i| (self.scene.sheets[s].id, i));
        if index.is_some() {
            self.scene.sheets[s].selected_item = None;
        }
        Ok(())
    }
    pub fn composer_add_picture(&mut self, path: Option<&Path>) -> Result<Option<usize>, String> {
        let Some(path) = path else { return Ok(None) };
        let s = active(self)?;
        let raw = path.to_str().ok_or("Picture path is not UTF-8")?;
        let mut value = serde_json::json!({"path":raw,"x_mm":self.scene.sheets[s].margin_mm+4.,"y_mm":self.scene.sheets[s].margin_mm+4.,"w_mm":60.,"h_mm":40.,"opacity":1.,"shape":"rect","radius_mm":4.,"feather_mm":0.,"fit":"stretch","border":false,"border_mm":0.3,"border_color":"#282e36","locked":false,"group_id":"","list_name":"","hidden":false,"z":0.});
        if let Some(p) = self.admitted_composer_picture(&value)? {
            value["h_mm"] =
                serde_json::json!(60. * (f64::from(p.image.height()) / f64::from(p.image.width())));
        }
        let sheet = &self.scene.sheets[s];
        let old = images(sheet)?;
        if old.len() >= 100_000 {
            return Err("Picture item limit exceeded".into());
        }
        let mut max: Option<f64> = None;
        // all_items includes hidden entities, across all original collections.
        for (key, array) in sheet
            .native_document
            .as_object()
            .into_iter()
            .flat_map(|v| v.iter())
        {
            if matches!(
                key.as_str(),
                "images"
                    | "perfiles"
                    | "leyendas"
                    | "cotas"
                    | "cotas_ang"
                    | "cotas_rad"
                    | "etiquetas"
                    | "niveles"
                    | "llamadas"
            ) {
                if let Some(rows) = array.as_array() {
                    for row in rows {
                        let z = row
                            .get("z")
                            .map_or(Ok(0.), |v| v.as_f64().ok_or("Invalid stacking order"))?;
                        if !z.is_finite() {
                            return Err("Invalid stacking order".into());
                        }
                        max = Some(max.map_or(z, |m| m.max(z)));
                    }
                }
            }
        }
        for item in &sheet.items {
            let z = match item {
                ingetrazo_core::composition::SheetItem::Shape(v) => v.shape.z,
                _ => sheet
                    .native_items
                    .get(&item.id())
                    .and_then(|v| v.get("z"))
                    .map_or(Ok(0.), |v| v.as_f64().ok_or("Invalid stacking order"))?,
            };
            if !z.is_finite() {
                return Err("Invalid stacking order".into());
            }
            max = Some(max.map_or(z, |m| m.max(z)));
        }
        let z = max.map_or(0., |m| m + 1.);
        if !z.is_finite() {
            return Err("Invalid stacking order".into());
        }
        value["z"] = serde_json::json!(z);
        let index = old.len();
        let mut candidate = old.to_vec();
        candidate
            .try_reserve(1)
            .map_err(|_| "Picture list allocation failed")?;
        candidate.push(value);
        self.finish_composer_edit();
        self.begin_composer_edit();
        self.scene.sheets[s].native_document["images"] = serde_json::Value::Array(candidate);
        self.finish_composer_edit();
        self.composer_state.picture_authoring.coalesce = None;
        Ok(Some(index))
    }
    pub fn composer_replace_picture(&mut self, path: Option<&Path>) -> Result<(), String> {
        let Some(path) = path else { return Ok(()) };
        let mut changes = serde_json::Map::new();
        changes.insert(
            "path".into(),
            serde_json::Value::String(path.to_str().ok_or("Picture path is not UTF-8")?.into()),
        );
        self.picture_patch(&changes, false)
    }
    pub fn composer_edit_picture(&mut self, changes: &serde_json::Value) -> Result<(), String> {
        self.picture_patch(changes.as_object().ok_or("Invalid picture changes")?, true)
    }
    fn picture_patch(
        &mut self,
        changes: &serde_json::Map<String, serde_json::Value>,
        coalesce: bool,
    ) -> Result<(), String> {
        let (sid, index) = self
            .composer_picture_selection()
            .ok_or("Select a picture")?;
        let s = active(self)?;
        let current = images(&self.scene.sheets[s])?
            .get(index)
            .ok_or("Picture no longer exists")?;
        let mut candidate = current.clone();
        let object = candidate.as_object_mut().ok_or("Invalid picture record")?;
        for (k, v) in changes {
            match k.as_str() {
                "path" | "shape" | "fit" | "border_color" => {
                    if !v.is_string() {
                        return Err(format!("Invalid picture {k}"));
                    }
                }
                "border" => {
                    if !v.is_boolean() {
                        return Err("Invalid picture border".into());
                    }
                }
                "x_mm" | "y_mm" | "w_mm" | "h_mm" | "opacity" | "radius_mm" | "feather_mm"
                | "border_mm" => {
                    let n = v.as_f64().ok_or_else(|| format!("Invalid picture {k}"))?;
                    if !n.is_finite() || n.abs() > 1e6 {
                        return Err(format!("Invalid picture {k}"));
                    }
                }
                _ => return Err(format!("Unsupported picture property {k}")),
            }
            object.insert(k.clone(), v.clone());
        }
        if candidate == *current {
            if !coalesce {
                self.finish_composer_edit();
                let before = self.scene.sheets[s].clone();
                let h = self.composer_state.sheet_history.entry(sid).or_default();
                h.0.push(before);
                h.1.clear();
                self.scene.bump_version();
                self.composer_state.picture_authoring.coalesce = None;
            }
            return Ok(());
        }
        self.admitted_composer_picture(&candidate)?;
        let key: Vec<_> = changes.keys().cloned().collect();
        let len = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        let merge = coalesce
            && self
                .composer_state
                .picture_authoring
                .coalesce
                .as_ref()
                .is_some_and(|(id, i, k, l, rev)| {
                    *id == sid
                        && *i == index
                        && *k == key
                        && *l == len
                        && *rev == self.scene.version
                });
        self.finish_composer_edit();
        if merge {
            self.composer_state.edit_before = self
                .composer_state
                .sheet_history
                .get_mut(&sid)
                .and_then(|h| h.0.pop());
        } else {
            self.begin_composer_edit();
        }
        self.scene.sheets[s].native_document["images"][index] = candidate;
        self.finish_composer_edit();
        let len = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        self.composer_state.picture_authoring.coalesce =
            coalesce.then_some((sid, index, key, len, self.scene.version));
        Ok(())
    }
}
pub(super) fn properties(app: &mut IngeTrazoApp, ui: &mut egui::Ui, s: usize) {
    let mut select = None;
    let mut replace = false;
    let mut patch = None;
    egui::Window::new(tr("Images"))
        .id(egui::Id::new("composer-picture-properties"))
        .default_pos(ui.max_rect().right_top() - egui::vec2(245., 0.))
        .show(ui.ctx(), |ui| {
            let selected = app.composer_picture_selection().map(|v| v.1);
            if let Ok(rows) = images(&app.scene.sheets[s]) {
                let row_height=ui.text_style_height(&egui::TextStyle::Body);
                egui::ScrollArea::vertical().max_height(160.).show_rows(
                    ui,
                    row_height,
                    rows.len().min(100_000),
                    |ui, range| {
                        for i in range {
                            let v = &rows[i];
                            let label = v["path"].as_str().unwrap_or("Invalid picture");
                            if ui
                                .selectable_label(
                                    selected == Some(i),
                                    format!("{}: {}", i + 1, label),
                                )
                                .clicked()
                            {
                                select = Some(i);
                            }
                        }
                    },
                );
                if let Some(index) = selected {
                    if let Some(old) = rows.get(index) {
                        let mut v = old.clone();
                        let replace_button = ui.button(tr("Choose image…"));
                        replace = replace_button.clicked();
                        if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new("picture-ui-replace"),
                                    replace_button.rect,
                                )
                            });
                        }
                        let mut changes = serde_json::Map::new();
                        let mut props = false;
                        for k in [
                            "x_mm",
                            "y_mm",
                            "w_mm",
                            "h_mm",
                            "radius_mm",
                            "feather_mm",
                            "border_mm",
                        ] {
                            if let Some(mut n) = v[k].as_f64() {
                                ui.horizontal(|ui| {
                                    ui.label(match k {
                                        "x_mm" => tr("X (mm)"),
                                        "y_mm" => tr("Y (mm)"),
                                        "w_mm" => tr("Width (mm)"),
                                        "h_mm" => tr("Height (mm)"),
                                        "radius_mm" => tr("Corner radius (mm)"),
                                        "feather_mm" => tr("Feathered edge (mm)"),
                                        _ => tr("Outline width (mm)"),
                                    });
                                    if ui
                                        .add(egui::DragValue::new(&mut n).speed(0.1).range(
                                            match k {
                                                "radius_mm" => 0.5..=100.,
                                                "feather_mm" => 0.0..=50.,
                                                "border_mm" => 0.1..=2.,
                                                "w_mm" | "h_mm" => 0.1..=100_000.,
                                                _ => -100_000.0..=100_000.,
                                            },
                                        ))
                                        .changed()
                                    {
                                        v[k] = serde_json::json!(n);
                                        if matches!(k, "radius_mm" | "feather_mm" | "border_mm") {
                                            props = true;
                                        } else {
                                            changes.insert(k.into(), v[k].clone());
                                        }
                                    }
                                });
                            }
                        }
                        let mut opacity = v["opacity"].as_f64().unwrap_or(1.) * 100.;
                        let opacity_control = ui.add(
                            egui::DragValue::new(&mut opacity)
                                .range(0..=100)
                                .suffix(" %"),
                        );
                        if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new("picture-ui-opacity"),
                                    opacity_control.rect,
                                )
                            });
                        }
                        if opacity_control.changed() {
                            changes.insert("opacity".into(), serde_json::json!(opacity / 100.));
                        }
                        for (k, options) in [
                            ("shape", &["rect", "rounded", "ellipse"][..]),
                            ("fit", &["stretch", "cover", "contain"][..]),
                        ] {
                            let mut text = v[k].as_str().unwrap_or(options[0]).to_owned();
                            egui::ComboBox::from_id_salt(k)
                                .selected_text(&text)
                                .show_ui(ui, |ui| {
                                    for &choice in options {
                                        props |= ui
                                            .selectable_value(&mut text, choice.into(), choice)
                                            .changed();
                                    }
                                });
                            v[k] = serde_json::json!(text);
                        }
                        let mut border = v["border"].as_bool().unwrap_or(false);
                        let border_control = ui.checkbox(&mut border, tr("Outline"));
                        props |= border_control.changed();
                        if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(
                                    egui::Id::new("picture-ui-border"),
                                    border_control.rect,
                                )
                            });
                        }
                        v["border"] = serde_json::json!(border);
                        if props {
                            for k in [
                                "shape",
                                "radius_mm",
                                "feather_mm",
                                "fit",
                                "border",
                                "border_mm",
                            ] {
                                changes.insert(k.into(), v[k].clone());
                            }
                        }
                        let mut color = v["border_color"].as_str().unwrap_or("#282e36").to_owned();
                        if ui.text_edit_singleline(&mut color).changed() {
                            changes.insert("border_color".into(), serde_json::json!(color));
                        }
                        if !changes.is_empty() {
                            patch = Some(serde_json::Value::Object(changes));
                        }
                    }
                }
            }
        });
    if let Some(i) = select {
        if let Err(e) = app.composer_select_picture(Some(i)) {
            app.status_message = e;
        }
    }
    if replace {
        let path =
            crate::dialogs::pick_open_file(&tr("Choose image…"), "Images (*.png *.jpg *.jpeg)");
        if let Err(e) = app.composer_replace_picture(path.as_deref()) {
            app.status_message = e;
        }
    }
    if let Some(p) = patch {
        if let Err(e) = app.composer_edit_picture(&p) {
            app.status_message = e;
        }
    }
}
pub(super) fn paint(
    app: &mut IngeTrazoApp,
    painter: &egui::Painter,
    s: usize,
    index: usize,
    zoom: f32,
    origin: egui::Pos2,
) -> Result<(), String> {
    let value = images(&app.scene.sheets[s])?
        .get(index)
        .ok_or("Picture no longer exists")?;
    let path = value["path"].as_str().unwrap_or("");
    let state = &mut app.composer_state.picture_authoring;
    if state.sheet != Some(app.scene.sheets[s].id) {
        state.previews.clear();
        state.seen.clear();
        state.sources.clear();
        state.sheet = Some(app.scene.sheets[s].id);
    }
    let cold = state.seen.get(&index) != Some(value);
    if cold {
        state.cold_previews += 1;
        let prior_texture_bytes = state.previews.values().try_fold(0usize, |n, p| {
            n.checked_add(p.pixels.rgba().len() * 12)
                .ok_or("Picture preview budget overflow")
        })?;
        app.sheet_image.painters.pictures.admit_preview(
            prior_texture_bytes
                .checked_add(if state.sources.contains_key(path) {
                    32 * 1024 * 1024
                } else {
                    80 * 1024 * 1024
                })
                .ok_or("Picture preview budget overflow")?,
        )?;
        if !state.sources.contains_key(path) {
            if state.sources.len() >= 4096 {
                return Err("Picture path owner limit exceeded".into());
            }
            let plain = serde_json::json!({"path":path,"w_mm":60.,"h_mm":40.});
            let pixels = app
                .sheet_image
                .painters
                .pictures
                .prepare(&plain)?
                .map(|p| p.image);
            state.sources.insert(path.into(), pixels);
        }
        if let Some(p) = app
            .sheet_image
            .painters
            .pictures
            .prepare_preview(value, state.sources.get(path).cloned().flatten())?
        {
            let bytes = p.image.rgba().len();
            let retained = state.previews.values().try_fold(0usize, |n, p| {
                n.checked_add(p.pixels.rgba().len() * 12)
                    .ok_or("Picture preview budget overflow")
            })?;
            let work = retained
                .checked_add(
                    bytes
                        .checked_mul(12)
                        .ok_or("Picture preview budget overflow")?,
                )
                .ok_or("Picture preview budget overflow")?;
            app.sheet_image.painters.pictures.admit_preview(work)?;
            let texture = if let Some(hit) = state
                .previews
                .values()
                .find(|v| v.pixels.id() == p.image.id())
            {
                hit.texture.clone()
            } else {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [p.image.width() as usize, p.image.height() as usize],
                    p.image.rgba(),
                );
                state.uploads += 1;
                painter.ctx().load_texture(
                    format!("sheet-picture-{}", p.image.id()),
                    image,
                    egui::TextureOptions::LINEAR,
                )
            };
            state.previews.insert(
                index,
                Preview {
                    pixels: p.image,
                    texture,
                },
            );
        } else {
            state.previews.remove(&index);
        }
        state.seen.insert(index, value.clone());
    }
    let n = |k: &str| {
        value[k]
            .as_f64()
            .ok_or_else(|| format!("Invalid picture {k}"))
    };
    let [x, y, w, h] = [n("x_mm")?, n("y_mm")?, n("w_mm")?, n("h_mm")?];
    let r = egui::Rect::from_min_size(
        origin + egui::vec2((x * zoom as f64) as f32, (y * zoom as f64) as f32),
        egui::vec2((w * zoom as f64) as f32, (h * zoom as f64) as f32),
    );
    if let Some(p) = state.previews.get(&index) {
        painter.image(
            p.texture.id(),
            r,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
            egui::Color32::WHITE,
        );
    } else {
        painter.rect_filled(r, 0., egui::Color32::from_rgb(240, 240, 242));
        painter.rect_stroke(
            r,
            0.,
            egui::Stroke::new(0.25 * zoom, egui::Color32::from_rgb(170, 176, 184)),
        );
        painter.line_segment(
            [r.left_top(), r.right_bottom()],
            egui::Stroke::new(0.25 * zoom, egui::Color32::from_rgb(170, 176, 184)),
        );
        painter.line_segment(
            [r.right_top(), r.left_bottom()],
            egui::Stroke::new(0.25 * zoom, egui::Color32::from_rgb(170, 176, 184)),
        );
    }
    if state.previews.contains_key(&index) && value["border"].as_bool() == Some(true) {
        let model = ingetrazo_core::sheet_shape::SheetShape {
            kind: if value["shape"].as_str() == Some("ellipse") {
                "elipse".into()
            } else {
                "rect".into()
            },
            x_mm: x,
            y_mm: y,
            w_mm: w,
            h_mm: h,
            stroke_mm: value["border_mm"].as_f64().unwrap_or(0.3).max(0.1),
            color: value["border_color"].as_str().unwrap_or("#282e36").into(),
            radius_mm: if value["shape"].as_str() == Some("rounded") {
                value["radius_mm"].as_f64().unwrap_or(4.)
            } else {
                0.
            },
            ..Default::default()
        };
        crate::composer::paint_sheet_shape(painter, &model, zoom, origin)?;
    }
    if state.selected == Some((app.scene.sheets[s].id, index)) {
        painter.rect_stroke(r, 0., egui::Stroke::new(1., egui::Color32::LIGHT_BLUE));
    }
    Ok(())
}
