//! One heterogeneous sheet cohort. Owners are IDs, never mutable row aliases.
use super::IngeTrazoApp;
use glam::DVec2;
use ingetrazo_core::composition::{RectMm, Sheet, SheetItem};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;
pub(crate) const KINDS: [&str; 15] = [
    "frames",
    "texts",
    "images",
    "scalebars",
    "nortes",
    "leyendas",
    "shapes",
    "cotas",
    "cotas_ang",
    "cotas_rad",
    "etiquetas",
    "perfiles",
    "niveles",
    "llamadas",
    "cajetin",
];
pub(crate) const LIMIT: usize = 10000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Owner {
    pub sheet: Uuid,
    pub category: u8,
    pub id: Uuid,
}
#[derive(Clone, Debug)]
pub(crate) enum Record {
    Typed { item: SheetItem, native: Value },
    Raw { category: u8, value: Value },
}
#[derive(Clone, Debug, Default)]
pub(super) struct State {
    pub members: Vec<Owner>,
    pub primary: Option<Owner>,
    membership: HashSet<Owner>,
    display: HashMap<Owner, RectMm>,
    pub stamp: Option<(Uuid, u64)>,
    pub cohort_mode: bool,
    band: Option<Band>,
    ctrl: bool,
    grab: Option<Group>,
    pub builds: u64,
    pub updates: u64,
    text_draft: Option<TextDraft>,
}
#[derive(Clone, Debug)]
struct TextDraft {
    owner: Owner,
    version: u64,
    text: String,
    size: f64,
    bold: bool,
    italic: bool,
    underline: bool,
}
#[derive(Clone, Debug)]
struct Band {
    start: DVec2,
    current: DVec2,
    zoom: f64,
}
#[derive(Clone, Debug)]
struct Peer {
    owner: Owner,
    index: usize,
    at: DVec2,
    current: DVec2,
    size: [f64; 2],
    before: Value,
}
#[derive(Clone, Debug)]
struct Group {
    sheet: Uuid,
    document: Uuid,
    revision: u64,
    press: DVec2,
    pressed: Owner,
    peers: Vec<Peer>,
    axes: [AxisIndex; 2],
    drawing: Vec<[f64; 2]>,
}
fn active(app: &IngeTrazoApp) -> Result<usize, String> {
    app.scene
        .active_sheet_index
        .filter(|&s| s < app.scene.sheets.len())
        .ok_or_else(|| "No active sheet".into())
}
pub(crate) fn category(item: &SheetItem) -> u8 {
    match item {
        SheetItem::ViewFrame(_) => 0,
        SheetItem::Text(_) => 1,
        SheetItem::GraphicScale(_) => 3,
        SheetItem::NorthArrow(_) => 4,
        SheetItem::Shape(_) => 6,
        SheetItem::TitleBlock(_) => 14,
    }
}
fn raw_slot(c: u8) -> Option<(bool, usize)> {
    match c {
        2 => Some((false, 0)),
        5 => Some((false, 1)),
        11 => Some((false, 2)),
        7 => Some((true, 0)),
        8 => Some((true, 1)),
        9 => Some((true, 2)),
        10 => Some((true, 3)),
        12 => Some((true, 4)),
        13 => Some((true, 5)),
        _ => None,
    }
}
pub(crate) fn ensure(sheet: &mut Sheet) -> Result<(), String> {
    sheet.ensure_annotation_ids()?;
    sheet.ensure_furniture_ids()
}
pub(crate) fn owner_at(sheet: &Sheet, c: u8, index: usize) -> Option<Owner> {
    let id = if let Some((a, f)) = raw_slot(c) {
        if a {
            sheet.annotation_id(f, index)?
        } else {
            sheet.furniture_id(f, index)?
        }
    } else {
        sheet.items.get(index).filter(|i| category(i) == c)?.id()
    };
    Some(Owner {
        sheet: sheet.id,
        category: c,
        id,
    })
}
pub(crate) fn locate(sheet: &Sheet, o: Owner) -> Option<usize> {
    if sheet.id != o.sheet {
        return None;
    }
    if let Some((a, f)) = raw_slot(o.category) {
        if a {
            sheet.annotation_index(f, o.id)
        } else {
            sheet.furniture_index(f, o.id)
        }
    } else {
        sheet
            .items
            .iter()
            .position(|i| i.id() == o.id && category(i) == o.category)
    }
}
fn raw(sheet: &Sheet, o: Owner) -> Option<&Value> {
    let i = locate(sheet, o)?;
    sheet
        .native_document
        .get(KINDS[o.category as usize])?
        .as_array()?
        .get(i)
}
pub(crate) fn selected_owners(app: &IngeTrazoApp) -> Vec<Owner> {
    app.composer_state.selection.members.clone()
}
pub(crate) fn primary(app: &IngeTrazoApp) -> Option<Owner> {
    app.composer_state.selection.primary
}
pub(crate) fn snapshot(app: &IngeTrazoApp, o: Owner) -> Result<Record, String> {
    let s = app
        .scene
        .sheets
        .iter()
        .find(|s| s.id == o.sheet)
        .ok_or("Selection sheet gone")?;
    let i = locate(s, o).ok_or("Selection owner gone")?;
    let _ = geometry(app, o)?;
    let mut count = 0;
    let mut bytes = 0;
    if let Some(v) = raw(s, o) {
        finite(v, 0, &mut count, &mut bytes)?;
    } else {
        if let Some(v) = s.native_items.get(&o.id) {
            finite(v, 0, &mut count, &mut bytes)?;
        }
        if let SheetItem::Text(t) = &s.items[i] {
            if t.text.capacity() > 32 * 1024 * 1024 {
                return Err("Text payload budget exceeded".into());
            }
        }
    }
    if raw_slot(o.category).is_some() {
        Ok(Record::Raw {
            category: o.category,
            value: raw(s, o).unwrap().clone(),
        })
    } else {
        let item = s.items[i].clone();
        let native = ingetrazo_formats::igz::paper_item_record(s, &item).1;
        Ok(Record::Typed { item, native })
    }
}
fn finite(v: &Value, depth: usize, count: &mut usize, bytes: &mut usize) -> Result<(), String> {
    if depth > 16 {
        return Err("Record nesting limit exceeded".into());
    }
    *count = count.checked_add(1).ok_or("Record count overflow")?;
    if *count > 100000 {
        return Err("Record node budget exceeded".into());
    }
    match v {
        Value::String(s) => {
            *bytes = bytes
                .checked_add(s.capacity())
                .ok_or("Record byte overflow")?;
        }
        Value::Array(a) => {
            *bytes = bytes
                .checked_add(a.capacity() * std::mem::size_of::<Value>())
                .ok_or("Record byte overflow")?;
            for v in a {
                finite(v, depth + 1, count, bytes)?;
            }
        }
        Value::Object(o) => {
            for (k, v) in o {
                *bytes = bytes
                    .checked_add(k.capacity() + 64)
                    .ok_or("Record byte overflow")?;
                finite(v, depth + 1, count, bytes)?;
            }
        }
        _ => {}
    }
    if *bytes > 32 * 1024 * 1024 {
        return Err("Record payload budget exceeded".into());
    }
    Ok(())
}
fn admit_record(r: &Record, count: &mut usize, bytes: &mut usize) -> Result<(), String> {
    let (c, v) = match r {
        Record::Typed { item, native } => (category(item), native),
        Record::Raw { category, value } => (*category, value),
    };
    if c > 14 || !v.is_object() {
        return Err("Invalid sheet record".into());
    }
    finite(v, 0, count, bytes)?;
    if matches!(c, 7 | 8 | 9 | 10 | 12 | 13) {
        super::annotation_authoring::admit(v)?;
    }
    if c == 5 {
        super::legend_authoring::admitted(v)?;
    }
    for k in [
        "locked",
        "hidden",
        "bold",
        "italic",
        "underline",
        "annotations",
        "section_marks",
        "perspective",
        "show_title",
        "title_scale",
        "profiles",
        "hidden_lines",
        "grid",
        "fill",
        "mirror",
        "dot",
        "centre_mark",
        "arrow",
        "border",
    ] {
        if v.get(k).is_some_and(|v| !v.is_boolean()) {
            return Err(format!("Invalid sheet Boolean field {k}"));
        }
    }
    for k in [
        "text",
        "path",
        "shape",
        "fit",
        "color",
        "text_color",
        "text_bg",
        "bg_color",
        "border_color",
        "text_pos",
        "text_along",
        "text_align",
        "units",
        "kind",
        "title",
        "number",
        "sheet_ref",
        "family",
        "view_key",
        "style",
        "group_id",
        "list_name",
    ] {
        if v.get(k).is_some_and(|v| !v.is_string()) {
            return Err(format!("Invalid sheet string field {k}"));
        }
    }
    if c == 2 {
        for k in ["opacity", "radius_mm", "feather_mm", "border_mm"] {
            if v.get(k)
                .is_some_and(|v| !v.as_f64().is_some_and(|n| n.is_finite() && n.abs() <= 1e6))
            {
                return Err(format!("Invalid picture field {k}"));
            }
        }
        if v.get("shape")
            .is_some_and(|v| !matches!(v.as_str(), Some("rect" | "rounded" | "ellipse")))
        {
            return Err("Unsupported picture shape".into());
        }
        if v.get("fit")
            .is_some_and(|v| !matches!(v.as_str(), Some("stretch" | "cover" | "contain")))
        {
            return Err("Unsupported picture fit".into());
        }
        if v.get("w_mm")
            .is_some_and(|v| v.as_f64().unwrap_or(0.) <= 0.)
            || v.get("h_mm")
                .is_some_and(|v| v.as_f64().unwrap_or(0.) <= 0.)
        {
            return Err("Picture dimensions must be positive".into());
        }
    }
    for k in [
        "x_mm",
        "y_mm",
        "w_mm",
        "h_mm",
        "size_mm",
        "radius_mm",
        "dx_mm",
        "dy_mm",
    ] {
        if let Some(v) = v.get(k) {
            if !v.as_f64().is_some_and(|n| n.is_finite() && n.abs() <= 1e6) {
                return Err(format!("Invalid sheet geometry {k}"));
            }
        }
    }
    if let Record::Typed { item, .. } = r {
        let r = item.rect();
        if [r.x, r.y, r.w, r.h]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        {
            return Err("Invalid typed sheet geometry".into());
        }
    }
    Ok(())
}
pub(crate) fn publish(app: &mut IngeTrazoApp, records: Vec<Record>) -> Result<Vec<Owner>, String> {
    let s = active(app)?;
    if records.is_empty() {
        return Ok(Vec::new());
    }
    if records.len() > LIMIT {
        return Err("Sheet cohort budget exceeded".into());
    }
    let mut count = 0;
    let mut bytes = 0;
    for r in &records {
        admit_record(r, &mut count, &mut bytes)?;
    }
    ensure(&mut app.scene.sheets[s])?;
    let old_count = app.scene.sheets[s].items.len()
        + [2u8, 5, 7, 8, 9, 10, 11, 12, 13]
            .iter()
            .map(|&c| {
                app.scene.sheets[s]
                    .native_document
                    .get(KINDS[c as usize])
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
            })
            .sum::<usize>();
    if old_count
        .checked_add(records.len())
        .is_none_or(|n| n > 100000)
    {
        return Err("Sheet inventory budget exceeded".into());
    }
    if records
        .iter()
        .filter(|r| {
            matches!(
                r,
                Record::Typed {
                    item: SheetItem::TitleBlock(_),
                    ..
                }
            )
        })
        .count()
        > 1
    {
        return Err("Duplicate singleton title publication".into());
    }
    let mut membership = HashSet::new();
    membership
        .try_reserve(records.len())
        .map_err(|_| "Publication membership allocation refused")?;
    let mut display = HashMap::new();
    display
        .try_reserve(records.len())
        .map_err(|_| "Publication display allocation refused")?;
    let mut owners = Vec::new();
    owners
        .try_reserve_exact(records.len())
        .map_err(|_| "Publication allocation refused")?;
    let mut typed = 0;
    let mut raw_counts = [0usize; 15];
    for r in &records {
        match r {
            Record::Typed { .. } => typed += 1,
            Record::Raw { category, .. } => raw_counts[*category as usize] += 1,
        }
    }
    app.scene.sheets[s]
        .items
        .try_reserve(typed)
        .map_err(|_| "Typed publication allocation refused")?;
    app.scene.sheets[s]
        .native_items
        .try_reserve(typed)
        .map_err(|_| "Metadata allocation refused")?;
    app.scene.sheets[s]
        .native_item_baselines
        .try_reserve(typed)
        .map_err(|_| "Metadata allocation refused")?;
    let mut fresh: HashMap<u8, Vec<Value>> = HashMap::new();
    for (c, n) in raw_counts.into_iter().enumerate() {
        if n == 0 {
            continue;
        }
        let key = KINDS[c];
        match app.scene.sheets[s].native_document.get_mut(key) {
            None | Some(Value::Null) => {
                let mut a = Vec::new();
                a.try_reserve_exact(n)
                    .map_err(|_| "Raw publication allocation refused")?;
                fresh.insert(c as u8, a);
            }
            Some(Value::Array(a)) => {
                if a.len() + n > 100000 {
                    return Err("Sheet inventory budget exceeded".into());
                }
                a.try_reserve(n)
                    .map_err(|_| "Raw publication allocation refused")?;
            }
            _ => return Err("Invalid raw publication inventory".into()),
        }
        let (a, f) = raw_slot(c as u8).ok_or("Invalid raw category")?;
        let ids = if a {
            &mut app.scene.sheets[s].annotation_ids[f]
        } else {
            &mut app.scene.sheets[s].furniture_ids[f]
        };
        ids.try_reserve(n)
            .map_err(|_| "Identity publication allocation refused")?;
    }
    let mut order = Vec::new();
    order
        .try_reserve_exact(records.len())
        .map_err(|_| "Publication selection allocation refused")?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(records.len())
        .map_err(|_| "Publication selection allocation refused")?;
    for (index, r) in records.iter().enumerate() {
        let (c, v) = match r {
            Record::Typed { item, native } => (category(item), native),
            Record::Raw { category, value } => (*category, value),
        };
        let z = v.get("z").map_or(Ok(0.), |v| {
            v.as_f64()
                .filter(|n| n.is_finite())
                .ok_or("Invalid sheet stacking order")
        })?;
        order.push((z, c, index));
    }
    order.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
    app.finish_composer_edit();
    app.begin_composer_edit();
    for r in records {
        match r {
            Record::Typed { item, native } => {
                let c = category(&item);
                let id = item.id();
                let sheet = &mut app.scene.sheets[s];
                if c == 14 {
                    if let Some(i) = sheet.items.iter().position(|i| category(i) == 14) {
                        let old = sheet.items.remove(i);
                        sheet.native_items.remove(&old.id());
                        sheet.native_item_baselines.remove(&old.id());
                    }
                }
                sheet.native_items.insert(id, native);
                sheet
                    .native_item_baselines
                    .insert(id, ingetrazo_formats::igz::paper_item_baseline(&item));
                sheet.items.push(item);
                owners.push(Owner {
                    sheet: sheet.id,
                    category: c,
                    id,
                });
            }
            Record::Raw { category: c, value } => {
                let sheet = &mut app.scene.sheets[s];
                if let Some(a) = fresh.remove(&c) {
                    sheet.native_document[KINDS[c as usize]] = Value::Array(a);
                }
                sheet.native_document[KINDS[c as usize]]
                    .as_array_mut()
                    .unwrap()
                    .push(value);
                let (a, f) = raw_slot(c).unwrap();
                let id = if a {
                    sheet.push_annotation_id(f)
                } else {
                    sheet.push_furniture_id(f)
                };
                owners.push(Owner {
                    sheet: sheet.id,
                    category: c,
                    id,
                });
            }
        }
    }
    for (_, _, index) in order {
        selected.push(owners[index]);
    }
    let shown = selected.first().copied();
    membership.extend(selected.iter().copied());
    app.composer_state.selection.members = selected;
    app.composer_state.selection.membership = membership;
    app.composer_state.selection.display = display;
    app.composer_state.selection.primary = shown;
    app.composer_state.selection.cohort_mode = true;
    app.composer_state.selection.stamp = None;
    mirror(app);
    app.finish_composer_edit();
    Ok(owners)
}
pub(crate) fn reselect(app: &mut IngeTrazoApp, owners: Vec<Owner>) -> Result<(), String> {
    set(app, owners, None)
}
fn flags(app: &IngeTrazoApp, o: Owner) -> Result<(bool, bool, String), String> {
    let s = &app.scene.sheets[active(app)?];
    let i = locate(s, o).ok_or("Selection owner gone")?;
    let v = if raw_slot(o.category).is_some() {
        raw(s, o).unwrap()
    } else {
        s.native_items.get(&o.id).unwrap_or(&Value::Null)
    };
    let locked = match s.items.get(i).filter(|_| raw_slot(o.category).is_none()) {
        Some(SheetItem::Shape(shape)) => shape.shape.locked,
        _ => v.get("locked").and_then(Value::as_bool).unwrap_or(false),
    };
    Ok((
        locked,
        v.get("hidden").and_then(Value::as_bool).unwrap_or(false),
        v.get("group_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
    ))
}

fn all(app: &mut IngeTrazoApp) -> Result<Vec<Owner>, String> {
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let sheet = &app.scene.sheets[s];
    let mut order = Vec::new();
    order
        .try_reserve(sheet.items.len())
        .map_err(|_| "Selection order allocation refused")?;
    for (i, item) in sheet.items.iter().enumerate() {
        let c = category(item);
        let v = sheet.native_items.get(&item.id());
        let z = match item {
            SheetItem::Shape(v) => v.shape.z,
            _ => v
                .and_then(|v| v.get("z"))
                .and_then(Value::as_f64)
                .unwrap_or(0.),
        };
        order.push((
            z,
            c,
            i,
            Owner {
                sheet: sheet.id,
                category: c,
                id: item.id(),
            },
        ));
    }
    for c in [2u8, 5, 7, 8, 9, 10, 11, 12, 13] {
        if let Some(rows) = sheet
            .native_document
            .get(KINDS[c as usize])
            .and_then(Value::as_array)
        {
            if order.len() + rows.len() > 100000 {
                return Err("Selection inventory budget exceeded".into());
            }
            order
                .try_reserve(rows.len())
                .map_err(|_| "Selection order allocation refused")?;
            for (i, v) in rows.iter().enumerate() {
                let z = v.get("z").and_then(Value::as_f64).unwrap_or(0.);
                if !z.is_finite() {
                    return Err("Invalid selection stacking order".into());
                }
                order.push((
                    z,
                    c,
                    i,
                    owner_at(sheet, c, i).ok_or("Selection identity missing")?,
                ));
            }
        }
    }
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    Ok(order.into_iter().rev().map(|v| v.3).collect())
}
fn set(
    app: &mut IngeTrazoApp,
    mut members: Vec<Owner>,
    wanted: Option<Owner>,
) -> Result<(), String> {
    if members.len() > LIMIT {
        return Err("Sheet cohort budget exceeded".into());
    }
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let mut seen = HashSet::new();
    members.retain(|o| locate(&app.scene.sheets[s], *o).is_some() && seen.insert(*o));
    let primary = wanted
        .filter(|o| members.contains(o))
        .or_else(|| {
            app.composer_state
                .selection
                .primary
                .filter(|o| members.contains(o))
        })
        .or_else(|| members.first().copied());
    let mut display = HashMap::new();
    display
        .try_reserve(members.len())
        .map_err(|_| "Selection display allocation refused")?;
    for &owner in &members {
        display.insert(owner, geometry(app, owner)?);
    }
    app.composer_state.selection.display = display;
    app.composer_state.selection.membership = members.iter().copied().collect();
    app.composer_state.selection.members = members;
    app.composer_state.selection.primary = primary;
    app.composer_state.selection.cohort_mode = true;
    mirror(app);
    Ok(())
}
fn mirror(app: &mut IngeTrazoApp) {
    let Ok(s) = active(app) else { return };
    let primary = app.composer_state.selection.primary;
    app.scene.sheets[s].selected_item = primary
        .filter(|o| raw_slot(o.category).is_none())
        .and_then(|o| locate(&app.scene.sheets[s], o));
    app.composer_state.picture_authoring.selected = primary
        .filter(|o| o.category == 2)
        .and_then(|o| locate(&app.scene.sheets[s], o).map(|i| (o.sheet, i)));
    app.composer_state.legend_authoring.selected = primary
        .filter(|o| o.category == 5)
        .and_then(|o| locate(&app.scene.sheets[s], o).map(|i| (o.sheet, i)));
    let annotations = app
        .composer_state
        .selection
        .members
        .iter()
        .filter_map(|o| {
            raw_slot(o.category).filter(|(a, _)| *a).and_then(|(_, f)| {
                locate(&app.scene.sheets[s], *o).map(|i| super::annotation_authoring::Owner {
                    sheet: o.sheet,
                    family: f,
                    index: i,
                })
            })
        })
        .collect();
    let shown = primary.and_then(|o| {
        raw_slot(o.category).filter(|(a, _)| *a).and_then(|(_, f)| {
            locate(&app.scene.sheets[s], o).map(|i| super::annotation_authoring::Owner {
                sheet: o.sheet,
                family: f,
                index: i,
            })
        })
    });
    app.set_annotation_cohort(annotations, shown);
}
pub(crate) fn revalidate(app: &mut IngeTrazoApp) {
    let Ok(s) = active(app) else {
        app.composer_state.selection = Default::default();
        return;
    };
    let stamp = (app.scene.sheets[s].id, app.scene.version);
    if app.composer_state.selection.stamp == Some(stamp) {
        return;
    }
    let mut members = std::mem::take(&mut app.composer_state.selection.members);
    members.retain(|o| locate(&app.scene.sheets[s], *o).is_some());
    let primary = app
        .composer_state
        .selection
        .primary
        .filter(|o| members.contains(o));
    app.composer_state.selection.members = members;
    app.composer_state.selection.membership = app
        .composer_state
        .selection
        .members
        .iter()
        .copied()
        .collect();
    app.composer_state.selection.primary =
        primary.or_else(|| app.composer_state.selection.members.first().copied());
    app.composer_state.selection.stamp = Some(stamp);
    let mut display = HashMap::new();
    for &owner in &app.composer_state.selection.members {
        if let Ok(r) = geometry(app, owner) {
            display.insert(owner, r);
        }
    }
    app.composer_state.selection.display = display;
    if app.composer_state.selection.cohort_mode {
        mirror(app);
    }
}
pub(crate) fn clear(app: &mut IngeTrazoApp) {
    app.composer_state.selection.members.clear();
    app.composer_state.selection.membership.clear();
    app.composer_state.selection.primary = None;
    app.composer_state.selection.display.clear();
    mirror(app);
}
pub(crate) fn is_selected(app: &IngeTrazoApp, o: Owner) -> bool {
    app.composer_state.selection.membership.contains(&o)
}
pub(crate) fn force_select(
    app: &mut IngeTrazoApp,
    c: u8,
    index: usize,
    extend: bool,
) -> Result<(), String> {
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let o = owner_at(&app.scene.sheets[s], c, index).ok_or("Sheet item missing")?;
    if flags(app, o)?.1 {
        if !extend {
            clear(app);
        }
        return Ok(());
    }
    let members = if extend {
        let mut m = selected_owners(app);
        if !m.contains(&o) {
            m.push(o);
        }
        m
    } else {
        vec![o]
    };
    set(app, members, None)
}
fn n(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(d)
}
fn annotation_owner(
    app: &IngeTrazoApp,
    o: Owner,
) -> Result<super::annotation_authoring::Owner, String> {
    let s = active(app)?;
    let (_, f) = raw_slot(o.category)
        .filter(|(a, _)| *a)
        .ok_or("Not an annotation")?;
    Ok(super::annotation_authoring::Owner {
        sheet: o.sheet,
        family: f,
        index: locate(&app.scene.sheets[s], o).ok_or("Annotation missing")?,
    })
}
fn geometry(app: &IngeTrazoApp, o: Owner) -> Result<RectMm, String> {
    let s = &app.scene.sheets[active(app)?];
    let i = locate(s, o).ok_or("Selected item missing")?;
    let r = match o.category {
        2 => super::picture_gestures::rect(s, i)?,
        5 => super::legend_authoring::rect(s, i)?,
        11 => {
            let v = raw(s, o).unwrap();
            RectMm::new(
                n(v, "x_mm", 20.),
                n(v, "y_mm", 20.),
                n(v, "w_mm", 180.),
                n(v, "h_mm", 70.),
            )
        }
        7 | 8 | 9 | 10 | 12 | 13 => {
            let v = raw(s, o).unwrap();
            let size = app.sheet_annotation_size(annotation_owner(app, o)?)?;
            RectMm::new(n(v, "x_mm", 0.), n(v, "y_mm", 0.), size[0], size[1])
        }
        _ => {
            let item = &s.items[i];
            let mut r = item.rect();
            let v = s.native_items.get(&o.id).unwrap_or(&Value::Null);
            if let SheetItem::Text(t) = item {
                r.w = n(v, "w_mm", 80.);
                r.h = (t.font_size_pt
                    * (25.4 / 72.)
                    * 1.4
                    * (t.text.matches('\n').count() + 1) as f64)
                    .max(6.);
            }
            if let SheetItem::GraphicScale(t) = item {
                let divisions = n(v, "segments", t.divisions as f64);
                if divisions <= 0. || divisions > 100000. || t.scale_n <= 0. {
                    return Err("Invalid scale dimensions".into());
                }
                let target = (100. / divisions) * t.scale_n / 1000.;
                let mut seg = 0.1;
                for value in [
                    0.1, 0.2, 0.25, 0.5, 1., 2., 2.5, 5., 10., 20., 25., 50., 100., 200., 250.,
                    500., 1000., 2000., 5000.,
                ] {
                    if value <= target {
                        seg = value;
                    }
                }
                r.w = (seg * 1000. / t.scale_n) * divisions;
                r.h = 8.;
            }
            r
        }
    };
    if [r.x, r.y, r.w, r.h]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("Invalid selected dimensions".into());
    }
    Ok(r)
}
fn bounds(app: &IngeTrazoApp, o: Owner) -> Result<RectMm, String> {
    let r = geometry(app, o)?;
    if matches!(o.category, 7 | 8 | 9 | 10 | 12 | 13) {
        let [x, y, w, h] = app.sheet_annotation_bounds(annotation_owner(app, o)?)?;
        return Ok(RectMm::new(r.x + x, r.y + y, w, h));
    }
    let s = &app.scene.sheets[active(app)?];
    let v = if raw_slot(o.category).is_some() {
        raw(s, o).unwrap()
    } else {
        s.native_items.get(&o.id).unwrap_or(&Value::Null)
    };
    let mut b = if o.category == 3 {
        RectMm::new(r.x - 12.5, r.y - 0.5, r.w + 25., 12.)
    } else {
        RectMm::new(r.x - 0.5, r.y - 0.5, r.w + 3.5, r.h + 3.5)
    };
    if o.category == 1
        && v.get("bg_color")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty())
    {
        b.x -= 1.5;
        b.y -= 1.5;
        b.w += 3.;
        b.h += 3.;
    }
    Ok(b)
}
fn probe(app: &IngeTrazoApp, o: Owner, p: DVec2) -> Result<(bool, bool, bool), String> {
    if matches!(o.category, 7 | 8 | 9 | 10 | 12 | 13) {
        return app.sheet_annotation_probe(annotation_owner(app, o)?, p);
    }
    let b = bounds(app, o)?;
    let r = geometry(app, o)?;
    let hit = p.x >= b.x && p.x < b.x + b.w && p.y >= b.y && p.y < b.y + b.h;
    let handle =
        o.category != 3 && (p.x - (r.x + r.w)).abs() <= 3. && (p.y - (r.y + r.h)).abs() <= 3.;
    Ok((hit, handle, false))
}
fn expand_groups(
    app: &mut IngeTrazoApp,
    mut members: Vec<Owner>,
    selected: Owner,
) -> Result<Vec<Owner>, String> {
    let gid = flags(app, selected)?.2;
    if !gid.is_empty() {
        for o in all(app)? {
            let (_, hidden, g) = flags(app, o)?;
            if !hidden && g == gid && !members.contains(&o) {
                if members.len() >= LIMIT {
                    return Err("Sheet cohort budget exceeded".into());
                }
                members.push(o);
            }
        }
    }
    Ok(members)
}

