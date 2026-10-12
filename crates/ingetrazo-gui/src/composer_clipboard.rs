//! Sheet clipboard owns only copied item records. Paste prepares a complete
//! cohort before one publication, and advances its offsets only after success.
use super::selection::{self, Record};
use crate::IngeTrazoApp;
use ingetrazo_core::composition::{Sheet, SheetItem};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const MAX_ITEMS: usize = 10_000;
const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    records: Vec<Record>,
    source: Option<Uuid>,
    bytes: usize,
}

fn value(record: &Record) -> &Value {
    match record {
        Record::Typed { native, .. } => native,
        Record::Raw { value, .. } => value,
    }
}
fn value_mut(record: &mut Record) -> &mut Value {
    match record {
        Record::Typed { native, .. } => native,
        Record::Raw { value, .. } => value,
    }
}
fn category(record: &Record) -> u8 {
    match record {
        Record::Raw { category, .. } => *category,
        Record::Typed { item, .. } => match item {
            SheetItem::ViewFrame(_) => 0,
            SheetItem::Text(_) => 1,
            SheetItem::GraphicScale(_) => 3,
            SheetItem::NorthArrow(_) => 4,
            SheetItem::Shape(_) => 6,
            SheetItem::TitleBlock(_) => 14,
        },
    }
}
fn number(value: &Value, key: &str, default: f64) -> Result<f64, String> {
    value
        .get(key)
        .map_or(Some(default), Value::as_f64)
        .filter(|x| x.is_finite() && x.abs() <= 1e6)
        .ok_or_else(|| format!("Invalid sheet clipboard {key}"))
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .map_or(Some(""), Value::as_str)
        .ok_or_else(|| format!("Invalid sheet clipboard {key}"))
}
fn account(value: &Value, depth: usize, bytes: &mut usize) -> Result<(), String> {
    if depth > 64 {
        return Err("Sheet clipboard record nesting exceeded".into());
    }
    let base = std::mem::size_of::<Value>();
    *bytes = bytes
        .checked_add(base)
        .ok_or("Sheet clipboard size overflow")?;
    match value {
        Value::String(s) => {
            *bytes = bytes
                .checked_add(s.len())
                .ok_or("Sheet clipboard size overflow")?
        }
        Value::Array(a) => {
            for child in a {
                account(child, depth + 1, bytes)?;
            }
        }
        Value::Object(o) => {
            for (key, child) in o {
                *bytes = bytes
                    .checked_add(key.len() + 4 * std::mem::size_of::<usize>())
                    .ok_or("Sheet clipboard size overflow")?;
                account(child, depth + 1, bytes)?;
            }
        }
        _ => {}
    }
    if *bytes > MAX_BYTES {
        return Err("Sheet clipboard memory budget exceeded".into());
    }
    Ok(())
}
fn admission(records: &[Record]) -> Result<usize, String> {
    if records.len() > MAX_ITEMS {
        return Err("Sheet clipboard item budget exceeded".into());
    }
    let mut bytes = records.len() * std::mem::size_of::<Record>();
    for record in records {
        let row = value(record);
        if !row.is_object() || category(record) > 14 {
            return Err("Invalid sheet clipboard record".into());
        }
        account(row, 0, &mut bytes)?;
        if let Record::Typed { item, .. } = record {
            account(
                &ingetrazo_formats::igz::paper_item_baseline(item),
                0,
                &mut bytes,
            )?;
        }
        number(row, "x_mm", 0.)?;
        number(row, "y_mm", 0.)?;
        number(row, "z", 0.)?;
    }
    Ok(bytes)
}
fn change_position(record: &mut Record, x: f64, y: f64) {
    if let Record::Typed { item, .. } = record {
        item.set_pos(x, y);
    }
    let row = value_mut(record);
    row["x_mm"] = json!(x);
    row["y_mm"] = json!(y);
}
fn typed_identity(item: &mut SheetItem, id: Uuid) {
    match item {
        SheetItem::ViewFrame(v) => v.id = id,
        SheetItem::Text(v) => v.id = id,
        SheetItem::TitleBlock(v) => v.id = id,
        SheetItem::NorthArrow(v) => v.id = id,
        SheetItem::GraphicScale(v) => v.id = id,
        SheetItem::Shape(v) => v.id = id,
    }
}
fn next_z(sheet: &Sheet) -> Result<f64, String> {
    let mut max: Option<f64> = None;
    let mut admit = |z: f64| -> Result<(), String> {
        if !z.is_finite() || z.abs() > 1e6 {
            return Err("Invalid sheet stacking order".into());
        }
        max = Some(max.map_or(z, |old| old.max(z)));
        Ok(())
    };
    for item in &sheet.items {
        let z = match item {
            SheetItem::Shape(s) => s.shape.z,
            _ => sheet
                .native_items
                .get(&item.id())
                .map_or(Ok(0.), |v| number(v, "z", 0.))?,
        };
        admit(z)?;
    }
    for key in [
        "images",
        "leyendas",
        "cotas",
        "cotas_ang",
        "cotas_rad",
        "etiquetas",
        "perfiles",
        "niveles",
        "llamadas",
    ] {
        if let Some(rows) = sheet.native_document.get(key).filter(|v| !v.is_null()) {
            for row in rows.as_array().ok_or("Invalid sheet inventory")? {
                admit(number(row, "z", 0.)?)?;
            }
        }
    }
    Ok(max.map_or(0., |z| z + 1.))
}
fn prepare(state: &State, sheet: &Sheet) -> Result<Vec<Record>, String> {
    admission(&state.records)?;
    let step = if state.source == Some(sheet.id) {
        5.
    } else {
        0.
    };
    let mut here = HashSet::new();
    for item in &sheet.items {
        if let SheetItem::ViewFrame(frame) = item {
            // Imported literal UIDs remain authoritative over the runtime UUID.
            let uid = sheet
                .native_items
                .get(&frame.id)
                .and_then(|r| r.get("uid"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| frame.id.simple().to_string());
            if !uid.is_empty() {
                here.insert(uid);
            }
        }
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(state.records.len())
        .map_err(|_| "Sheet clipboard allocation refused")?;
    let mut frames = HashMap::new();
    let mut groups = HashMap::new();
    let mut z = next_z(sheet)?;
    for source in &state.records {
        let mut record = source.clone();
        let kind = category(&record);
        let x = number(value(source), "x_mm", 0.)? + if kind == 14 { 0. } else { step };
        let y = number(value(source), "y_mm", 0.)? + if kind == 14 { 0. } else { step };
        if !x.is_finite() || !y.is_finite() || x.abs().max(y.abs()) > 1e6 {
            return Err("Sheet paste position exceeds domain".into());
        }
        change_position(&mut record, x, y);
        let id = Uuid::new_v4();
        if let Record::Typed { item, .. } = &mut record {
            typed_identity(item, id);
        }
        let row = value_mut(&mut record);
        row["locked"] = json!(false);
        let group = string(row, "group_id")?.to_owned();
        if !group.is_empty() {
            let new = groups
                .entry(group)
                .or_insert_with(|| Uuid::new_v4().simple().to_string());
            row["group_id"] = json!(new);
        }
        if kind == 0 {
            let old = string(row, "uid")?.to_owned();
            let new = id.simple().to_string();
            if !old.is_empty() {
                frames.insert(old, new.clone());
            }
            row["uid"] = json!(new);
        } else if matches!(kind, 8 | 9 | 10 | 12 | 13) {
            row["uid"] = json!("");
        }
        if !z.is_finite() || z > 1e6 {
            return Err("Sheet paste stacking order exceeds domain".into());
        }
        row["z"] = json!(z);
        let group = string(row, "group_id")?.to_owned();
        if let Record::Typed {
            item: SheetItem::Shape(shape),
            ..
        } = &mut record
        {
            shape.shape.z = z;
            shape.shape.locked = false;
            shape.shape.group_id = group;
        }
        result.push(record);
        z += 1.;
    }
    for record in &mut result {
        let kind = category(record);
        let row = value_mut(record);
        if matches!(kind, 1 | 13) {
            let reference = string(row, "frame_uid")?.to_owned();
            if let Some(new) = frames.get(&reference) {
                row["frame_uid"] = json!(new);
            } else if !reference.is_empty() && !here.contains(&reference) {
                row["frame_uid"] = json!("");
            }
        }
        if matches!(kind, 7 | 10 | 12) {
            let anchor = string(row, "anchor_uid")?.to_owned();
            if !anchor.is_empty() {
                let new = frames.get(&anchor).cloned().unwrap_or_default();
                if new.is_empty() {
                    row["a_world"] = Value::Null;
                    if kind == 7 {
                        row["b_world"] = Value::Null;
                    }
                }
                row["anchor_uid"] = json!(new);
            }
        }
    }
    admission(&result)?;
    Ok(result)
}

impl IngeTrazoApp {
    /// Returns plain text for the platform clipboard when the copied cohort
    /// contains Text/Label content. Empty selection preserves both clipboards.
    pub fn copy_composer_items(&mut self) -> Result<(usize, Option<String>), String> {
        let owners = selection::selected_owners(self);
        if owners.is_empty() {
            return Ok((0, None));
        }
        if owners.len() > MAX_ITEMS {
            return Err("Sheet clipboard item budget exceeded".into());
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(owners.len())
            .map_err(|_| "Sheet clipboard allocation refused")?;
        let mut text = Vec::new();
        let mut bytes = 0usize;
        for owner in owners {
            let record = selection::snapshot(self, owner)?;
            bytes = bytes
                .checked_add(admission(std::slice::from_ref(&record))?)
                .ok_or("Sheet clipboard size overflow")?;
            if bytes > MAX_BYTES {
                return Err("Sheet clipboard memory budget exceeded".into());
            }
            if matches!(category(&record), 1 | 10) {
                let content = string(value(&record), "text")?;
                if !content.is_empty() {
                    text.push(content.to_owned());
                }
            }
            records.push(record);
        }
        let index = self.scene.active_sheet_index.ok_or("No active sheet")?;
        let source = self.scene.sheets.get(index).ok_or("No active sheet")?.id;
        let count = records.len();
        self.sheet_clipboard = State {
            records,
            source: Some(source),
            bytes,
        };
        self.status_message = format!("{count} item(s) copied.");
        Ok((count, (!text.is_empty()).then(|| text.join("\n"))))
    }
    pub fn cut_composer_items(&mut self) -> Result<(usize, Option<String>), String> {
        let copied = self.copy_composer_items()?;
        if copied.0 != 0 {
            selection::delete_selected(self)?;
        }
        Ok(copied)
    }
    pub fn paste_composer_items(&mut self) -> Result<usize, String> {
        if self.sheet_clipboard.records.is_empty() {
            return Ok(0);
        }
        let index = self.scene.active_sheet_index.ok_or("No active sheet")?;
        let sheet = self.scene.sheets.get(index).ok_or("No active sheet")?;
        let source = sheet.id;
        let prepared = prepare(&self.sheet_clipboard, sheet)?;
        // Only position advances on the clipboard originals. New UIDs, z,
        // groups and anchor changes belong exclusively to each pasted cohort.
        let positions = prepared
            .iter()
            .map(|r| {
                let v = value(r);
                Ok((number(v, "x_mm", 0.)?, number(v, "y_mm", 0.)?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let count = prepared.len();
        selection::publish(self, prepared)?;
        for (record, (x, y)) in self.sheet_clipboard.records.iter_mut().zip(positions) {
            if category(record) != 14 {
                change_position(record, x, y);
            }
        }
        self.sheet_clipboard.source = Some(source);
        self.status_message = format!("{count} item(s) pasted.");
        Ok(count)
    }
    /// Observes the ordered clipboard without modifying application state.
    pub fn composer_clipboard_observation(&self) -> Value {
        json!({"source_sheet": self.sheet_clipboard.source,
            "bytes": self.sheet_clipboard.bytes,
            "records": self.sheet_clipboard.records.iter().map(|r|
                json!({"category":category(r),"record":value(r)})).collect::<Vec<_>>()})
    }
}

pub(crate) fn route(app: &mut IngeTrazoApp, ctx: &egui::Context, event: &egui::Event) {
    let result = match event {
        egui::Event::Copy => app.copy_composer_items(),
        egui::Event::Cut => app.cut_composer_items(),
        egui::Event::Paste(_) => app.paste_composer_items().map(|n| (n, None)),
        _ => return,
    };
    match result {
        Ok((_, Some(text))) => ctx.copy_text(text),
        Ok(_) => {}
        Err(error) => app.status_message = error,
    }
}
