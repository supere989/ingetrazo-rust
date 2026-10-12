//! Six source-backed composer annotation owners. Raw fields remain authoritative.
use super::IngeTrazoApp;
use glam::DVec2;
use serde_json::{json, Value};
use uuid::Uuid;
#[path = "composer_annotation_preferences.rs"]
mod preferences;
pub(super) const FAMILIES: [&str; 6] = [
    "cotas",
    "cotas_ang",
    "cotas_rad",
    "etiquetas",
    "niveles",
    "llamadas",
];
pub(super) const LABELS: [&str; 6] = [
    "Dimension",
    "Angular dimension",
    "Radius / diameter",
    "Leader label",
    "Level mark",
    "Detail callout",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Owner {
    pub sheet: Uuid,
    pub family: usize,
    pub index: usize,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub ignore_release: bool,
    pub hover: Option<DVec2>,
    pub armed: Option<usize>,
    pub points: Vec<DVec2>,
    pub selected: Option<Owner>,
    pub grab: Option<Grab>,
    pub coalesce: Option<(Owner, Vec<String>, usize, u64)>,
    pub draft: Option<(Owner, Value)>,
    pub canvas: super::annotation_canvas::Cache,
    pub geometry_epoch: u64,
    pub layouts: LayoutCache,
    pub last_cota: serde_json::Map<String, Value>,
    pub last_level: serde_json::Map<String, Value>,
    pub axis: &'static str,
}
#[derive(Clone)]
pub(super) struct Grab {
    pub owner: Owner,
    pub press: DVec2,
    pub before: Value,
    pub targets: super::pointer_transactions::Targets,
    pub size: [f64; 2],
}
fn active(app: &IngeTrazoApp) -> Result<usize, String> {
    app.scene
        .active_sheet_index
        .filter(|&s| s < app.scene.sheets.len())
        .ok_or_else(|| "No active sheet".into())
}
pub(super) fn record(app: &IngeTrazoApp, owner: Owner) -> Option<&Value> {
    let s = app.scene.sheets.iter().find(|s| s.id == owner.sheet)?;
    s.native_document
        .get(*FAMILIES.get(owner.family)?)
        .and_then(Value::as_array)?
        .get(owner.index)
}
fn defaults(family: usize) -> Result<Value, String> {
    let key = FAMILIES.get(family).ok_or("Unknown annotation family")?;
    let all: Value = serde_json::from_str(include_str!("composer_annotation_defaults.json"))
        .map_err(|e| e.to_string())?;
    Ok(all[*key].clone())
}
pub(super) fn admit(v: &Value) -> Result<(), String> {
    let o = v.as_object().ok_or("Invalid annotation record")?;
    fn check(v: &Value, budget: &mut usize, depth: usize) -> Result<(), String> {
        if depth > 16 {
            return Err("Annotation nesting limit exceeded".into());
        }
        *budget = budget
            .checked_add(1)
            .ok_or("Annotation inventory overflow")?;
        if *budget > 100_000 {
            return Err("Annotation inventory limit exceeded".into());
        }
        match v {
            Value::Number(n) => {
                if n.as_f64().is_none_or(|x| !x.is_finite()) {
                    return Err("Invalid annotation numeric field".into());
                }
            }
            Value::String(s) => {
                if s.len() > 1024 * 1024 {
                    return Err("Annotation text limit exceeded".into());
                }
            }
            Value::Array(a) => {
                for x in a {
                    check(x, budget, depth + 1)?
                }
            }
            Value::Object(o) => {
                for x in o.values() {
                    check(x, budget, depth + 1)?
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut count = 0;
    for (k, v) in o {
        if KNOWN_FIELDS.contains(&k.as_str()) {
            check(v, &mut count, 0)?;
        }
    }
    for k in [
        "x_mm",
        "y_mm",
        "dx_mm",
        "dy_mm",
        "ax_mm",
        "ay_mm",
        "bx_mm",
        "by_mm",
        "w_mm",
        "h_mm",
        "z",
        "sep_mm",
        "radius_mm",
        "angle_deg",
        "scale_n",
        "size_mm",
        "size_pt",
        "text_mm",
        "stroke_mm",
        "line_mm",
        "offset_mm",
        "text_dx_mm",
        "text_dy_mm",
        "z_m",
        "datum_m",
        "bg_opacity",
        "text_bg_opacity",
        "decimals",
    ] {
        if o.get(k)
            .is_some_and(|v| v.as_f64().is_none_or(|x| !x.is_finite() || x.abs() > 1e6))
        {
            return Err("Invalid annotation geometry/style field".into());
        }
    }
    for k in ["hidden", "locked"] {
        if o.get(k).is_some_and(|v| !v.is_boolean()) {
            return Err("Invalid annotation visibility".into());
        }
    }
    for k in ["x_mm", "y_mm", "z"] {
        if o.get(k).is_some_and(|v| v.as_f64().is_none()) {
            return Err("Invalid annotation geometry".into());
        }
    }
    Ok(())
}
fn patch(v: &mut Value, fields: &Value) -> Result<(), String> {
    for (k, x) in fields.as_object().ok_or("Invalid annotation edit")? {
        v[k] = x.clone();
    }
    admit(v)
}
impl IngeTrazoApp {
    pub fn composer_annotation_selection(&self) -> Option<(Uuid, usize, usize)> {
        let owner = self.composer_state.annotation_authoring.selected?;
        let s = active(self).ok()?;
        if self.scene.sheets[s].id != owner.sheet
            || self.scene.sheets[s].selected_item.is_some()
            || self.composer_state.picture_authoring.selected.is_some()
            || self.composer_state.legend_authoring.selected.is_some()
        {
            return None;
        }
        record(self, owner)?;
        Some((owner.sheet, owner.family, owner.index))
    }
    pub(super) fn revalidate_annotation_selection(&mut self) {
        if self.composer_annotation_selection().is_none() {
            self.composer_state.annotation_authoring.selected = None;
            self.composer_state.annotation_authoring.draft = None;
        }
    }
    pub fn composer_select_annotation(
        &mut self,
        family: usize,
        index: Option<usize>,
    ) -> Result<(), String> {
        let s = active(self)?;
        let id = self.scene.sheets[s].id;
        let owner = index.map(|index| Owner {
            sheet: id,
            family,
            index,
        });
        if let Some(o) = owner {
            let v = record(self, o).ok_or("Annotation no longer exists")?;
            admit(v)?;
            if v.get("hidden").and_then(Value::as_bool) == Some(true) {
                self.composer_state.annotation_authoring.selected = None;
                self.scene.sheets[s].selected_item = None;
                self.composer_state.picture_authoring.selected = None;
                self.composer_state.legend_authoring.selected = None;
                return Ok(());
            }
        }
        self.finish_composer_edit();
        self.composer_state.annotation_authoring.coalesce = None;
        self.composer_state.annotation_authoring.selected = owner;
        self.composer_state.annotation_authoring.draft = None;
        if owner.is_some() {
            self.scene.sheets[s].selected_item = None;
            self.composer_state.picture_authoring.selected = None;
            self.composer_state.legend_authoring.selected = None;
        }
        Ok(())
    }
    pub fn activate_composer_annotation(&mut self, family: usize) -> Result<(), String> {
        active(self)?;
        FAMILIES.get(family).ok_or("Unknown annotation family")?;
        if self
            .composer_state
            .annotation_authoring
            .last_cota
            .is_empty()
        {
            self.composer_state.annotation_authoring.last_cota =
                preferences::load(&self.document_session.preferences);
        }
        self.finish_composer_edit();
        self.composer_state.text_tool = false;
        self.composer_state.shape_tool = None;
        self.composer_state.legend_authoring.armed = false;
        self.composer_state.legend_authoring.pending = None;
        self.composer_state.annotation_authoring.armed = Some(family);
        self.composer_state.annotation_authoring.points.clear();
        self.composer_state.annotation_authoring.coalesce = None;
        Ok(())
    }
    pub fn composer_annotation_pending(&self) -> (Option<usize>, Vec<[f64; 2]>) {
        (
            self.composer_state.annotation_authoring.armed,
            self.composer_state
                .annotation_authoring
                .points
                .iter()
                .map(|p| p.to_array())
                .collect(),
        )
    }
    pub fn cancel_composer_annotation_placement(&mut self) {
        if self.composer_state.annotation_authoring.points.is_empty() {
            self.composer_state.annotation_authoring.armed = None;
        } else {
            self.composer_state.annotation_authoring.points.clear();
        }
    }
    pub fn composer_annotation_preview_stats(&self) -> (u64, u64, usize) {
        self.composer_state.annotation_authoring.canvas.stats()
    }
    pub fn edit_composer_annotation(&mut self, fields: Value) -> Result<(), String> {
        let (sid, family, index) = self
            .composer_annotation_selection()
            .ok_or("No annotation selected")?;
        let owner = Owner {
            sheet: sid,
            family,
            index,
        };
        let s = active(self)?;
        let before = record(self, owner).ok_or("Annotation no longer exists")?;
        raw_bytes(before)?;
        let mut candidate = before.clone();
        patch(&mut candidate, &fields)?;
        let unchanged = candidate == *before;
        let mut keys = fields
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        if family == 0 {
            self.composer_state.annotation_authoring.last_cota =
                preferences::remember(&mut self.document_session.preferences, &candidate);
            self.persist_preferences();
        }
        if family == 4 {
            for k in [
                "text",
                "symbol",
                "datum_m",
                "decimals",
                "size_mm",
                "line_mm",
                "mirror",
                "stroke_mm",
            ] {
                if let Some(v) = candidate.get(k) {
                    self.composer_state
                        .annotation_authoring
                        .last_level
                        .insert(k.into(), v.clone());
                }
            }
        }
        if unchanged {
            return Ok(());
        }
        let n = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        let combine = self
            .composer_state
            .annotation_authoring
            .coalesce
            .as_ref()
            .is_some_and(|(o, k, count, version)| {
                *o == owner && *k == keys && *count == n && *version == self.scene.version
            });
        if !combine {
            self.finish_composer_edit();
            self.begin_composer_edit();
        }
        self.scene.sheets[s].native_document[FAMILIES[family]][index] = candidate;
        if combine {
            self.scene.bump_version();
        } else {
            self.finish_composer_edit();
        }
        let n = self
            .composer_state
            .sheet_history
            .get(&sid)
            .map_or(0, |h| h.0.len());
        self.composer_state.annotation_authoring.coalesce =
            Some((owner, keys, n, self.scene.version));
        self.composer_state.annotation_authoring.draft = None;
        Ok(())
    }
    pub fn delete_composer_annotation(&mut self) -> Result<(), String> {
        let (sid, family, index) = self
            .composer_annotation_selection()
            .ok_or("No annotation selected")?;
        let owner = Owner {
            sheet: sid,
            family,
            index,
        };
        let s = active(self)?;
        self.finish_composer_edit();
        self.begin_composer_edit();
        let list = self.scene.sheets[s].native_document[FAMILIES[family]]
            .as_array_mut()
            .ok_or("Invalid annotation list")?;
        list.remove(index);
        self.composer_state.annotation_authoring.selected = None;
        self.composer_state.annotation_authoring.draft = None;
        self.composer_state.annotation_authoring.coalesce = None;
        self.finish_composer_edit();
        Ok(())
    }
    pub fn place_composer_annotation(
        &mut self,
        points: &[[f64; 2]],
        diameter: bool,
    ) -> Result<bool, String> {
        let s = active(self)?;
        let family = self
            .composer_state
            .annotation_authoring
            .armed
            .ok_or("No annotation tool armed")?;
        if points
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 1e6)
        {
            return Err("Invalid annotation placement".into());
        }
        let required = [3, 4, 2, 2, 1, 2][family];
        if points.len() != required {
            return Err("Incomplete annotation placement".into());
        }
        let p = points
            .iter()
            .map(|p| DVec2::from_array(*p))
            .collect::<Vec<_>>();
        let mut v = defaults(family)?;
        if family <= 2 {
            for (k, x) in &self.composer_state.annotation_authoring.last_cota {
                if family == 0
                    || family == 1
                        && matches!(k.as_str(), "text_mm" | "stroke_mm" | "color" | "text_color")
                    || family == 2
                {
                    v[k] = x.clone();
                }
            }
        }
        if family == 4 {
            for (k, x) in &self.composer_state.annotation_authoring.last_level {
                v[k] = x.clone();
            }
        }
        v["x_mm"] = json!(p[0].x);
        v["y_mm"] = json!(p[0].y);
        let scale = self.scene.sheets[s]
            .items
            .iter()
            .find_map(|i| {
                if let ingetrazo_core::composition::SheetItem::ViewFrame(f) = i {
                    Some(f.scale_n)
                } else {
                    None
                }
            })
            .unwrap_or(100.);
        match family {
            0 => {
                let d = p[1] - p[0];
                if d.length() < 0.5 {
                    return Ok(false);
                }
                v["offset_mm"] = json!(self
                    .composer_state
                    .annotation_authoring
                    .last_cota
                    .get("offset_mm")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.5));
                v["ends"] = json!(self
                    .composer_state
                    .annotation_authoring
                    .last_cota
                    .get("ends")
                    .and_then(Value::as_str)
                    .unwrap_or("arrow"));
                v["text_pos"] = json!("above");
                v["text_align"] = json!("aligned");
                v["text_along"] = json!("middle");
                v["axis"] = json!(self.composer_state.annotation_authoring.axis);
                v["dx_mm"] = json!(d.x);
                v["dy_mm"] = json!(d.y);
                v["sep_mm"] = json!((p[2] - p[0]).dot(DVec2::new(-d.y, d.x) / d.length()));
                v["scale_n"] = json!(scale);
            }
            1 => {
                let a = p[1] - p[0];
                let b = p[2] - p[0];
                if a.length() < 1e-6 || b.length() < 1e-6 {
                    return Ok(false);
                }
                for (k, x) in [
                    ("ax_mm", a.x),
                    ("ay_mm", a.y),
                    ("bx_mm", b.x),
                    ("by_mm", b.y),
                    ("radius_mm", (p[3] - p[0]).length().max(2.)),
                ] {
                    v[k] = json!(x);
                }
            }
            2 => {
                let d = p[1] - p[0];
                if d.length() < 0.5 {
                    return Ok(false);
                }
                v["radius_mm"] = json!(d.length());
                v["angle_deg"] = json!(d.y.atan2(d.x).to_degrees());
                v["scale_n"] = json!(scale);
                v["kind"] = json!(if diameter { "diameter" } else { "radius" });
            }
            3 => {
                v["x_mm"] = json!(p[1].x);
                v["y_mm"] = json!(p[1].y);
                v["ax_mm"] = json!(p[0].x - p[1].x);
                v["ay_mm"] = json!(p[0].y - p[1].y);
                v["text"] = json!(ingetrazo_core::i18n::tr("Label"));
            }
            4 => {}
            5 => {
                let w = (p[1].x - p[0].x).abs().max(8.);
                let h = (p[1].y - p[0].y).abs().max(8.);
                v["x_mm"] = json!(p[0].x.min(p[1].x));
                v["y_mm"] = json!(p[0].y.min(p[1].y));
                v["w_mm"] = json!(w);
                v["h_mm"] = json!(h);
                let bubble = (number(&v, "size_mm", 3.) * 2.4).max(3.);
                v["bx_mm"] = json!(w + bubble);
                v["by_mm"] = json!(-bubble * 0.9);
                let count = self.scene.sheets[s]
                    .native_document
                    .get("llamadas")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                v["number"] = json!((count + 1).to_string());
            }
            _ => unreachable!(),
        }
        v["z"] = json!(super::picture_authoring::next_z(&self.scene.sheets[s])?);
        admit(&v)?;
        let count = self.scene.sheets[s]
            .native_document
            .get(FAMILIES[family])
            .map(|a| a.as_array().ok_or("Invalid annotation inventory"))
            .transpose()?
            .map_or(0, Vec::len);
        if count >= 100_000 {
            return Err("Annotation inventory limit exceeded".into());
        }
        if let Some(list) = self.scene.sheets[s]
            .native_document
            .get_mut(FAMILIES[family])
            .and_then(Value::as_array_mut)
        {
            list.try_reserve(1)
                .map_err(|_| "Annotation inventory allocation refused")?;
        }
        let mut fresh = Vec::new();
        if count == 0
            && self.scene.sheets[s]
                .native_document
                .get(FAMILIES[family])
                .is_none()
        {
            fresh
                .try_reserve_exact(1)
                .map_err(|_| "Annotation inventory allocation refused")?;
        }
        self.finish_composer_edit();
        self.begin_composer_edit();
        let raw = &mut self.scene.sheets[s].native_document;
        if raw.get(FAMILIES[family]).is_none() {
            fresh.push(v);
            raw[FAMILIES[family]] = Value::Array(fresh);
        } else {
            raw[FAMILIES[family]].as_array_mut().unwrap().push(v);
        }
        self.composer_state.annotation_authoring.selected = Some(Owner {
            sheet: self.scene.sheets[s].id,
            family,
            index: count,
        });
        self.scene.sheets[s].selected_item = None;
        self.composer_state.picture_authoring.selected = None;
        self.composer_state.legend_authoring.selected = None;
        self.composer_state.annotation_authoring.armed =
            if family <= 2 { Some(family) } else { None };
        self.composer_state.annotation_authoring.points.clear();
        self.composer_state.annotation_authoring.axis = "";
        self.finish_composer_edit();
        Ok(true)
    }
}

fn property_fields(f: usize) -> &'static [(&'static str, &'static str)] {
    match f {
        0 => &[
            ("scale_n", "Scale denominator"),
            ("text", "Text (<> = measurement)"),
            ("sep_mm", "Separation (mm)"),
            ("text_mm", "Text height (mm)"),
            ("decimals", "Decimal places"),
            ("units", "Units"),
            ("ends", "Ends"),
            ("stroke_mm", "Line width (mm)"),
            ("text_pos", "Text position"),
            ("axis", "Measurement axis"),
            ("text_along", "Along line"),
            ("text_align", "Text alignment"),
            ("text_color", "Text color"),
        ],
        1 => &[
            ("text", "Text (<> = angle)"),
            ("radius_mm", "Arc radius (mm)"),
            ("text_mm", "Text height (mm)"),
            ("decimals", "Decimal places"),
            ("ends", "Ends"),
            ("stroke_mm", "Line width (mm)"),
        ],
        2 => &[
            ("kind", "Radius or diameter"),
            ("text", "Text (<> = measurement)"),
            ("radius_mm", "Radius (mm)"),
            ("angle_deg", "Direction (degrees)"),
            ("placement", "Text placement"),
            ("centre_mark", "Center mark"),
            ("text_mm", "Text height (mm)"),
            ("decimals", "Decimal places"),
            ("units", "Units"),
            ("ends", "Ends"),
            ("stroke_mm", "Line width (mm)"),
        ],
        3 => &[
            ("text", "Label text"),
            ("size_pt", "Text size (pt)"),
            ("bold", "Bold"),
            ("italic", "Italic"),
            ("underline", "Underline"),
            ("arrow", "Arrow"),
            ("dot", "Landing dot"),
            ("stroke_mm", "Line width (mm)"),
        ],
        4 => &[
            ("text", "Level text ({z} = elevation)"),
            ("symbol", "Symbol"),
            ("z_m", "Elevation (m)"),
            ("datum_m", "Datum (m)"),
            ("decimals", "Decimal places"),
            ("size_mm", "Text height (mm)"),
            ("line_mm", "Level line length (mm)"),
            ("mirror", "Mirror layout"),
            ("stroke_mm", "Line width (mm)"),
        ],
        _ => &[
            ("number", "Detail number"),
            ("sheet", "Sheet reference"),
            ("shape", "Boundary"),
            ("size_mm", "Bubble text height (mm)"),
            ("stroke_mm", "Line width (mm)"),
            ("follow", "Follow view"),
        ],
    }
}
fn choices(k: &str) -> &'static [&'static str] {
    match k {
        "units" => &[
            "m",
            "cm",
            "mm",
            "in",
            "ft",
            "ft-in",
            "in-frac",
            "ft-in-frac",
        ],
        "ends" => &["arrow", "tick", "none"],
        "text_pos" => &["above"],
        "axis" => &["", "h", "v"],
        "text_along" => &["middle", "start", "end"],
        "text_align" => &["aligned", "horizontal"],
        "kind" => &["radius", "diameter"],
        "placement" => &["auto", "in", "out"],
        "symbol" => &["triangle", "circle"],
        "shape" => &["rect", "circle"],
        _ => &[],
    }
}
pub(super) fn observe(ui: &egui::Ui, key: &str, rect: egui::Rect) {
    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new(key), rect));
    }
}
pub(super) fn properties(app: &mut IngeTrazoApp, ui: &mut egui::Ui, s: usize) {
    let Some((sid, f, index)) = app.composer_annotation_selection() else {
        return;
    };
    let owner = Owner {
        sheet: sid,
        family: f,
        index,
    };
    let Some(actual) = record(app, owner) else {
        return;
    };
    if app
        .composer_state
        .annotation_authoring
        .draft
        .as_ref()
        .is_none_or(|(o, _)| *o != owner)
    {
        let mut draft = actual.clone();
        normalize_property_draft(f, &mut draft);
        app.composer_state.annotation_authoring.draft = Some((owner, draft));
    }
    let mut changed = false;
    let mut delete = false;
    let mut reset = false;
    egui::Window::new(ingetrazo_core::i18n::tr(LABELS[f]))
        .id(egui::Id::new("annotation-properties"))
        .default_pos(egui::pos2(800., 160.))
        .show(ui.ctx(), |ui| {
            ui.set_enabled(true);
            let draft = &mut app
                .composer_state
                .annotation_authoring
                .draft
                .as_mut()
                .unwrap()
                .1;
            for &(key, label) in property_fields(f) {
                if key == "z_m"
                    && draft
                        .get("anchor_uid")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                    && draft.get("a_world").is_some_and(|v| !v.is_null())
                {
                    continue;
                }
                ui.label(ingetrazo_core::i18n::tr(label));
                let old = draft.get(key).cloned().unwrap_or(Value::Null);
                let rect = if let Some(mut b) = old.as_bool() {
                    let r = ui.checkbox(&mut b, ingetrazo_core::i18n::tr(label));
                    if r.changed() {
                        draft[key] = json!(b);
                        changed = true;
                    }
                    r.rect
                } else if let Some(mut n) = old.as_f64() {
                    let range = if key == "decimals" {
                        0. ..=6.
                    } else if matches!(
                        key,
                        "stroke_mm" | "size_pt" | "size_mm" | "text_mm" | "radius_mm" | "scale_n"
                    ) {
                        0.01..=100000.
                    } else {
                        -100000. ..=100000.
                    };
                    let r = ui.add(egui::DragValue::new(&mut n).range(range).speed(0.1));
                    if r.changed() {
                        draft[key] = if key == "decimals" {
                            json!(n.round() as i64)
                        } else {
                            json!(n)
                        };
                        changed = true;
                    }
                    r.rect
                } else {
                    let mut text = old.as_str().unwrap_or("").to_string();
                    let options = choices(key);
                    if options.is_empty() {
                        let r = if f == 3 && key == "text" {
                            ui.text_edit_multiline(&mut text)
                        } else {
                            ui.text_edit_singleline(&mut text)
                        };
                        draft[key] = json!(text);
                        if r.changed() && !(matches!(f, 0 | 1 | 2) && key == "text")
                            || r.lost_focus()
                            || (r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            changed = true;
                        }
                        r.rect
                    } else {
                        let response = egui::ComboBox::from_id_salt(("annotation-property", key))
                            .selected_text(&text)
                            .show_ui(ui, |ui| {
                                for option in options {
                                    if ui
                                        .selectable_value(
                                            &mut text,
                                            (*option).into(),
                                            if option.is_empty() {
                                                "Automatic"
                                            } else {
                                                option
                                            },
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                        draft[key] = json!(text);
                        response.response.rect
                    }
                };
                observe(ui, &format!("annotation-ui-property-{key}"), rect);
            }
            if f == 0 {
                reset = ui.button("Reset label position").clicked();
            }
            let r = ui.button(ingetrazo_core::i18n::tr("Delete"));
            observe(ui, "annotation-ui-delete", r.rect);
            delete = r.clicked();
        });
    if changed {
        let draft = &app
            .composer_state
            .annotation_authoring
            .draft
            .as_ref()
            .unwrap()
            .1;
        let mut fields = serde_json::Map::new();
        for (k, _) in property_fields(f) {
            if *k == "z_m"
                && draft
                    .get("anchor_uid")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
                && draft.get("a_world").is_some_and(|v| !v.is_null())
            {
                continue;
            }
            if let Some(v) = draft.get(*k) {
                fields.insert((*k).into(), v.clone());
            }
        }
        if let Err(e) = app.edit_composer_annotation(Value::Object(fields)) {
            app.status_message = e;
        }
    }
    if reset {
        if let Err(e) = app.edit_composer_annotation(
            json!({"text_dx_mm":0.,"text_dy_mm":0.,"text_along":"middle"}),
        ) {
            app.status_message = e;
        }
    }
    if delete {
        if let Err(e) = app.delete_composer_annotation() {
            app.status_message = e;
        }
    }
    let _ = s;
}
pub(super) fn toolbar(app: &mut IngeTrazoApp, ui: &mut egui::Ui) {
    let select = ui.selectable_label(
        app.composer_state.annotation_authoring.armed.is_none(),
        "Select",
    );
    observe(ui, "annotation-ui-select", select.rect);
    if select.clicked() {
        app.composer_state.annotation_authoring.armed = None;
        app.composer_state.annotation_authoring.points.clear();
    }
    let menu = ui.menu_button("+ Annotation", |ui| {
        for (f, label) in LABELS.iter().enumerate() {
            let r = ui.button(ingetrazo_core::i18n::tr(label));
            observe(ui, &format!("annotation-ui-add-{f}"), r.rect);
            if r.clicked() {
                if let Err(e) = app.activate_composer_annotation(f) {
                    app.status_message = e;
                }
                ui.close_menu();
            }
        }
    });
    observe(ui, "annotation-ui-menu", menu.response.rect);
}

#[derive(Clone)]
struct Layout {
    value: Value,
    context: u64,
    language: String,
    version: u64,
    epoch: u64,
    draws: std::sync::Arc<crate::composer_annotations::Draws>,
    bytes: usize,
}
#[derive(Clone, Default)]
pub(super) struct LayoutCache {
    sheet: Option<Uuid>,
    entries: std::collections::HashMap<(usize, usize), Layout>,
    builds: u64,
    order: std::sync::Arc<Vec<(u8, usize, usize)>>,
    pointer_order: std::sync::Arc<Vec<(u8, usize)>>,
    version: u64,
    order_ready: bool,
    context_version: Option<u64>,
    context_key: u64,
}
fn pixel_same(a: &Value, b: &Value) -> bool {
    let Some(a) = a.as_object() else { return false };
    let Some(b) = b.as_object() else { return false };
    let relevant = |k: &str| {
        !matches!(
            k,
            "x_mm" | "y_mm" | "z" | "hidden" | "locked" | "uid" | "group_id" | "list_name"
        ) && !k.starts_with("vendor_")
    };
    a.iter()
        .filter(|(k, _)| relevant(k))
        .all(|(k, v)| b.get(k) == Some(v))
        && b.iter()
            .filter(|(k, _)| relevant(k))
            .all(|(k, v)| a.get(k) == Some(v))
}
pub(super) fn canvas_order(
    app: &mut IngeTrazoApp,
    s: usize,
) -> Result<std::sync::Arc<Vec<(u8, usize, usize)>>, String> {
    let cache = &mut app.composer_state.annotation_authoring.layouts;
    let id = app.scene.sheets[s].id;
    if cache.sheet != Some(id) {
        *cache = LayoutCache {
            sheet: Some(id),
            ..Default::default()
        };
        app.composer_state.annotation_authoring.canvas.clear();
    }
    if !cache.order_ready || cache.version != app.scene.version {
        let mut order = Vec::new();
        for item in app.scene.sheets[s].physical_export_items()? {
            use ingetrazo_core::composition::PhysicalExportItem as I;
            order.push(match item {
                I::Typed { index, .. } => (0, index, 0),
                I::Picture { index, .. } => (1, index, 0),
                I::Legend { index, .. } => (2, index, 0),
                I::Annotation { key, index, .. } => (
                    3,
                    index,
                    FAMILIES
                        .iter()
                        .position(|k| *k == key)
                        .ok_or("Unknown annotation family")?,
                ),
            });
        }
        cache.pointer_order = std::sync::Arc::new(
            order
                .iter()
                .filter_map(|&(k, i, _)| (k != 3).then_some((k, i)))
                .collect(),
        );
        cache.order = std::sync::Arc::new(order);
        cache.version = app.scene.version;
        cache.order_ready = true;
    }
    Ok(cache.order.clone())
}
pub(super) fn paint(
    app: &mut IngeTrazoApp,
    painter: &egui::Painter,
    s: usize,
    f: usize,
    index: usize,
    zoom: f32,
    paper: egui::Pos2,
) -> Result<(), String> {
    let owner = Owner {
        sheet: app.scene.sheets[s].id,
        family: f,
        index,
    };
    let value = app.scene.sheets[s]
        .native_document
        .get(FAMILIES[f])
        .and_then(Value::as_array)
        .and_then(|rows| rows.get(index))
        .ok_or("Annotation no longer exists")?;
    let x = value.get("x_mm").and_then(Value::as_f64).unwrap_or(0.);
    let y = value.get("y_mm").and_then(Value::as_f64).unwrap_or(0.);
    let fields = matches!(f, 3 | 5)
        && value
            .get(if f == 3 { "text" } else { "sheet" })
            .and_then(Value::as_str)
            .is_some_and(|s| s.contains('{'));
    let entry = app
        .composer_state
        .annotation_authoring
        .layouts
        .entries
        .get(&(f, index));
    let changed = entry.is_none_or(|p| {
        p.version != app.scene.version
            || p.epoch != app.composer_state.annotation_authoring.geometry_epoch
            || !ingetrazo_core::i18n::language_is_current(&p.language)
    });
    let prior_context = entry.map_or(0, |p| p.context);
    let context = if changed && fields {
        if app
            .composer_state
            .annotation_authoring
            .layouts
            .context_version
            != Some(app.scene.version)
        {
            let key = context_key(app, s);
            app.composer_state.annotation_authoring.layouts.context_key = key;
            app.composer_state
                .annotation_authoring
                .layouts
                .context_version = Some(app.scene.version);
        }
        app.composer_state.annotation_authoring.layouts.context_key
    } else {
        prior_context
    };
    let entry = app
        .composer_state
        .annotation_authoring
        .layouts
        .entries
        .get(&(f, index));
    let cold = entry.is_none()
        || changed
            && entry.is_some_and(|p| {
                !ingetrazo_core::i18n::language_is_current(&p.language)
                    || p.context != context
                    || !pixel_same(&p.value, value)
            });
    if cold {
        let language = ingetrazo_core::i18n::current_language();
        admit(value)?;
        let raw_bytes = raw_bytes(value)?;
        let mut local = value.clone();
        local["x_mm"] = json!(0.);
        local["y_mm"] = json!(0.);
        let sheet = &app.scene.sheets[s];
        let indices = crate::composer_text_fields::source_indices(
            &app.scene.sheets,
            std::slice::from_ref(sheet),
        )?;
        let date = if fields {
            crate::composer_text_fields::today()?
        } else {
            String::new()
        };
        let context_fields = crate::composer_text_fields::Context {
            index: indices.get(&(sheet as *const _)).copied(),
            total: app.scene.sheets.len(),
            path: app.current_file.as_deref(),
            date: &date,
        };
        let draws = crate::composer_annotations::draws(
            sheet,
            FAMILIES[f],
            &local,
            &mut app.composer_state.paper_text.borrow_mut(),
            &context_fields,
            false,
        )?;
        let mut bytes = raw_bytes;
        let mut fonts = std::collections::HashSet::new();
        for d in &draws.draws {
            bytes = bytes
                .checked_add(
                    std::mem::size_of::<ingetrazo_formats::sheet_pdf::Draw>()
                        + d.clip.as_ref().map_or(0, |v| {
                            v.capacity()
                                * std::mem::size_of::<ingetrazo_formats::sheet_pdf::PathOp>()
                        })
                        + d.clip_intersection.as_ref().map_or(0, |v| {
                            v.capacity()
                                * std::mem::size_of::<ingetrazo_formats::sheet_pdf::PathOp>()
                        }),
                )
                .ok_or("Annotation clip budget overflow")?;
            bytes = bytes
                .checked_add(
                    d.path.capacity() * std::mem::size_of::<ingetrazo_formats::sheet_pdf::PathOp>(),
                )
                .ok_or("Annotation layout budget overflow")?;
            if let Some(t) = &d.text {
                bytes = bytes
                    .checked_add(
                        t.glyphs.capacity()
                            * std::mem::size_of::<ingetrazo_formats::sheet_pdf::TextGlyph>()
                            + t.decorations.capacity()
                                * std::mem::size_of::<ingetrazo_formats::sheet_pdf::TextDecoration>(
                                ),
                    )
                    .ok_or("Annotation glyph budget overflow")?;
                if fonts.insert(t.font.key()) {
                    bytes = bytes
                        .checked_add(t.font.shared_bytes().len())
                        .ok_or("Annotation font budget overflow")?;
                }
            }
        }
        if bytes > 32 * 1024 * 1024 {
            return Err("Annotation layout budget exceeded".into());
        }
        let cache = &mut app.composer_state.annotation_authoring.layouts;
        while cache.entries.len() >= 128
            || cache.entries.values().map(|p| p.bytes).sum::<usize>() + bytes > 64 * 1024 * 1024
        {
            let Some(k) = cache.entries.keys().copied().find(|k| *k != (f, index)) else {
                break;
            };
            cache.entries.remove(&k);
        }
        cache
            .entries
            .try_reserve(1)
            .map_err(|_| "Annotation layout allocation refused")?;
        cache.entries.insert(
            (f, index),
            Layout {
                value: local,
                context,
                language,
                version: app.scene.version,
                epoch: app.composer_state.annotation_authoring.geometry_epoch,
                draws: std::sync::Arc::new(draws),
                bytes,
            },
        );
        cache.builds += 1;
    }
    if let Some(entry) = app
        .composer_state
        .annotation_authoring
        .layouts
        .entries
        .get_mut(&(f, index))
    {
        entry.version = app.scene.version;
        entry.epoch = app.composer_state.annotation_authoring.geometry_epoch;
    }
    let draws = &app.composer_state.annotation_authoring.layouts.entries[&(f, index)].draws;
    let factory = app
        .native_composer
        .factory
        .as_ref()
        .ok_or("Native annotation canvas provider unavailable")?;
    let base = paper + egui::vec2(x as f32 * zoom, y as f32 * zoom);
    let rect = app.composer_state.annotation_authoring.canvas.paint(
        painter,
        factory,
        &mut app.composer_state.text_canvas,
        (f, index),
        draws,
        zoom,
        base,
    )?;
    if app.composer_state.annotation_authoring.selected == Some(owner) {
        super::render_selection_box(painter, rect);
    }
    Ok(())
}

fn context_key(app: &IngeTrazoApp, s: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    fn raw(v: &Value, h: &mut impl Hasher) {
        match v {
            Value::Null => 0u8.hash(h),
            Value::Bool(v) => v.hash(h),
            Value::Number(v) => v.as_f64().map(f64::to_bits).hash(h),
            Value::String(v) => v.hash(h),
            Value::Array(v) => {
                for v in v {
                    raw(v, h)
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    k.hash(h);
                    raw(v, h)
                }
            }
        }
    }
    let sheet = &app.scene.sheets[s];
    sheet.name.hash(&mut h);
    app.scene.sheets.len().hash(&mut h);
    app.current_file.hash(&mut h);
    for i in &sheet.items {
        match i {
            ingetrazo_core::composition::SheetItem::ViewFrame(f) => {
                f.view_key.hash(&mut h);
                f.scale_n.to_bits().hash(&mut h);
                f.rect.w.to_bits().hash(&mut h);
                f.rect.h.to_bits().hash(&mut h);
                if let Some(v) = sheet.native_items.get(&f.id) {
                    raw(v, &mut h)
                }
            }
            ingetrazo_core::composition::SheetItem::TitleBlock(t) => {
                t.project_name.hash(&mut h);
                t.author.hash(&mut h);
                t.date.hash(&mut h);
                t.sheet_number.hash(&mut h);
                t.drawing_title.hash(&mut h);
            }
            _ => {}
        }
    }
    h.finish()
}

fn number(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(d)
}
fn size(v: &Value, f: usize) -> [f64; 2] {
    match f {
        0 => [
            number(v, "dx_mm", 40.).abs().max(2.),
            number(v, "dy_mm", 0.).abs().max(2.),
        ],
        1 => [
            number(v, "ax_mm", 30.)
                .abs()
                .max(number(v, "bx_mm", 0.).abs())
                .max(number(v, "radius_mm", 15.))
                .max(2.),
            number(v, "ay_mm", 0.)
                .abs()
                .max(number(v, "by_mm", -30.).abs())
                .max(number(v, "radius_mm", 15.))
                .max(2.),
        ],
        2 => [(2. * number(v, "radius_mm", 15.)).max(2.); 2],
        3 => [
            number(v, "w_mm", 50.),
            (number(v, "size_pt", 11.)
                * (25.4 / 72.)
                * 1.4
                * (v.get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .matches('\n')
                    .count()
                    + 1) as f64)
                .max(6.),
        ],
        4 => [
            number(v, "line_mm", 14.) + 20.,
            number(v, "size_mm", 2.5) * 2.,
        ],
        _ => [number(v, "w_mm", 30.), number(v, "h_mm", 20.)],
    }
}
// QPainterPathStroker's default SquareCap includes endpoint corner squares.
fn distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l = d.length();
    if l == 0. {
        return (p - a).abs().max_element();
    }
    let u = d / l;
    let t = (p - a).dot(u);
    (p - a).perp_dot(u).abs().max((-t).max(t - l).max(0.))
}

fn hit(app: &IngeTrazoApp, owner: Owner, point: DVec2) -> bool {
    let Some(v) = record(app, owner) else {
        return false;
    };
    let p = point - DVec2::new(number(v, "x_mm", 0.), number(v, "y_mm", 0.));
    if owner.family == 4 {
        let sz = number(v, "size_mm", 2.5);
        let symbol = (sz * 1.2).max(1.5);
        let h = sz * 1.4;
        let sign = if v.get("mirror").and_then(Value::as_bool) == Some(true) {
            -1.
        } else {
            1.
        };
        let line = number(v, "line_mm", 14.);
        let ax = number(v, "ax_mm", 0.);
        let ay = number(v, "ay_mm", 0.);
        let x0 = (-symbol / 2.).min(sign * (line + 0.5)).min(ax).min(0.) - 0.5;
        let x1 = (symbol / 2.).max(sign * (line + 0.5)).max(ax).max(0.) + 0.5;
        let circle = v.get("symbol").and_then(Value::as_str) == Some("circle");
        let y0 = (if circle {
            -symbol / 2. - h
        } else {
            -symbol - h
        })
        .min(ay)
            - 0.5;
        let y1 = (if circle { symbol / 2. } else { 0.5_f64 }).max(ay) + 0.5;
        if p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1 {
            return true;
        }
    }
    let Some(layout) = app
        .composer_state
        .annotation_authoring
        .layouts
        .entries
        .get(&(owner.family, owner.index))
    else {
        return false;
    };
    for d in &layout.draws.draws {
        let mut current = DVec2::from_array(d.origin_mm);
        let mut start = current;
        for op in &d.path {
            match op {
                ingetrazo_formats::sheet_pdf::PathOp::Move(q) => {
                    current = DVec2::from_array(d.origin_mm) + DVec2::from_array(*q);
                    start = current;
                }
                ingetrazo_formats::sheet_pdf::PathOp::Line(q) => {
                    let b = DVec2::from_array(d.origin_mm) + DVec2::from_array(*q);
                    if distance(p, current, b) <= 3. {
                        return true;
                    }
                    current = b;
                }
                ingetrazo_formats::sheet_pdf::PathOp::Cubic(c) => {
                    let base = DVec2::from_array(d.origin_mm);
                    let contour = ingetrazo_render::qt_gray::PathContour {
                        start: current.to_array(),
                        segments: vec![ingetrazo_render::qt_gray::Segment::Cubic {
                            control1: (base + DVec2::new(c[0], c[1])).to_array(),
                            control2: (base + DVec2::new(c[2], c[3])).to_array(),
                            to: (base + DVec2::new(c[4], c[5])).to_array(),
                        }],
                    };
                    if let Ok(points) =
                        ingetrazo_render::qt_gray::flatten_local_path(&contour, 1., 1024)
                    {
                        if points.windows(2).any(|q| {
                            distance(p, DVec2::from_array(q[0]), DVec2::from_array(q[1])) <= 3.
                        }) {
                            return true;
                        }
                    }
                    current = base + DVec2::new(c[4], c[5]);
                }
                ingetrazo_formats::sheet_pdf::PathOp::Close => {
                    if distance(p, current, start) <= 3. {
                        return true;
                    }
                    current = start;
                }
            }
        }
        if let Some(t) = &d.text {
            let mut bounds = egui::Rect::NOTHING;
            for g in &t.glyphs {
                bounds.extend_with(egui::pos2(
                    (d.origin_mm[0] + g.position_px[0] * t.scale_mm) as f32,
                    (d.origin_mm[1] + g.position_px[1] * t.scale_mm) as f32,
                ));
            }
            if bounds
                .expand((100. * t.scale_mm) as f32)
                .contains(egui::pos2(p.x as f32, p.y as f32))
            {
                return true;
            }
        }
    }
    false
}
fn special(v: &Value, f: usize, p: DVec2) -> bool {
    let near = |q: DVec2| (p.x - q.x).abs() <= 3. && (p.y - q.y).abs() <= 3.;
    match f {
        0 => {
            let d = DVec2::new(number(v, "dx_mm", 40.), number(v, "dy_mm", 0.));
            let n = if d.length() > 1e-9 {
                DVec2::new(-d.y, d.x) / d.length()
            } else {
                DVec2::new(0., -1.)
            };
            near(d) || near(d * 0.5 + n * number(v, "sep_mm", 0.))
        }
        1 => {
            let a = number(v, "ay_mm", 0.).atan2(number(v, "ax_mm", 30.));
            let b = number(v, "by_mm", -30.).atan2(number(v, "bx_mm", 0.));
            let mut sweep = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            if sweep <= -std::f64::consts::PI {
                sweep = std::f64::consts::PI;
            }
            near(
                DVec2::new((a + sweep / 2.).cos(), (a + sweep / 2.).sin())
                    * number(v, "radius_mm", 15.),
            )
        }
        2 => {
            let a = number(v, "angle_deg", -30.).to_radians();
            near(DVec2::new(a.cos(), a.sin()) * number(v, "radius_mm", 15.))
        }
        3 => {
            near(DVec2::new(
                number(v, "ax_mm", -20.),
                number(v, "ay_mm", 15.),
            )) || v.get("leaders").and_then(Value::as_array).is_some_and(|a| {
                a.iter()
                    .any(|v| near(DVec2::new(number(v, "ax_mm", 0.), number(v, "ay_mm", 0.))))
            }) || near(DVec2::from_array(size(v, f)))
        }
        5 => {
            near(DVec2::new(number(v, "bx_mm", 36.), number(v, "by_mm", -6.)))
                || near(DVec2::from_array(size(v, f)))
        }
        _ => false,
    }
}
fn motion(app: &mut IngeTrazoApp, p: DVec2) -> Result<(), String> {
    let Some(grab) = app.composer_state.annotation_authoring.grab.take() else {
        return Ok(());
    };
    let result = (|| {
        let s = active(app)?;
        if app.scene.sheets[s].id != grab.owner.sheet {
            return Err("Annotation sheet changed during move".into());
        }
        let d = p - grab.press;
        let (x, y) = grab.targets.position(
            number(&grab.before, "x_mm", 0.) + d.x,
            number(&grab.before, "y_mm", 0.) + d.y,
            grab.size[0],
            grab.size[1],
        );
        let dx = x - number(&grab.before, "x_mm", 0.);
        let dy = y - number(&grab.before, "y_mm", 0.);
        let mut fields = json!({"x_mm":x,"y_mm":y});
        if matches!(grab.owner.family, 3 | 4) {
            fields["ax_mm"] = json!(number(&grab.before, "ax_mm", 0.) - dx);
            fields["ay_mm"] = json!(number(&grab.before, "ay_mm", 0.) - dy);
        }
        if grab.owner.family == 0 && (dx != 0. || dy != 0.) {
            fields["anchor_uid"] = json!("");
            fields["a_world"] = Value::Null;
            fields["b_world"] = Value::Null;
        }
        admit(&fields)?;
        let current = record(app, grab.owner).ok_or("Annotation no longer exists")?;
        let changed = fields
            .as_object()
            .unwrap()
            .iter()
            .any(|(k, v)| current.get(k) != Some(v));
        if !changed {
            return Ok(());
        }
        app.begin_composer_edit();
        let current =
            &mut app.scene.sheets[s].native_document[FAMILIES[grab.owner.family]][grab.owner.index];
        for (k, v) in fields.as_object().unwrap() {
            current[k] = v.clone();
        }
        if grab.owner.family == 3 {
            if let (Some(before), Some(after)) = (
                grab.before.get("leaders").and_then(Value::as_array),
                current.get_mut("leaders").and_then(Value::as_array_mut),
            ) {
                for (was, ld) in before.iter().zip(after.iter_mut()) {
                    ld["ax_mm"] = json!(number(was, "ax_mm", 0.) - dx);
                    ld["ay_mm"] = json!(number(was, "ay_mm", 0.) - dy);
                }
            }
        }
        if matches!(grab.owner.family, 0 | 3 | 4) {
            app.composer_state.annotation_authoring.geometry_epoch = app
                .composer_state
                .annotation_authoring
                .geometry_epoch
                .wrapping_add(1);
        }
        app.composer_state.annotation_authoring.selected = Some(grab.owner);
        app.composer_state.annotation_authoring.coalesce = None;
        Ok(())
    })();
    app.composer_state.annotation_authoring.grab = Some(grab);
    result
}

pub(super) fn input(
    app: &mut IngeTrazoApp,
    ui: &egui::Ui,
    s: usize,
    response: &egui::Response,
    paper: egui::Rect,
    toolbar: egui::Rect,
    zoom: f32,
    order: &[(u8, usize, usize)],
) -> bool {
    if !ui.is_enabled() || ui.ctx().output(|o| o.num_completed_passes) != 0 {
        return app.composer_state.annotation_authoring.armed.is_some()
            || app.composer_state.annotation_authoring.grab.is_some();
    }
    let events = ui.input(|i| i.events.clone());
    let page = |p: egui::Pos2| {
        DVec2::new(
            f64::from((p.x - paper.min.x) / zoom),
            f64::from((p.y - paper.min.y) / zoom),
        )
    };
    let mut owned = app.composer_state.annotation_authoring.armed.is_some()
        || app.composer_state.annotation_authoring.grab.is_some();
    for e in events {
        if matches!(
            &e,
            egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            }
        ) {
            app.composer_state.annotation_authoring.ignore_release = false;
        }
        match e {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if paper.contains(pos)
                && !toolbar.contains(pos)
                && ui.ctx().layer_id_at(pos) == Some(response.layer_id) =>
            {
                let mut p = page(pos);
                if let Some(f) = app.composer_state.annotation_authoring.armed {
                    owned = true;
                    if f == 1
                        && modifiers.shift
                        && !app.composer_state.annotation_authoring.points.is_empty()
                    {
                        let pts = &app.composer_state.annotation_authoring.points;
                        let d = p - pts[0];
                        let base = if pts.len() >= 2 {
                            (pts[1] - pts[0]).y.atan2((pts[1] - pts[0]).x)
                        } else {
                            0.
                        };
                        let angle = ((d.y.atan2(d.x) - base) / (std::f64::consts::PI / 12.))
                            .round_ties_even()
                            * (std::f64::consts::PI / 12.)
                            + base;
                        p = pts[0] + DVec2::new(angle.cos(), angle.sin()) * d.length();
                    }
                    if f == 0
                        && app.composer_state.annotation_authoring.points.len() == 1
                        && modifiers.shift
                    {
                        let d = p - app.composer_state.annotation_authoring.points[0];
                        app.composer_state.annotation_authoring.axis =
                            if d.x.abs() >= d.y.abs() { "h" } else { "v" };
                    }
                    if matches!(f, 0 | 3 | 5)
                        && app.composer_state.annotation_authoring.points.len() == 1
                    {
                        let d = p - app.composer_state.annotation_authoring.points[0];
                        if d.x.abs() + d.y.abs() < 4. / f64::from(zoom) {
                            continue;
                        }
                    }
                    app.composer_state.annotation_authoring.points.push(p);
                    let required = [3, 4, 2, 2, 1, 2][f];
                    if app.composer_state.annotation_authoring.points.len() == required && f != 4 {
                        let pts = app
                            .composer_state
                            .annotation_authoring
                            .points
                            .iter()
                            .map(|p| p.to_array())
                            .collect::<Vec<_>>();
                        match app.place_composer_annotation(&pts, modifiers.ctrl) {
                            Ok(_) => {
                                if matches!(f, 3 | 5) {
                                    app.composer_state.annotation_authoring.ignore_release = true;
                                }
                            }
                            Err(e) => app.status_message = e,
                        }
                        app.composer_state.annotation_authoring.points.clear();
                    }
                } else if app.composer_state.picture_grab.is_none()
                    && app.composer_state.dragging_item_idx.is_none()
                    && app.composer_state.legend_authoring.grab.is_none()
                {
                    for &(kind, index, f) in order.iter().rev() {
                        if kind != 3 {
                            let hit = if kind == 0 {
                                app.scene.sheets[s].items.get(index).is_some_and(|i| {
                                    let r = i.rect();
                                    p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h
                                })
                            } else if kind == 1 {
                                super::picture_gestures::rect(&app.scene.sheets[s], index)
                                    .is_ok_and(|r| {
                                        p.x >= r.x
                                            && p.x <= r.x + r.w
                                            && p.y >= r.y
                                            && p.y <= r.y + r.h
                                    })
                            } else {
                                super::legend_authoring::rect(&app.scene.sheets[s], index)
                                    .is_ok_and(|r| {
                                        p.x >= r.x
                                            && p.x <= r.x + r.w
                                            && p.y >= r.y
                                            && p.y <= r.y + r.h
                                    })
                            };
                            if hit {
                                break;
                            }
                            continue;
                        }
                        let owner = Owner {
                            sheet: app.scene.sheets[s].id,
                            family: f,
                            index,
                        };
                        let Some(v) = record(app, owner) else {
                            continue;
                        };
                        if v.get("locked").and_then(Value::as_bool) == Some(true)
                            || !hit(app, owner, p)
                        {
                            continue;
                        }
                        let before = v.clone();
                        let local = p - DVec2::new(
                            number(&before, "x_mm", 0.),
                            number(&before, "y_mm", 0.),
                        );
                        let handle = special(&before, f, local);
                        if let Err(e) = app.composer_select_annotation(f, Some(index)) {
                            app.status_message = e;
                            break;
                        }
                        owned = true;
                        if !handle {
                            match super::pointer_transactions::Targets::capture_raw(
                                &app.scene.sheets[s],
                                FAMILIES[f],
                                index,
                            ) {
                                Ok(targets) => {
                                    app.composer_state.annotation_authoring.grab = Some(Grab {
                                        owner,
                                        press: p,
                                        size: size(&before, f),
                                        before,
                                        targets,
                                    })
                                }
                                Err(e) => app.status_message = e,
                            }
                        } else {
                            app.status_message =
                                "This annotation handle is not yet editable".into();
                        }
                        break;
                    }
                    if !owned {
                        app.composer_state.annotation_authoring.selected = None;
                        app.composer_state.annotation_authoring.draft = None;
                    }
                }
            }
            egui::Event::PointerMoved(pos) => {
                app.composer_state.annotation_authoring.hover = Some(page(pos));
                if app.composer_state.annotation_authoring.grab.is_some() {
                    owned = true;
                    if let Err(e) = motion(app, page(pos)) {
                        app.status_message = e;
                    }
                }
            }
            egui::Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if std::mem::take(&mut app.composer_state.annotation_authoring.ignore_release) {
                    owned = true;
                    continue;
                }
                if app.composer_state.annotation_authoring.armed == Some(4)
                    && app.composer_state.annotation_authoring.points.len() == 1
                {
                    let p = app.composer_state.annotation_authoring.points[0];
                    if let Err(e) = app.place_composer_annotation(&[p.to_array()], false) {
                        app.status_message = e;
                    }
                    owned = true;
                }
                if app.composer_state.annotation_authoring.grab.is_some() {
                    owned = true;
                    app.finish_composer_edit();
                }
            }
            egui::Event::PointerGone => {
                app.composer_state.annotation_authoring.ignore_release = false;
                if app.composer_state.annotation_authoring.grab.is_some() {
                    app.finish_composer_edit();
                    owned = true;
                }
            }
            _ => {}
        }
    }
    owned
}

pub(super) fn pointer_order(app: &IngeTrazoApp) -> std::sync::Arc<Vec<(u8, usize)>> {
    app.composer_state
        .annotation_authoring
        .layouts
        .pointer_order
        .clone()
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnnotationState")
            .field("armed", &self.armed)
            .field("selected", &self.selected)
            .field("layout_builds", &self.layouts.builds)
            .finish()
    }
}