#[derive(Clone, Copy, Debug)]
struct Coordinate(f64);
impl PartialEq for Coordinate {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}
impl Eq for Coordinate {}
impl PartialOrd for Coordinate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Coordinate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
#[derive(Clone, Debug)]
struct AxisIndex {
    points: BTreeMap<(Coordinate, usize), Option<Owner>>,
    ranks: Vec<usize>,
}
impl AxisIndex {
    fn new(fixed: Vec<f64>, peers: &[Peer], axis: usize, ranks: Vec<usize>) -> Self {
        let mut points = fixed
            .into_iter()
            .enumerate()
            .map(|(rank, p)| ((Coordinate(p), rank), None))
            .collect::<BTreeMap<_, _>>();
        for (p, &rank) in peers.iter().zip(&ranks) {
            let x = p.at[axis];
            points.insert((Coordinate(x), rank), Some(p.owner));
            points.insert((Coordinate(x + p.size[axis]), rank + 1), Some(p.owner));
        }
        Self { points, ranks }
    }
    fn moved(&mut self, index: usize, owner: Owner, old: f64, new: f64, size: f64) {
        if old.to_bits() == new.to_bits() {
            return;
        }
        for (offset, delta) in [(0, 0.), (1, size)] {
            let rank = self.ranks[index] + offset;
            self.points.remove(&(Coordinate(old + delta), rank));
            self.points
                .insert((Coordinate(new + delta), rank), Some(owner));
        }
    }
    fn snap(&self, owner: Owner, value: f64) -> f64 {
        let key = Coordinate(value);
        let left_coordinate = self
            .points
            .range(..=(key, usize::MAX))
            .rev()
            .find(|(_, o)| **o != Some(owner))
            .map(|(k, _)| k.0);
        let left = left_coordinate.and_then(|x| {
            self.points
                .range((x, 0)..=(x, usize::MAX))
                .find(|(_, o)| **o != Some(owner))
        });
        let right = self
            .points
            .range((key, 0)..)
            .find(|(_, o)| **o != Some(owner));
        let mut best: Option<(f64, f64, usize)> = None;
        for (key, _) in left.into_iter().chain(right) {
            let (x, rank) = *key;
            let d = (x.0 - value).abs();
            if d <= 2.
                && best.is_none_or(|(_, old, old_rank)| d < old || d == old && rank < old_rank)
            {
                best = Some((x.0, d, rank));
            }
        }
        best.map_or(value, |(x, _, _)| x)
    }
}

