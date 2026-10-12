//! Source-backed raw legend authoring; active-sheet transactions retain opaque fields.
use super::IngeTrazoApp;
use serde_json::{Value, json};
#[derive(Clone, Default)]
pub(super) struct State {
    pub(super) armed: bool,
    pub(super) pending: Option<glam::DVec2>,
    pub(super) selected: Option<(uuid::Uuid, usize)>,
    pub(super) grab: Option<Grab>,
    cache: std::collections::HashMap<usize, Preview>,
    cache_sheet: Option<uuid::Uuid>,
    pub(super) cache_builds: u64,
    pub(super) title_draft: Option<(uuid::Uuid, usize, String)>,
    pub(super) coalesce: Option<(uuid::Uuid, usize, &'static str, usize, u64)>,
}
fn active(app: &IngeTrazoApp) -> Result<usize, String> {
    app.scene
        .active_sheet_index
        .filter(|&i| i < app.scene.sheets.len())
        .ok_or_else(|| "No active sheet".into())
}
fn rows(app: &IngeTrazoApp) -> Result<Value, String> {
    let mut rows = Vec::new();
    rows.try_reserve(app.scene.layers.len())
        .map_err(|_| "Legend rows allocation refused")?;
    for layer in &app.scene.layers {
        if layer.visible {
            rows.push(Value::String(layer.name.clone()));
        }
    }
    if rows.len() > 10000 {
        return Err("Legend row limit exceeded".into());
    }
    Ok(Value::Array(rows))
}
fn admitted(value: &Value) -> Result<(), String> {
    let o = value.as_object().ok_or("Invalid legend record")?;
    for (k, default) in [("x_mm", 20.), ("y_mm", 20.), ("w_mm", 55.), ("z", 0.)] {
        let n = o
            .get(k)
            .map_or(Some(default), Value::as_f64)
            .filter(|n| n.is_finite() && n.abs() <= 1e6)
            .ok_or("Invalid legend geometry")?;
        if k == "w_mm" && n <= 10. {
            return Err("Legend width must exceed 10 mm".into());
        }
    }
    if o.get("title")
        .is_some_and(|v| v.as_str().is_none_or(|s| s.len() > 1024 * 1024))
    {
        return Err("Invalid legend title".into());
    }
    if let Some(v) = o.get("rows") {
        let r = v.as_array().ok_or("Invalid legend rows")?;
        if r.len() > 10000
            || r.iter()
                .any(|v| v.as_str().is_none_or(|s| s.len() > 1024 * 1024))
        {
            return Err("Invalid legend rows".into());
        }
    }
    for k in ["locked", "hidden"] {
        if o.get(k).is_some_and(|v| !v.is_boolean()) {
            return Err("Invalid legend visibility".into());
        }
    }
    Ok(())
}
impl IngeTrazoApp {
    pub fn composer_legend_heights(&self) -> Result<Vec<f64>, String> {
        let s = active(self)?;
        let count = self.scene.sheets[s]
            .native_document
            .get("leyendas")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        (0..count)
            .map(|i| rect(&self.scene.sheets[s], i).map(|r| r.h))
            .collect()
    }
    pub fn composer_legend_preview_builds(&self) -> u64 {
        self.composer_state.legend_authoring.cache_builds
    }
    pub(super) fn revalidate_legend_selection(&mut self) {
        if self.composer_legend_selection().is_none() {
            self.composer_state.legend_authoring.selected = None;
        }
    }
    pub fn composer_legend_selection(&self) -> Option<(uuid::Uuid, usize)> {
        let s = active(self).ok()?;
        if self.scene.sheets[s].selected_item.is_some()
            || self.composer_state.picture_authoring.selected.is_some()
        {
            return None;
        }
        let selected = self.composer_state.legend_authoring.selected?;
        (selected.0 == self.scene.sheets[s].id
            && self.scene.sheets[s]
                .native_document
                .get("leyendas")?
                .as_array()?
                .get(selected.1)
                .is_some())
        .then_some(selected)
    }
    pub fn composer_select_legend(&mut self, index: Option<usize>) -> Result<(), String> {
        let s = active(self)?;
        let id = self.scene.sheets[s].id;
        if let Some(i) = index {
            let value = self.scene.sheets[s]
                .native_document
                .get("leyendas")
                .and_then(Value::as_array)
                .and_then(|v| v.get(i))
                .ok_or("Legend no longer exists")?;
            admitted(value)?;
            if value.get("hidden").and_then(Value::as_bool) == Some(true) {
                return Err("Legend is hidden or locked".into());
            }
        }
        self.finish_composer_edit();
        self.composer_state.legend_authoring.selected = index.map(|i| (id, i));
        if index.is_some() {
            self.scene.sheets[s].selected_item = None;
            self.composer_state.picture_authoring.selected = None;
        }
        Ok(())
    }
    pub fn activate_composer_legend(&mut self) -> Result<(), String> {
        active(self)?;
        self.finish_composer_edit();
        self.composer_state.text_tool = false;
        self.composer_state.shape_tool = None;
        self.composer_state.shape_start = None;
        self.composer_state.legend_authoring.pending = None;
        self.composer_state.legend_authoring.armed = true;
        Ok(())
    }
    pub fn composer_legend_pending(&self) -> bool {
        self.composer_state.legend_authoring.pending.is_some()
    }
    pub fn composer_legend_armed(&self) -> bool {
        self.composer_state.legend_authoring.armed
    }
    pub fn place_composer_legend(&mut self, point: glam::DVec2) -> Result<usize, String> {
        if !self.composer_state.legend_authoring.armed {
            return Err("Legend placement tool is not armed".into());
        }
        let s = active(self)?;
        if !point.is_finite() || point.abs().max_element() > 1e6 {
            return Err("Invalid legend placement".into());
        }
        let z = super::picture_authoring::next_z(&self.scene.sheets[s])?;
        let value = json!({"x_mm":point.x,"y_mm":point.y,"w_mm":55.,"title":"LEYENDA","rows":rows(self)?,"z":z,"locked":false,"group_id":"","list_name":"","hidden":false});
        admitted(&value)?;
        let count = self.scene.sheets[s]
            .native_document
            .get("leyendas")
            .map_or(Some(0), |v| v.as_array().map(Vec::len))
            .ok_or("Invalid legend inventory")?;
        if count >= 10000 {
            return Err("Legend item limit exceeded".into());
        }
        let mut candidate = self.scene.sheets[s]
            .native_document
            .get("leyendas")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        candidate
            .try_reserve(1)
            .map_err(|_| "Legend list allocation refused")?;
        candidate.push(value);
        self.finish_composer_edit();
        self.begin_composer_edit();
        let sheet = &mut self.scene.sheets[s];
        sheet.native_document["leyendas"] = Value::Array(candidate);
        let id = sheet.id;
        sheet.selected_item = None;
        self.finish_composer_edit();
        self.composer_state.picture_authoring.selected = None;
        self.composer_state.legend_authoring.selected = Some((id, count));
        self.composer_state.legend_authoring.armed = false;
        Ok(count)
    }
    fn edit_legend(&mut self, key: &'static str, new: Value) -> Result<(), String> {
        let (sid, index) = self
            .composer_legend_selection()
            .ok_or("No selected legend")?;
        let s = active(self)?;
        let current = &self.scene.sheets[s].native_document["leyendas"][index];
        let mut candidate = current.clone();
        candidate[key] = new;
        admitted(&candidate)?;
        if candidate == *current {
            return Ok(());
        }
        let len = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        let merge =
            self.composer_state
                .legend_authoring
                .coalesce
                .is_some_and(|(id, i, k, l, rev)| {
                    id == sid && i == index && k == key && l == len && rev == self.scene.version
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
        self.scene.sheets[s].native_document["leyendas"][index] = candidate;
        self.finish_composer_edit();
        let len = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        self.composer_state.legend_authoring.coalesce =
            Some((sid, index, key, len, self.scene.version));
        Ok(())
    }
    pub fn composer_edit_legend_title(&mut self, title: &str) -> Result<(), String> {
        self.edit_legend("title", Value::String(title.into()))
    }
    pub fn refresh_composer_legend(&mut self) -> Result<(), String> {
        let value = rows(self)?;
        self.edit_legend("rows", value)
    }
    pub fn delete_composer_legend(&mut self) -> Result<(), String> {
        let (_, i) = self
            .composer_legend_selection()
            .ok_or("No selected legend")?;
        let s = active(self)?;
        self.finish_composer_edit();
        self.begin_composer_edit();
        self.scene.sheets[s].native_document["leyendas"]
            .as_array_mut()
            .unwrap()
            .remove(i);
        // Explicit empty ownership prevents retained IGZ overlay from resurrecting imported records.
        self.finish_composer_edit();
        self.composer_state.legend_authoring.selected = None;
        self.composer_state.legend_authoring.coalesce = None;
        Ok(())
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LegendState")
            .field("armed", &self.armed)
            .field("selected", &self.selected)
            .field("cache_builds", &self.cache_builds)
            .finish()
    }
}
#[derive(Clone, Debug)]
pub(super) struct Grab {
    sheet: uuid::Uuid,
    index: usize,
    press: egui::Pos2,
    original: ingetrazo_core::composition::RectMm,
    resize: bool,
    targets: super::pointer_transactions::Targets,
}
#[derive(Clone)]
struct Preview {
    language: String,
    value: Value,
    draws: Vec<Part>,
    bytes: usize,
}
#[derive(Clone)]
enum Part {
    Path(ingetrazo_formats::sheet_pdf::Draw),
    Text(
        ingetrazo_core::composition::TextItem,
        std::sync::Arc<Vec<ingetrazo_formats::sheet_pdf::Draw>>,
    ),
}
pub(super) fn rect(
    sheet: &ingetrazo_core::composition::Sheet,
    i: usize,
) -> Result<ingetrazo_core::composition::RectMm, String> {
    let v = sheet
        .native_document
        .get("leyendas")
        .and_then(Value::as_array)
        .and_then(|v| v.get(i))
        .ok_or("Legend no longer exists")?;
    admitted(v)?;
    Ok(ingetrazo_core::composition::RectMm::new(
        v["x_mm"].as_f64().unwrap_or(20.),
        v["y_mm"].as_f64().unwrap_or(20.),
        v["w_mm"].as_f64().unwrap_or(55.),
        7.5 + 5.5
            * v.get("rows")
                .and_then(Value::as_array)
                .map_or(1, |v| v.len().max(1)) as f64,
    ))
}
pub(super) fn locked(sheet: &ingetrazo_core::composition::Sheet, i: usize) -> bool {
    sheet.native_document["leyendas"][i]
        .get("locked")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
pub(super) fn acquire(
    app: &mut IngeTrazoApp,
    s: usize,
    i: usize,
    pos: egui::Pos2,
    paper: egui::Rect,
    zoom: f32,
) -> Result<(), String> {
    let r = rect(&app.scene.sheets[s], i)?;
    let targets = super::pointer_transactions::Targets::capture_legend(&app.scene.sheets[s], i)?;
    app.composer_select_legend(Some(i))?;
    let corner = paper.min + egui::vec2((r.x + r.w) as f32 * zoom, (r.y + r.h) as f32 * zoom);
    let resize = app.composer_legend_selection().is_some()
        && (pos.x - corner.x).abs() <= 3. * zoom
        && (pos.y - corner.y).abs() <= 3. * zoom;
    app.composer_state.legend_authoring.grab = Some(Grab {
        sheet: app.scene.sheets[s].id,
        index: i,
        press: pos,
        original: r,
        resize,
        targets,
    });
    Ok(())
}
pub(super) fn motion(
    app: &mut IngeTrazoApp,
    s: usize,
    pos: egui::Pos2,
    paper: egui::Rect,
    zoom: f32,
) -> Result<(), String> {
    let Some(g) = app.composer_state.legend_authoring.grab.as_ref() else {
        return Ok(());
    };
    if app.scene.sheets[s].id != g.sheet {
        return Err("Legend sheet changed".into());
    }
    let original = g.original;
    let (sid, index) = (g.sheet, g.index);
    let (x, y, w) = if g.resize {
        (
            original.x,
            original.y,
            (f64::from((pos.x - paper.min.x) / zoom) - original.x).max(25.),
        )
    } else {
        let dx = f64::from((pos.x - g.press.x) / zoom);
        let dy = f64::from((pos.y - g.press.y) / zoom);
        let mut x = original.x + dx;
        let mut y = original.y + dy;
        x = super::pointer_transactions::Targets::snap(x, &g.targets.x);
        x = super::pointer_transactions::Targets::snap(x + original.w, &g.targets.x) - original.w;
        y = super::pointer_transactions::Targets::snap(y, &g.targets.y);
        y = super::pointer_transactions::Targets::snap(y + original.h, &g.targets.y) - original.h;
        (x, y, original.w)
    };
    if [x, y, w].iter().any(|n| !n.is_finite() || n.abs() > 1e6) || w <= 10. {
        return Err("Invalid legend gesture geometry".into());
    }
    app.composer_state.legend_authoring.selected = Some((sid, index));
    let current = &app.scene.sheets[s].native_document["leyendas"][index];
    if current["x_mm"].as_f64() != Some(x)
        || current["y_mm"].as_f64() != Some(y)
        || current["w_mm"].as_f64() != Some(w)
    {
        app.begin_composer_edit();
        let current = &mut app.scene.sheets[s].native_document["leyendas"][index];
        current["x_mm"] = json!(x);
        current["y_mm"] = json!(y);
        current["w_mm"] = json!(w);
    }
    Ok(())
}

pub(super) fn paint(
    app: &mut IngeTrazoApp,
    painter: &egui::Painter,
    s: usize,
    i: usize,
    zoom: f32,
    origin: egui::Pos2,
) -> Result<(), String> {
    let sheet = &app.scene.sheets[s];
    let id = sheet.id;
    let v = &sheet.native_document["leyendas"][i];
    let r = rect(sheet, i)?;
    let state = &mut app.composer_state.legend_authoring;
    if state.cache_sheet != Some(id) {
        state.cache.clear();
        state.cache_sheet = Some(id);
    }
    let same = state.cache.get(&i).is_some_and(|p| {
        ingetrazo_core::i18n::language_is_current(&p.language)
            && ["w_mm", "title", "rows"]
                .iter()
                .all(|k| p.value.get(k) == v.get(k))
    });
    if !same {
        admitted(v)?;
        let raw = v.to_string().len();
        if raw > 2 * 1024 * 1024 {
            return Err("Legend preview record exceeds budget".into());
        }
        let mut local = v.clone();
        local["x_mm"] = json!(0.);
        local["y_mm"] = json!(0.);
        let draws =
            crate::composer_legend::draws(&local, &mut app.composer_state.paper_text.borrow_mut())?
                .draws;
        let mut parts = Vec::new();
        let mut bytes = raw;
        let mut fonts = std::collections::HashSet::new();
        for draw in draws {
            bytes = bytes
                .checked_add(
                    draw.path.len() * 24 + draw.text.as_ref().map_or(0, |t| t.glyphs.len() * 24),
                )
                .ok_or("Legend preview budget overflow")?;
            if let Some(t) = &draw.text {
                if fonts.insert(t.font.key()) {
                    bytes = bytes
                        .checked_add(t.font.shared_bytes().len())
                        .ok_or("Legend font budget overflow")?;
                }
                parts.push(Part::Text(
                    ingetrazo_core::composition::TextItem {
                        id: uuid::Uuid::new_v4(),
                        pos: (0., 0.),
                        text: String::new(),
                        font_size_pt: 10.,
                        bold: false,
                    },
                    std::sync::Arc::new(vec![draw]),
                ));
            } else {
                parts.push(Part::Path(draw));
            }
        }
        if bytes > 32 * 1024 * 1024 {
            return Err("Legend preview budget exceeded".into());
        }
        while state.cache.len() >= 128
            || state.cache.values().map(|v| v.bytes).sum::<usize>() + bytes > 64 * 1024 * 1024
        {
            let Some(key) = state.cache.keys().copied().find(|&key| key != i) else {
                break;
            };
            state.cache.remove(&key);
        }
        state
            .cache
            .try_reserve(1)
            .map_err(|_| "Legend preview cache allocation refused")?;
        state.cache.insert(
            i,
            Preview {
                language: ingetrazo_core::i18n::current_language(),
                value: v.clone(),
                draws: parts,
                bytes,
            },
        );
        state.cache_builds += 1;
    }
    let base = origin + egui::vec2(r.x as f32 * zoom, r.y as f32 * zoom);
    let color = |c: [f32; 4]| {
        egui::Color32::from_rgba_unmultiplied(
            (c[0] * 255.).round() as u8,
            (c[1] * 255.).round() as u8,
            (c[2] * 255.).round() as u8,
            (c[3] * 255.).round() as u8,
        )
    };
    for part in &state.cache[&i].draws {
        match part {
            Part::Text(item, draws) => {
                app.composer_state.text_canvas.paint_prepared(
                    painter,
                    draws,
                    item,
                    zoom,
                    base,
                    app.native_composer.factory.as_ref(),
                )?;
            }
            Part::Path(draw) => {
                let mut bounds = egui::Rect::NOTHING;
                for op in &draw.path {
                    if let ingetrazo_formats::sheet_pdf::PathOp::Move(p)
                    | ingetrazo_formats::sheet_pdf::PathOp::Line(p) = op
                    {
                        bounds.extend_with(
                            base + egui::vec2(
                                (draw.origin_mm[0] + p[0]) as f32 * zoom,
                                (draw.origin_mm[1] + p[1]) as f32 * zoom,
                            ),
                        );
                    }
                }
                if let Some(fill) = draw.fill {
                    painter.rect_filled(bounds, 0., color(fill));
                }
                if let Some((c, w)) = draw.stroke {
                    painter.rect_stroke(
                        bounds,
                        0.,
                        egui::Stroke::new(w as f32 * zoom, color(c)),
                    );
                }
            }
        }
    }
    if state.selected == Some((id, i)) {
        super::render_selection_box(
            painter,
            egui::Rect::from_min_size(base, egui::vec2(r.w as f32 * zoom, r.h as f32 * zoom)),
        );
    }
    Ok(())
}

pub(super) fn properties(app: &mut IngeTrazoApp, ui: &mut egui::Ui, s: usize) {
    let count = app.scene.sheets[s]
        .native_document
        .get("leyendas")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut select = None;
    let mut title = None;
    let mut refresh = false;
    let mut delete = false;
    egui::Window::new(ingetrazo_core::i18n::tr("Layer legend"))
        .id(egui::Id::new("legend-properties"))
        .default_pos(egui::pos2(800., 160.))
        .show(ui.ctx(), |ui| {
            egui::ScrollArea::vertical()
                .max_height(150.)
                .show_rows(ui, 20., count, |ui, range| {
                    for i in range {
                        let value = &app.scene.sheets[s].native_document["leyendas"][i];
                        let label = value
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("LEYENDA");
                        if ui
                            .selectable_label(
                                app.composer_legend_selection().is_some_and(|v| v.1 == i),
                                label,
                            )
                            .clicked()
                        {
                            select = Some(i);
                        }
                    }
                });
            if let Some((sid, i)) = app.composer_legend_selection() {
                let actual = app.scene.sheets[s].native_document["leyendas"][i]
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("LEYENDA");
                if app
                    .composer_state
                    .legend_authoring
                    .title_draft
                    .as_ref()
                    .is_none_or(|(id, index, _)| *id != sid || *index != i)
                {
                    app.composer_state.legend_authoring.title_draft = Some((sid, i, actual.into()));
                }
                ui.label(ingetrazo_core::i18n::tr("Title"));
                let response = ui.text_edit_singleline(
                    &mut app
                        .composer_state
                        .legend_authoring
                        .title_draft
                        .as_mut()
                        .unwrap()
                        .2,
                );
                observe(ui, "legend-ui-title", response.rect);
                if response.lost_focus()
                    || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    title = Some(
                        app.composer_state
                            .legend_authoring
                            .title_draft
                            .as_ref()
                            .unwrap()
                            .2
                            .clone(),
                    );
                }
                let response = ui.button(ingetrazo_core::i18n::tr("Refresh layers"));
                observe(ui, "legend-ui-refresh", response.rect);
                refresh = response.clicked();
                let response = ui.button(ingetrazo_core::i18n::tr("Delete"));
                observe(ui, "legend-ui-delete", response.rect);
                delete = response.clicked();
            }
        });
    if let Some(i) = select {
        if let Err(e) = app.composer_select_legend(Some(i)) {
            app.status_message = e;
        }
    }
    if let Some(t) = title {
        if let Err(e) = app.composer_edit_legend_title(&t) {
            app.status_message = e;
        }
    }
    if refresh {
        if let Err(e) = app.refresh_composer_legend() {
            app.status_message = e;
        }
    }
    if delete {
        if let Err(e) = app.delete_composer_legend() {
            app.status_message = e;
        }
    }
}
pub(super) fn observe(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new(key), rect));
    }
}
pub(super) fn placement(
    app: &mut IngeTrazoApp,
    ui: &egui::Ui,
    paper: egui::Rect,
    toolbar: egui::Rect,
    response: &egui::Response,
    zoom: f32,
) -> bool {
    if !app.composer_state.legend_authoring.armed {
        return false;
    }
    if !ui.is_enabled() || ui.ctx().output(|o| o.num_completed_passes) != 0 {
        return true;
    }
    let events = ui.input(|i| i.events.clone());
    for e in events {
        match e {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } if paper.contains(pos)
                && !toolbar.contains(pos)
                && ui.ctx().layer_id_at(pos) == Some(response.layer_id) =>
            {
                app.composer_state.legend_authoring.pending = Some(glam::DVec2::new(
                    f64::from((pos.x - paper.min.x) / zoom),
                    f64::from((pos.y - paper.min.y) / zoom),
                ));
            }
            egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(point) = app.composer_state.legend_authoring.pending.take() {
                    if let Err(e) = app.place_composer_legend(point) {
                        app.status_message = e;
                    }
                }
            }
            egui::Event::PointerGone => {
                app.composer_state.legend_authoring.pending = None;
            }
            _ => {}
        }
    }
    true
}