fn raw_bytes(v: &Value) -> Result<usize, String> {
    fn count(v: &Value, depth: usize, total: &mut usize) -> Result<(), String> {
        if depth > 16 {
            return Err("Annotation nesting budget exceeded".into());
        }
        let bytes = match v {
            Value::String(s) => s.capacity(),
            Value::Array(a) => a.capacity() * std::mem::size_of::<Value>(),
            Value::Object(o) => {
                o.len() * (std::mem::size_of::<Value>() + std::mem::size_of::<String>() + 64)
            }
            _ => std::mem::size_of::<Value>(),
        };
        *total = total
            .checked_add(bytes)
            .ok_or("Annotation raw budget overflow")?;
        if *total > 4 * 1024 * 1024 {
            return Err("Annotation raw budget exceeded".into());
        }
        match v {
            Value::Array(a) => {
                for v in a {
                    count(v, depth + 1, total)?
                }
            }
            Value::Object(o) => {
                for (k, v) in o {
                    *total = total
                        .checked_add(k.capacity())
                        .ok_or("Annotation key budget overflow")?;
                    count(v, depth + 1, total)?
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut bytes = 0;
    count(v, 0, &mut bytes)?;
    Ok(bytes)
}

const KNOWN_FIELDS: &[&str] = &[
    "a_world",
    "anchor_uid",
    "angle_deg",
    "arrow",
    "ax_mm",
    "axis",
    "ay_mm",
    "b_world",
    "bg_color",
    "bg_opacity",
    "bold",
    "bx_mm",
    "by_mm",
    "centre_mark",
    "color",
    "datum_m",
    "decimals",
    "dot",
    "dx_mm",
    "dy_mm",
    "ends",
    "follow",
    "frame_uid",
    "group_id",
    "h_mm",
    "hidden",
    "italic",
    "kind",
    "leaders",
    "line_mm",
    "list_name",
    "locked",
    "mirror",
    "number",
    "offset_mm",
    "placement",
    "radius_mm",
    "scale_n",
    "sep_mm",
    "shape",
    "sheet",
    "size_mm",
    "size_pt",
    "stroke_mm",
    "symbol",
    "text",
    "text_align",
    "text_along",
    "text_bg",
    "text_bg_opacity",
    "text_color",
    "text_dx_mm",
    "text_dy_mm",
    "text_mm",
    "text_pos",
    "uid",
    "underline",
    "units",
    "w_mm",
    "x_mm",
    "y_mm",
    "z",
    "z_m",
];

// Qt QDoubleSpinBox stores rounded widget values during blocked selection setup;
// its next real property callback publishes those values into the model.
fn normalize_property_draft(f: usize, v: &mut Value) {
    for (k, _) in property_fields(f) {
        if let Some(n) = v.get(*k).and_then(Value::as_f64) {
            // Pinned radial callbacks throw before editing. The independently declared
            // corrected text workflow retains radius/angle; numeric edits remain explicit.
            if f == 2 {
                continue;
            }
            let decimals = if *k == "decimals" {
                0
            } else if matches!(*k, "z_m" | "datum_m") {
                3
            } else {
                2
            };
            let n = format!("{n:.decimals$}").parse::<f64>().unwrap_or(n);
            let (lo, hi) = match *k {
                "sep_mm" => (-100., 100.),
                "radius_mm" => (2., 300.),
                "text_mm" => (1., 10.),
                "size_mm" => {
                    if f == 4 {
                        (1., 12.)
                    } else {
                        (1.5, 10.)
                    }
                }
                "size_pt" => (4., 72.),
                "stroke_mm" => (0.1, 1.5),
                "decimals" => (0., 4.),
                "z_m" | "datum_m" => (-100000., 100000.),
                "line_mm" => (2., 200.),
                _ => (-1000000., 1000000.),
            };
            v[*k] = if *k == "decimals" {
                json!(n.clamp(lo, hi) as i64)
            } else {
                json!(n.clamp(lo, hi))
            };
        }
    }
    if f == 0 {
        v["text_pos"] = json!("above");
    }
}