fn capture(app: &mut IngeTrazoApp, pressed: Owner, press: DVec2) -> Result<(), String> {
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let members = selected_owners(app);
    if members.len() > LIMIT {
        return Err("Sheet cohort budget exceeded".into());
    }
    let mut peers = Vec::new();
    peers
        .try_reserve_exact(members.len())
        .map_err(|_| "Group acquisition refused")?;
    let mut count = 0;
    let mut bytes = 0;
    for owner in members {
        if flags(app, owner)?.0 {
            continue;
        }
        let r = geometry(app, owner)?;
        let before = match snapshot(app, owner)? {
            Record::Typed { native, .. } | Record::Raw { value: native, .. } => native,
        };
        finite(&before, 0, &mut count, &mut bytes)?;
        peers.push(Peer {
            owner,
            index: locate(&app.scene.sheets[s], owner).ok_or("Group owner missing")?,
            at: DVec2::new(r.x, r.y),
            current: DVec2::new(r.x, r.y),
            size: [r.w, r.h],
            before,
        });
    }
    let peer_indexes = peers
        .iter()
        .enumerate()
        .map(|(i, p)| (p.owner, i))
        .collect::<HashMap<_, _>>();
    let mut ranks = vec![[usize::MAX; 2]; peers.len()];
    let (w, h) = app.scene.sheets[s].dimensions_mm();
    let margin = app.scene.sheets[s].margin_mm;
    let mut x = vec![0., margin, w / 2., w - margin, w];
    let mut y = vec![0., margin, h / 2., h - margin, h];
    for (axis, values) in [("v", &mut x), ("h", &mut y)] {
        if let Some(a) = app.scene.sheets[s]
            .native_document
            .get("guides")
            .and_then(|v| v.get(axis))
            .and_then(Value::as_array)
        {
            if a.len() > 100000 {
                return Err("Guide inventory budget exceeded".into());
            }
            values
                .try_reserve(a.len())
                .map_err(|_| "Guide inventory refused")?;
            for v in a {
                values.push(
                    v.as_f64()
                        .filter(|n| n.is_finite())
                        .ok_or("Invalid guide")?,
                );
            }
        }
    }
    let ordered = all(app)?;
    x.try_reserve(ordered.len() * 2)
        .map_err(|_| "Group targets refused")?;
    y.try_reserve(ordered.len() * 2)
        .map_err(|_| "Group targets refused")?;
    for owner in ordered {
        let r = geometry(app, owner)?;
        if let Some(&i) = peer_indexes.get(&owner) {
            ranks[i] = [x.len(), y.len()];
        }
        x.extend([r.x, r.x + r.w]);
        y.extend([r.y, r.y + r.h]);
    }
    if ranks.iter().any(|r| r.contains(&usize::MAX)) {
        return Err("Group target owner missing".into());
    }
    let axes = [
        AxisIndex::new(x, &peers, 0, ranks.iter().map(|r| r[0]).collect()),
        AxisIndex::new(y, &peers, 1, ranks.iter().map(|r| r[1]).collect()),
    ];
    let drawing = if peers.iter().any(|p| p.owner.category == 2) {
        super::picture_gestures::drawing_points(app, s)?
    } else {
        Vec::new()
    };
    app.composer_state.selection.grab = Some(Group {
        sheet: app.scene.sheets[s].id,
        document: app.clipboard_document,
        revision: app.model_revision,
        pressed,
        press,
        peers,
        axes,
        drawing,
    });
    app.composer_state.selection.builds = app.composer_state.selection.builds.wrapping_add(1);
    Ok(())
}
fn move_one(
    app: &mut IngeTrazoApp,
    peer: &Peer,
    next: DVec2,
    pressed: Owner,
) -> Result<(), String> {
    if next == peer.current {
        return Ok(());
    }
    if !next.is_finite() || next.abs().max_element() > 1e6 {
        return Err("Group motion domain refused".into());
    }
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    app.begin_composer_edit();
    if let Some(r) = app.composer_state.selection.display.get_mut(&peer.owner) {
        r.x = next.x;
        r.y = next.y;
    }
    let sheet = &mut app.scene.sheets[s];
    if raw_slot(peer.owner.category).is_none() {
        sheet.items[peer.index].set_pos(next.x, next.y);
    } else {
        let v = &mut sheet.native_document[KINDS[peer.owner.category as usize]][peer.index];
        v["x_mm"] = json!(next.x);
        v["y_mm"] = json!(next.y);
        let delta = next - peer.at;
        if peer.owner == pressed && matches!(peer.owner.category, 10 | 12) {
            v["ax_mm"] = json!(n(&peer.before, "ax_mm", 0.) - delta.x);
            v["ay_mm"] = json!(n(&peer.before, "ay_mm", 0.) - delta.y);
        }
        if peer.owner == pressed && peer.owner.category == 10 {
            if let (Some(old), Some(new)) = (
                peer.before.get("leaders").and_then(Value::as_array),
                v.get_mut("leaders").and_then(Value::as_array_mut),
            ) {
                for (a, b) in old.iter().zip(new) {
                    b["ax_mm"] = json!(n(a, "ax_mm", 0.) - delta.x);
                    b["ay_mm"] = json!(n(a, "ay_mm", 0.) - delta.y);
                }
            }
        }
    }
    if peer.owner == pressed && matches!(peer.owner.category, 10 | 12) {
        let (_, f) = raw_slot(peer.owner.category).unwrap();
        app.invalidate_sheet_annotation(f, peer.index);
    }
    Ok(())
}
fn motion(app: &mut IngeTrazoApp, point: DVec2) -> Result<(), String> {
    if !point.is_finite() || point.abs().max_element() > 1e6 {
        return Err("Invalid sheet pointer".into());
    }
    let Some(mut group) = app.composer_state.selection.grab.take() else {
        return Ok(());
    };
    let result = (|| {
        let s = active(app)?;
        if app.scene.sheets[s].id != group.sheet
            || app.clipboard_document != group.document
            || app.model_revision != group.revision
        {
            return Err("Group document owner changed".into());
        }
        let delta = point - group.press;
        let pressed = group
            .peers
            .iter()
            .position(|p| p.owner == group.pressed)
            .ok_or("Pressed group owner missing")?;
        let sequence =
            std::iter::once(pressed).chain((0..group.peers.len()).filter(|&i| i != pressed));
        for i in sequence {
            let p = &group.peers[i];
            let mut next = p.at + delta;
            next.x = group.axes[0].snap(p.owner, next.x);
            next.x = group.axes[0].snap(p.owner, next.x + p.size[0]) - p.size[0];
            next.y = group.axes[1].snap(p.owner, next.y);
            next.y = group.axes[1].snap(p.owner, next.y + p.size[1]) - p.size[1];
            if p.owner.category == 2 {
                let mut best = None;
                for [xx, yy] in [
                    [next.x, next.y],
                    [next.x + p.size[0], next.y],
                    [next.x, next.y + p.size[1]],
                    [next.x + p.size[0], next.y + p.size[1]],
                ] {
                    let mut nearest = None;
                    let mut distance = 4.;
                    for [px, py] in &group.drawing {
                        let d = (px - xx).powi(2) + (py - yy).powi(2);
                        if d < distance {
                            distance = d;
                            nearest = Some([*px, *py]);
                        }
                    }
                    if let Some([px, py]) = nearest {
                        if best.is_none_or(|(old, _, _)| distance < old) {
                            best = Some((distance, next.x + (px - xx), next.y + (py - yy)));
                        }
                    }
                }
                if let Some((_, x, y)) = best {
                    next = DVec2::new(x, y);
                }
            }
            move_one(app, p, next, group.pressed)?;
            for axis in 0..2 {
                group.axes[axis].moved(i, p.owner, p.current[axis], next[axis], p.size[axis]);
            }
            group.peers[i].current = next;
        }
        app.composer_state.selection.updates = app.composer_state.selection.updates.wrapping_add(1);
        Ok(())
    })();
    app.composer_state.selection.grab = Some(group);
    result
}

fn finish_grab(app: &mut IngeTrazoApp) -> Result<(), String> {
    let Some(group) = app.composer_state.selection.grab.take() else {
        return Ok(());
    };
    let s = active(app)?;
    if app.scene.sheets[s].id != group.sheet {
        return Err("Group sheet owner changed".into());
    }
    if let Some(p) = group.peers.iter().find(|p| p.owner == group.pressed) {
        let delta = p.current - p.at;
        if delta != DVec2::ZERO && p.owner.category == 7 {
            let v = &mut app.scene.sheets[s].native_document["cotas"][p.index];
            if v.get("anchor_uid")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
            {
                v["anchor_uid"] = json!("");
                v["a_world"] = Value::Null;
                v["b_world"] = Value::Null;
            }
        }
        if delta != DVec2::ZERO && p.owner.category == 0 {
            let uid = app.scene.sheets[s]
                .native_items
                .get(&p.owner.id)
                .and_then(|v| v.get("uid"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !uid.is_empty() {
                let sheet = &mut app.scene.sheets[s];
                let native = &sheet.native_items;
                for item in &mut sheet.items {
                    let SheetItem::Text(text) = item else {
                        continue;
                    };
                    if group.peers.iter().any(|p| p.owner.id == text.id) {
                        continue;
                    }
                    let v = native.get(&text.id);
                    if v.and_then(|v| v.get("frame_uid")).and_then(Value::as_str)
                        == Some(uid.as_str())
                        && v.and_then(|v| v.get("follow"))
                            .and_then(Value::as_bool)
                            .unwrap_or(true)
                    {
                        text.pos.0 += delta.x;
                        text.pos.1 += delta.y;
                    }
                }
            }
        }
    }
    let unmoved=group.peers.iter().all(|p|p.at==p.current);
    if unmoved {set(app,vec![group.pressed],Some(group.pressed))?;}
    app.finish_composer_edit();
    revalidate(app);
    Ok(())
}
pub(crate) fn delete_selected(app: &mut IngeTrazoApp) -> Result<bool, String> {
    let s = active(app)?;
    let owners = selected_owners(app);
    if owners.is_empty() {
        return Ok(false);
    }
    if owners.len() > LIMIT {
        return Err("Sheet cohort budget exceeded".into());
    }
    ensure(&mut app.scene.sheets[s])?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(owners.len())
        .map_err(|_| "Deletion admission refused")?;
    for o in &owners {
        entries.push((
            *o,
            locate(&app.scene.sheets[s], *o).ok_or("Selected owner changed")?,
        ));
    }
    app.finish_composer_edit();
    app.begin_composer_edit();
    let removed = owners.iter().map(|o| o.id).collect::<HashSet<_>>();
    if let Some(before) = app.composer_state.edit_before.as_mut() {
        let old = std::mem::take(&mut before.items);
        let mut removed_items = HashMap::new();
        for item in old {
            if removed.contains(&item.id()) {
                removed_items.insert(item.id(), item);
            } else {
                before.items.push(item);
            }
        }
        for o in owners.iter().rev() {
            if let Some(item) = removed_items.remove(&o.id) {
                before.items.push(item);
            }
        }
        for c in [2u8, 5, 7, 8, 9, 10, 11, 12, 13] {
            let (a, f) = raw_slot(c).unwrap();
            let Some(rows) = before
                .native_document
                .get_mut(KINDS[c as usize])
                .and_then(Value::as_array_mut)
            else {
                continue;
            };
            let ids = if a {
                &mut before.annotation_ids[f]
            } else {
                &mut before.furniture_ids[f]
            };
            let old_ids = std::mem::take(ids);
            let mut removed_rows = HashMap::new();
            for (id, value) in old_ids.into_iter().zip(std::mem::take(rows)) {
                if removed.contains(&id) {
                    removed_rows.insert(id, value);
                } else {
                    ids.push(id);
                    rows.push(value);
                }
            }
            for o in owners.iter().rev().filter(|o| o.category == c) {
                if let Some(v) = removed_rows.remove(&o.id) {
                    ids.push(o.id);
                    rows.push(v);
                }
            }
        }
    }
    entries.sort_by_key(|(o, i)| (o.category, *i));
    for (o, i) in entries.into_iter().rev() {
        let sheet = &mut app.scene.sheets[s];
        if let Some((a, f)) = raw_slot(o.category) {
            sheet.native_document[KINDS[o.category as usize]]
                .as_array_mut()
                .ok_or("Selected inventory changed")?
                .remove(i);
            if a {
                sheet.remove_annotation_id(f, i);
            } else {
                sheet.remove_furniture_id(f, i);
            }
        } else {
            let i = sheet
                .items
                .iter()
                .position(|v| v.id() == o.id)
                .ok_or("Typed owner changed")?;
            let item = sheet.items.remove(i);
            sheet.native_items.remove(&item.id());
            sheet.native_item_baselines.remove(&item.id());
            if o.category == 14 {
                sheet.native_document["cajetin"] = Value::Null;
            }
        }
    }
    clear(app);
    app.finish_composer_edit();
    Ok(true)
}
fn band_release(
    app: &mut IngeTrazoApp,
    band: Band,
    p: DVec2,
    mods: egui::Modifiers,
) -> Result<(), String> {
    if (p - band.start).abs().element_sum() * band.zoom < 4. {
        if !mods.ctrl && !mods.shift {
            clear(app);
        }
        return Ok(());
    }
    let lo = p.min(band.start);
    let hi = p.max(band.start);
    let crossing = p.x < band.start.x;
    let mut members = if !mods.ctrl && !mods.shift {
        Vec::new()
    } else {
        selected_owners(app)
    };
    for owner in all(app)? {
        let (locked, hidden, _) = flags(app, owner)?;
        if locked || hidden {
            continue;
        }
        let b = bounds(app, owner)?;
        let a = DVec2::new(b.x, b.y);
        let z = a + DVec2::new(b.w, b.h);
        let found = if crossing {
            a.x < hi.x && a.y < hi.y && z.x > lo.x && z.y > lo.y
        } else {
            a.x >= lo.x && a.y >= lo.y && z.x <= hi.x && z.y <= hi.y
        };
        if !found {
            continue;
        }
        if mods.shift {
            if let Some(i) = members.iter().position(|&v| v == owner) {
                members.remove(i);
            } else if !mods.ctrl {
                members = expand_groups(
                    app,
                    {
                        let mut m = members;
                        m.push(owner);
                        m
                    },
                    owner,
                )?;
            }
        } else if !members.contains(&owner) {
            members.push(owner);
            members = expand_groups(app, members, owner)?;
        }
        if members.len() > LIMIT {
            return Err("Sheet cohort budget exceeded".into());
        }
    }
    set(app, members, None)
}
pub(super) fn input(
    app: &mut IngeTrazoApp,
    ui: &egui::Ui,
    s: usize,
    response: &egui::Response,
    paper: egui::Rect,
    toolbar: egui::Rect,
    zoom: f32,
    space: bool,
) -> bool {
    if !ui.is_enabled()
        || space
        || !zoom.is_finite()
        || zoom <= 0.
        || ui.ctx().output(|o| o.num_completed_passes) != 0
    {
        return false;
    }
    let enabled = app.composer_state.annotation_authoring.armed.is_none()
        && !app.composer_state.text_tool
        && app.composer_state.shape_tool.is_none()
        && !app.composer_state.legend_authoring.armed
        && app.composer_state.inline_text.is_none();
    let events = ui.input(|i| i.events.clone());
    let page = |p: egui::Pos2| {
        DVec2::new(
            f64::from((p.x - paper.min.x) / zoom),
            f64::from((p.y - paper.min.y) / zoom),
        )
    };
    let mut owned = app.composer_state.selection.ctrl
        || app.composer_state.selection.band.is_some()
        || app.composer_state.selection.grab.is_some();
    for event in events {
        if app.composer_state.selection.ctrl {
            match event {
                egui::Event::PointerMoved(_) => {
                    owned = true;
                    continue;
                }
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                }
                | egui::Event::PointerGone => {
                    app.composer_state.selection.ctrl = false;
                    owned = true;
                    continue;
                }
                _ => {}
            }
        }
        match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            } if enabled
                && paper.contains(pos)
                && !toolbar.contains(pos)
                && ui.ctx().layer_id_at(pos) == Some(response.layer_id) =>
            {
                let p = page(pos);
                let result = (|| {
                    adopt(app)?;
                    let ordered = all(app)?;
                    let mut hits = Vec::new();
                    for o in ordered {
                        let (locked, hidden, _) = flags(app, o)?;
                        if locked || hidden {
                            continue;
                        }
                        let (hit, handle, ctrl_tip) = probe(app, o, p)?;
                        if hit {
                            hits.push((o, handle, ctrl_tip));
                        }
                    }
                    if modifiers.alt && hits.len() >= 2 {
                        let old = hits
                            .iter()
                            .position(|(o, _, _)| is_selected(app, *o))
                            .unwrap_or(hits.len() - 1);
                        let target=hits[(old+1)%hits.len()].0;
                        let members=expand_groups(app,vec![target],target)?;
                        set(app,members,Some(target))?;
                        app.composer_state.selection.ctrl = true;
                        return Ok(true);
                    }
                    if let Some((o, handle, ctrl_tip)) = hits.first().copied() {
                        let mut members = selected_owners(app);
                        if modifiers.ctrl && !ctrl_tip {
                            if let Some(i) = members.iter().position(|&v| v == o) {
                                members.remove(i);
                            } else {
                                if members.len() >= LIMIT {
                                    return Err("Sheet cohort budget exceeded".into());
                                }
                                members.push(o);
                                members = expand_groups(app, members, o)?;
                            }
                            set(app, members, None)?;
                            app.composer_state.selection.ctrl = true;
                            return Ok(true);
                        }
                        if !members.contains(&o) {
                            if !handle {
                                members.clear();
                            }
                            if members.len() >= LIMIT {
                                return Err("Sheet cohort budget exceeded".into());
                            }
                            members.push(o);
                            members = expand_groups(app, members, o)?;
                        }
                        set(app, members, None)?;
                        if handle {
                            return Ok(false);
                        }
                        capture(app, o, p)?;
                        return Ok(true);
                    }
                    app.composer_state.selection.band = Some(Band {
                        start: p,
                        current: p,
                        zoom: f64::from(zoom),
                    });
                    Ok(true)
                })();
                match result {
                    Ok(v) => owned |= v,
                    Err(e) => {
                        app.status_message = e;
                        owned = true;
                    }
                }
            }
            egui::Event::PointerMoved(pos) => {
                let p = page(pos);
                if let Some(b) = app.composer_state.selection.band.as_mut() {
                    b.current = p;
                    owned = true;
                }
                if app.composer_state.selection.grab.is_some() {
                    if let Err(e) = motion(app, p) {
                        app.status_message = e;
                    }
                    owned = true;
                }
            }
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            } => {
                if let Some(b) = app.composer_state.selection.band.take() {
                    if let Err(e) = band_release(app, b, page(pos), modifiers) {
                        app.status_message = e;
                    }
                    owned = true;
                }
                if app.composer_state.selection.grab.is_some() {
                    if let Err(e) = finish_grab(app) {
                        app.status_message = e;
                    }
                    owned = true;
                }
            }
            egui::Event::PointerGone => {
                app.composer_state.selection.band = None;
                app.composer_state.selection.ctrl = false;
                if app.composer_state.selection.grab.is_some() {
                    if let Err(e) = finish_grab(app) {
                        app.status_message = e;
                    }
                    owned = true;
                }
            }
            _ => {}
        }
    }
    if let Some(b) = &app.composer_state.selection.band {
        let to = |p: DVec2| paper.min + egui::vec2(p.x as f32 * zoom, p.y as f32 * zoom);
        let r = egui::Rect::from_two_pos(to(b.start), to(b.current));
        let c = if b.current.x < b.start.x {
            egui::Color32::from_rgb(41, 158, 92)
        } else {
            egui::Color32::from_rgb(41, 128, 214)
        };
        ui.painter().rect_filled(r, 0., c.gamma_multiply(0.16));
        ui.painter().rect_stroke(r, 0., egui::Stroke::new(1., c));
    }
    owned
}

fn reidentify(item: &mut SheetItem, id: Uuid) {
    match item {
        SheetItem::ViewFrame(v) => v.id = id,
        SheetItem::Text(v) => v.id = id,
        SheetItem::TitleBlock(v) => v.id = id,
        SheetItem::GraphicScale(v) => v.id = id,
        SheetItem::NorthArrow(v) => v.id = id,
        SheetItem::Shape(v) => v.id = id,
    }
}
pub(crate) fn edit_primary(app: &mut IngeTrazoApp, fields: &Value) -> Result<(), String> {
    let primary = primary(app).ok_or("No selected item")?;
    let map = fields.as_object().ok_or("Invalid property fields")?;
    let before = snapshot(app, primary)?;
    let native = match &before {
        Record::Typed { native, .. } | Record::Raw { value: native, .. } => native,
    };
    let changed = map
        .iter()
        .filter(|(k, v)| native.get(*k) != Some(*v))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<serde_json::Map<_, _>>();
    if changed.is_empty() {
        if let Some((true, f)) = raw_slot(primary.category) {
            super::annotation_authoring::admit(native)?;
            app.remember_annotation_candidate(f, native);
        }
        return Ok(());
    }
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let mut candidates = Vec::new();
    candidates
        .try_reserve(app.composer_state.selection.members.len())
        .map_err(|_| "Property cohort admission refused")?;
    let mut signature = Vec::new();
    let mut count = 0;
    let mut bytes = 0;
    let members = std::iter::once(primary)
        .chain(
            app.composer_state
                .selection
                .members
                .iter()
                .copied()
                .filter(|&o| o != primary && o.category == primary.category),
        )
        .collect::<Vec<_>>();
    for owner in members {
        let rec = snapshot(app, owner)?;
        let mut value = match rec {
            Record::Typed { native, .. } | Record::Raw { value: native, .. } => native,
        };
        let applied = if owner == primary { map } else { &changed };
        if matches!(owner.category, 7 | 8 | 9 | 10 | 12 | 13) {
            super::annotation_authoring::patch(&mut value, &Value::Object(applied.clone()))?;
        } else {
            for (k, v) in applied {
                value[k] = v.clone();
            }
        }
        let candidate = if raw_slot(owner.category).is_some() {
            Record::Raw {
                category: owner.category,
                value,
            }
        } else {
            let mut item = ingetrazo_formats::igz::paper_item_from_record(
                KINDS[owner.category as usize],
                &value,
            )?;
            reidentify(&mut item, owner.id);
            Record::Typed {
                item,
                native: value,
            }
        };
        admit_record(&candidate, &mut count, &mut bytes)?;
        let mut keys = applied.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        signature.push((owner.id, keys));
        candidates.push((owner, candidate));
    }
    if let Some((true, f)) = raw_slot(primary.category) {
        if let Some((_, Record::Raw { value, .. })) = candidates.first() {
            app.remember_annotation_candidate(f, value);
        }
    }
    let combine = app.scene.sheets[s].edit_signature.as_ref() == Some(&signature);
    if !combine {
        app.finish_composer_edit();
        app.begin_composer_edit();
    }
    for (owner, record) in candidates {
        let i = locate(&app.scene.sheets[s], owner).ok_or("Property cohort owner changed")?;
        let sheet = &mut app.scene.sheets[s];
        match record {
            Record::Typed { item, native } => {
                sheet.native_items.insert(owner.id, native);
                sheet
                    .native_item_baselines
                    .insert(owner.id, ingetrazo_formats::igz::paper_item_baseline(&item));
                sheet.items[i] = item;
            }
            Record::Raw { category, value } => {
                sheet.native_document[KINDS[category as usize]][i] = value;
                if let Some((true, f)) = raw_slot(category) {
                    app.invalidate_sheet_annotation(f, i);
                }
            }
        }
    }
    if combine {
        app.scene.bump_version();
    } else {
        app.finish_composer_edit();
    }
    app.scene.sheets[s].edit_signature = Some(signature);
    revalidate(app);
    if matches!(primary.category,7|8|9|10|12|13){app.composer_state.annotation_authoring.draft=None;}
    Ok(())
}
fn observe(ui: &egui::Ui, id: &str, r: egui::Rect) {
    if std::env::var_os("INGETRAZO_PDF_UI_OBSERVE").is_some() {
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(id), r));
    }
}
pub(super) fn properties(app: &mut IngeTrazoApp, ui: &egui::Ui) {
    let Some(owner) = primary(app).filter(|o| o.category == 1) else {
        return;
    };
    let Ok(s) = active(app) else { return };
    let Some(index) = locate(&app.scene.sheets[s], owner) else {
        return;
    };
    let mut draft = app
        .composer_state
        .selection
        .text_draft
        .take()
        .filter(|d| d.owner == owner && d.version == app.scene.version)
        .unwrap_or_else(|| {
            let SheetItem::Text(t) = &app.scene.sheets[s].items[index] else {
                unreachable!()
            };
            let v = app.scene.sheets[s]
                .native_items
                .get(&owner.id)
                .unwrap_or(&Value::Null);
            TextDraft {
                owner,
                version: app.scene.version,
                text: t.text.clone(),
                size: t.font_size_pt,
                bold: t.bold,
                italic: v.get("italic").and_then(Value::as_bool).unwrap_or(false),
                underline: v.get("underline").and_then(Value::as_bool).unwrap_or(false),
            }
        });
    let mut changed = false;
    egui::Window::new("Text properties")
        .id(egui::Id::new("composer-mixed-text"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12., 116.))
        .default_width(260.)
        .show(ui.ctx(), |ui| {
            let r = ui.text_edit_multiline(&mut draft.text);
            observe(ui, "mixed-ui-text", r.rect);
            changed |= r.changed();
            let r = ui.add(
                egui::DragValue::new(&mut draft.size)
                    .range(0.1..=1000.)
                    .suffix(" pt"),
            );
            observe(ui, "mixed-ui-size", r.rect);
            changed |= r.changed();
            changed |= ui.checkbox(&mut draft.bold, "Bold").changed();
            changed |= ui.checkbox(&mut draft.italic, "Italic").changed();
            changed |= ui.checkbox(&mut draft.underline, "Underline").changed();
        });
    if changed {
        if let Err(e) = edit_primary(
            app,
            &json!({"text":draft.text,"size_pt":draft.size,"bold":draft.bold,"italic":draft.italic,"underline":draft.underline}),
        ) {
            app.status_message = e;
        }
        draft.version = app.scene.version;
    }
    app.composer_state.selection.text_draft = Some(draft);
}
impl IngeTrazoApp {
    pub fn composer_selection_observation(&self) -> Value {
        json!({"selection_order":self.composer_sheet_selections().iter().map(|(f,i)|json!({"family":f,"index":i})).collect::<Vec<_>>(),"panel_owner":self.composer_sheet_primary().map(|(f,i)|json!({"family":f,"index":i}))})
    }
    pub fn composer_sheet_selections(&self) -> Vec<(String, usize)> {
        let Some(s) = self
            .scene
            .active_sheet_index
            .and_then(|s| self.scene.sheets.get(s))
        else {
            return Vec::new();
        };
        selected_owners(self)
            .iter()
            .filter_map(|&o| {
                locate(s, o).map(|i| {
                    (
                        KINDS[o.category as usize].into(),
                        if raw_slot(o.category).is_none() {
                            s.items[..i]
                                .iter()
                                .filter(|v| category(v) == o.category)
                                .count()
                        } else {
                            i
                        },
                    )
                })
            })
            .collect()
    }
    pub fn composer_sheet_primary(&self) -> Option<(String, usize)> {
        let s = self
            .scene
            .active_sheet_index
            .and_then(|s| self.scene.sheets.get(s))?;
        let o = primary(self)?;
        let i = locate(s, o)?;
        Some((
            KINDS[o.category as usize].into(),
            if raw_slot(o.category).is_none() {
                s.items[..i]
                    .iter()
                    .filter(|v| category(v) == o.category)
                    .count()
            } else {
                i
            },
        ))
    }
    pub fn composer_select_sheet_item(
        &mut self,
        category_name: &str,
        index: usize,
        extend: bool,
    ) -> Result<(), String> {
        let c = KINDS
            .iter()
            .position(|&v| v == category_name)
            .ok_or("Unknown sheet kind")? as u8;
        let s = active(self)?;
        let index = if raw_slot(c).is_none() {
            self.scene.sheets[s]
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| category(i) == c)
                .nth(index)
                .map(|(i, _)| i)
                .ok_or("Sheet item missing")?
        } else {
            index
        };
        force_select(self, c, index, extend)
    }
    pub fn composer_edit_sheet_selection(&mut self, fields: &Value) -> Result<(), String> {
        edit_primary(self, fields)
    }
    pub fn composer_delete_sheet_selection(&mut self) -> Result<bool, String> {
        delete_selected(self)
    }
    pub fn composer_sheet_selection_stats(&self) -> (u64, u64) {
        (
            self.composer_state.selection.builds,
            self.composer_state.selection.updates,
        )
    }
    pub fn composer_sheet_pointer_for_control(&mut self, point: [f64; 2]) -> Result<(), String> {
        motion(self, DVec2::from_array(point))
    }
}

pub(crate) fn admit_add(app: &IngeTrazoApp, n: usize) -> Result<(), String> {
    let count = if app.composer_state.selection.cohort_mode {
        app.composer_state.selection.members.len()
    } else {
        app.composer_state.annotation_authoring.multi.members.len()
    };
    if count.checked_add(n).is_none_or(|n| n > LIMIT) {
        return Err("Sheet cohort budget exceeded".into());
    }
    Ok(())
}
pub(super) fn paint(app: &IngeTrazoApp, painter: &egui::Painter, paper: egui::Pos2, zoom: f32) {
    for &owner in &app.composer_state.selection.members {
        if let Some(r) = app.composer_state.selection.display.get(&owner) {
            let a = paper + egui::vec2(r.x as f32 * zoom, r.y as f32 * zoom);
            super::render_selection_box(
                painter,
                egui::Rect::from_min_size(a, egui::vec2(r.w as f32 * zoom, r.h as f32 * zoom)),
            );
        }
    }
}
pub(super) fn adopt(app: &mut IngeTrazoApp) -> Result<(), String> {
    if app.composer_state.selection.cohort_mode {
        return Ok(());
    }
    let s = active(app)?;
    ensure(&mut app.scene.sheets[s])?;
    let sheet = &app.scene.sheets[s];
    let mut owners = Vec::new();
    if let Some(i) = sheet.selected_item {
        if let Some(item) = sheet.items.get(i) {
            owners.push(Owner {
                sheet: sheet.id,
                category: category(item),
                id: item.id(),
            });
        }
    }
    if let Some((sid, i)) = app.composer_picture_selection() {
        if sid == sheet.id {
            if let Some(o) = owner_at(sheet, 2, i) {
                owners.push(o);
            }
        }
    }
    if let Some((sid, i)) = app.composer_legend_selection() {
        if sid == sheet.id {
            if let Some(o) = owner_at(sheet, 5, i) {
                owners.push(o);
            }
        }
    }
    for (f, i) in app.composer_annotation_selections() {
        let c = [7u8, 8, 9, 10, 12, 13][f];
        if let Some(o) = owner_at(sheet, c, i) {
            owners.push(o);
        }
    }
    if !owners.is_empty() {
        set(app, owners, None)?;
    }
    Ok(())
}
