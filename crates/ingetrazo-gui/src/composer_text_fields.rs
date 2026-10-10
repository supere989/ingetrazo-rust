// SPDX-License-Identifier: GPL-3.0-or-later
//! Source dynamic Text fields. Context borrows the live document and never mutates authored text.
use ingetrazo_core::composition::{Sheet, SheetItem, TextItem};
use std::path::Path;
/// Python's default :g is six significant digits, unlike the drafting scale display's four decimals.
fn source_scale(n: f64) -> String {
    if n <= 0. {
        return String::new();
    }
    let v = if n < 1. { 1. / n } else { n };
    let number = if v.is_nan() {
        "nan".into()
    } else if v.is_infinite() {
        "inf".into()
    } else {
        let scientific = format!("{v:.5e}");
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        let e: i32 = exponent.parse().unwrap();
        if !(-4..6).contains(&e) {
            format!(
                "{}e{}{exponent:02}",
                mantissa.trim_end_matches('0').trim_end_matches('.'),
                if e < 0 { "-" } else { "+" },
                exponent = e.abs()
            )
        } else {
            let raw = format!("{:.*}", (5 - e).max(0) as usize, v);
            if raw.contains('.') {
                raw.trim_end_matches('0').trim_end_matches('.').into()
            } else {
                raw
            }
        }
    };
    if n < 1. {
        format!("{number}:1")
    } else {
        format!("1:{number}")
    }
}
/// Source date.today uses the native local calendar; never a UTC or stale authored-date substitute.
pub(crate) fn today() -> Result<String, String> {
    #[cfg(unix)]
    let (year, month, day) = unsafe {
        let now = libc::time(std::ptr::null_mut());
        if now == -1 {
            return Err("Native calendar clock failed".into());
        }
        let mut local: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut local).is_null() {
            return Err("Native local calendar conversion failed".into());
        }
        (local.tm_year + 1900, local.tm_mon + 1, local.tm_mday)
    };
    #[cfg(windows)]
    let (year, month, day) = unsafe {
        #[repr(C)]
        struct SystemTime {
            year: u16,
            month: u16,
            weekday: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            millis: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(time: *mut SystemTime);
        }
        let mut local: SystemTime = std::mem::zeroed();
        GetLocalTime(&mut local);
        (
            i32::from(local.year),
            i32::from(local.month),
            i32::from(local.day),
        )
    };
    #[cfg(not(any(unix, windows)))]
    return Err("Native local calendar provider unavailable".into());
    #[cfg(any(unix, windows))]
    {
        if !(1..=9999).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err("Invalid native local calendar".into());
        }
        Ok(format!("{day:02}/{month:02}/{year:04}"))
    }
}
/// Compare source dataclass records, excluding opaque archive keys and Rust UI identities.
/// Only duplicate sheet headers need this cold bounded key; each candidate is admitted once/export.
fn equality_key(sheet: &Sheet) -> Result<Vec<u8>, String> {
    use serde_json::{Map, Value};
    static SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("paper_composition_context_schema.json"))
            .expect("pinned source dataclass schema")
    });
    let mut raw = ingetrazo_formats::igz::paper_context_sheet_record(sheet, &SCHEMA["classes"])?;
    // Runtime original Frame identity is the literal retained UID, also used by field binding.
    if let Some(records) = raw
        .get_mut("frames")
        .and_then(serde_json::Value::as_array_mut)
    {
        for (record, frame) in records.iter_mut().zip(sheet.items.iter().filter_map(|v| {
            if let SheetItem::ViewFrame(f) = v {
                Some(f)
            } else {
                None
            }
        })) {
            if let Some(uid) = sheet
                .native_items
                .get(&frame.id)
                .and_then(|v| v.get("uid"))
                .and_then(serde_json::Value::as_str)
            {
                record["uid"] = serde_json::Value::String(uid.into());
            }
        }
    }
    let mut model = Map::new();
    for k in ["name", "paper", "landscape", "margin_mm"] {
        model.insert(
            k.into(),
            raw.get(k).unwrap_or(&SCHEMA["sheet_defaults"][k]).clone(),
        );
    }
    for (kind, defaults) in SCHEMA["classes"].as_object().unwrap() {
        let item = |v: &Value| -> Value {
            let mut fields = Map::new();
            for (k, d) in defaults.as_object().unwrap() {
                fields.insert(k.clone(), v.get(k).unwrap_or(d).clone());
            }
            Value::Object(fields)
        };
        let value = if kind == "cajetin" {
            raw.get(kind)
                .filter(|v| v.is_object())
                .map(item)
                .unwrap_or(Value::Null)
        } else {
            Value::Array(
                raw.get(kind)
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter(|v| v.is_object()).map(item).collect())
                    .unwrap_or_default(),
            )
        };
        model.insert(kind.clone(), value);
    }
    let border = raw.get("border");
    let on = border
        .and_then(|v| v.get("on"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    model.insert("border".into(), Value::Bool(on));
    for (k, record) in [
        ("border_mm", "mm"),
        ("border_color", "color"),
        ("border_radius_mm", "radius_mm"),
        ("border_style", "style"),
    ] {
        model.insert(
            k.into(),
            {
                border
                    .and_then(|v| v.get(record))
                    .unwrap_or(&SCHEMA["sheet_defaults"][k])
                    .clone()
            },
        );
    }
    for (k, record) in [("guides_v", "v"), ("guides_h", "h")] {
        model.insert(
            k.into(),
            raw.get("guides")
                .and_then(|v| v.get(record))
                .unwrap_or(&SCHEMA["sheet_defaults"][k])
                .clone(),
        );
    }
    fn append(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
        if out
            .len()
            .checked_add(bytes.len())
            .filter(|n| *n <= 4 * 1024 * 1024)
            .is_none()
        {
            return Err("Paper context key budget exceeded".into());
        }
        out.try_reserve(bytes.len())
            .map_err(|_| "Paper context key allocation failed")?;
        out.extend_from_slice(bytes);
        Ok(())
    }
    fn emit(out: &mut Vec<u8>, v: &Value) -> Result<(), String> {
        match v {
            Value::Null => append(out, b"N"),
            Value::Bool(b) => {
                append(out, b"i")?;
                append(out, &(if *b { 1i128 } else { 0i128 }).to_be_bytes())
            }
            Value::Number(n) => {
                // Python int/float equality is exact: preserve integers above the f64 mantissa range.
                if let Some(i) = n.as_i64() {
                    append(out, b"i")?;
                    return append(out, &i128::from(i).to_be_bytes());
                }
                if let Some(i) = n.as_u64() {
                    append(out, b"i")?;
                    return append(out, &i128::from(i).to_be_bytes());
                }
                let f = n.as_f64().ok_or("Paper context number unsupported")?;
                if !f.is_finite() {
                    return Err("Paper context number is nonfinite".into());
                }
                if f.fract() == 0. && f >= i64::MIN as f64 && f < 18446744073709551616f64 {
                    append(out, b"i")?;
                    append(out, &(f as i128).to_be_bytes())
                } else {
                    append(out, b"f")?;
                    append(out, &f.to_bits().to_be_bytes())
                }
            }
            Value::String(s) => {
                append(out, b"s")?;
                append(out, &(s.len() as u64).to_be_bytes())?;
                append(out, s.as_bytes())
            }
            Value::Array(a) => {
                append(out, b"a")?;
                append(out, &(a.len() as u64).to_be_bytes())?;
                for v in a {
                    emit(out, v)?;
                }
                Ok(())
            }
            Value::Object(o) => {
                append(out, b"o")?;
                append(out, &(o.len() as u64).to_be_bytes())?;
                let mut entries = Vec::new();
                entries
                    .try_reserve(o.len())
                    .map_err(|_| "Paper context object allocation failed")?;
                entries.extend(o.iter());
                entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
                for (k, v) in entries {
                    append(out, b"s")?;
                    append(out, &(k.len() as u64).to_be_bytes())?;
                    append(out, k.as_bytes())?;
                    emit(out, v)?;
                }
                Ok(())
            }
        }
    }
    let mut out = vec![];
    emit(&mut out, &Value::Object(model))?;
    Ok(out)
}
/// Actual source Frame UID is an equality field; short-circuit independent placements without record copies.
fn same_frame_identities(a: &Sheet, b: &Sheet) -> bool {
    let frames = |s: &Sheet| {
        s.items
            .iter()
            .filter_map(|v| {
                if let SheetItem::ViewFrame(f) = v {
                    Some(f)
                } else {
                    None
                }
            })
            .count()
    };
    if frames(a) != frames(b) {
        return false;
    }
    let mut right = b.items.iter().filter_map(|v| {
        if let SheetItem::ViewFrame(f) = v {
            Some(f)
        } else {
            None
        }
    });
    for left in a.items.iter().filter_map(|v| {
        if let SheetItem::ViewFrame(f) = v {
            Some(f)
        } else {
            None
        }
    }) {
        let right = right.next().unwrap();
        let av = a.native_items.get(&left.id).and_then(|v| v.get("uid"));
        let bv = b.native_items.get(&right.id).and_then(|v| v.get("uid"));
        // Non-string foreign IDs require the complete typed comparison rather than a guessed identity.
        if av.is_some_and(|v| !v.is_string()) || bv.is_some_and(|v| !v.is_string()) {
            continue;
        }
        let mut abuf = [0; 32];
        let mut bbuf = [0; 32];
        let aid = av
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| left.id.simple().encode_lower(&mut abuf));
        let bid = bv
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| right.id.simple().encode_lower(&mut bbuf));
        if aid != bid {
            return false;
        }
    }
    true
}
pub(crate) fn source_indices<'a>(
    all: &'a [Sheet],
    selected: &'a [Sheet],
) -> Result<std::collections::HashMap<*const Sheet, usize>, String> {
    use std::collections::HashMap;
    let mut indices = HashMap::new();
    indices
        .try_reserve(selected.len())
        .map_err(|_| "Paper context index allocation failed")?;
    let mut keys: HashMap<*const Sheet, Vec<u8>> = HashMap::new();
    let mut bytes = 0usize;
    for sheet in selected {
        let mut index = None;
        for (i, candidate) in all.iter().enumerate() {
            if std::ptr::eq(sheet, candidate) {
                index = Some(i);
                break;
            }
            if sheet.name != candidate.name
                || sheet.paper_size != candidate.paper_size
                || sheet.orientation != candidate.orientation
                || sheet.margin_mm != candidate.margin_mm
            {
                continue;
            }
            if !same_frame_identities(sheet, candidate) {
                continue;
            }
            for item in [sheet, candidate] {
                let p = item as *const Sheet;
                if !keys.contains_key(&p) {
                    let key = equality_key(item)?;
                    bytes = bytes
                        .checked_add(key.len())
                        .ok_or("Paper context aggregate overflow")?;
                    if bytes > 32 * 1024 * 1024 {
                        return Err("Paper context aggregate budget exceeded".into());
                    }
                    keys.try_reserve(1)
                        .map_err(|_| "Paper context cache allocation failed")?;
                    keys.insert(p, key);
                }
            }
            if keys[&(sheet as *const _)] == keys[&(candidate as *const _)] {
                index = Some(i);
                break;
            }
        }
        indices.insert(
            sheet as *const _,
            index.ok_or("Paper Text context no longer exists")?,
        );
    }
    Ok(indices)
}
pub(crate) struct Context<'a> {
    pub index: Option<usize>,
    pub total: usize,
    pub path: Option<&'a Path>,
    pub date: &'a str,
}
pub(crate) fn expand(
    sheet: &Sheet,
    item: &TextItem,
    context: &Context<'_>,
) -> Result<String, String> {
    const LIMIT: usize = ingetrazo_formats::paper_text_layout::MAX_LINE_BYTES;
    if item.text.len() > LIMIT {
        return Err("Dynamic paper Text input exceeds byte budget".into());
    }
    let title = sheet.items.iter().find_map(|v| {
        if let SheetItem::TitleBlock(t) = v {
            Some(t)
        } else {
            None
        }
    });
    let frames = || {
        sheet.items.iter().filter_map(|v| {
            if let SheetItem::ViewFrame(f) = v {
                Some(f)
            } else {
                None
            }
        })
    };
    let mut main = None;
    let mut area = f64::NEG_INFINITY;
    for frame in frames() {
        let a = frame.rect.w * frame.rect.h;
        if main.is_none() || a > area {
            main = Some(frame);
            area = a;
        }
    }
    let bound = sheet
        .native_items
        .get(&item.id)
        .and_then(|v| v.get("frame_uid"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !bound.is_empty() {
        if let Some(frame) = frames().find(|f| {
            sheet
                .native_items
                .get(&f.id)
                .and_then(|v| v.get("uid"))
                .and_then(|v| v.as_str())
                .map(|uid| uid == bound)
                .unwrap_or_else(|| {
                    let mut uid = [0; 32];
                    f.id.simple().encode_lower(&mut uid) == bound
                })
        }) {
            main = Some(frame);
        }
    }
    let scene = main
        .into_iter()
        .chain(frames())
        .find_map(|f| f.view_key.strip_prefix("scene:"))
        .unwrap_or("");
    let scale = main.map(|f| source_scale(f.scale_n)).unwrap_or_default();
    let filename = context
        .path
        .and_then(Path::file_stem)
        .map(|v| v.to_string_lossy())
        .unwrap_or_default();
    let page = context
        .index
        .map(|v| v.checked_add(1).ok_or("Paper sheet index overflow"))
        .transpose()?
        .map(|v| v.to_string())
        .unwrap_or_default();
    let total = if context.total == 0 {
        String::new()
    } else {
        context.total.to_string()
    };
    let lamina = title
        .map(|t| t.sheet_number.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&sheet.name);
    let values = [
        (
            "proyecto",
            title.map(|t| t.project_name.as_str()).unwrap_or(""),
        ),
        ("autor", title.map(|t| t.author.as_str()).unwrap_or("")),
        ("lamina", lamina),
        ("nombre", sheet.name.as_str()),
        ("escala", scale.as_str()),
        ("escena", scene),
        ("fecha", context.date),
        ("archivo", filename.as_ref()),
        ("hoja", page.as_str()),
        ("total", total.as_str()),
    ];
    let text = item.text.as_bytes();
    let mut out = String::new();
    let mut start = 0;
    let mut i = 0;
    let append = |out: &mut String, part: &str| -> Result<(), String> {
        if out
            .len()
            .checked_add(part.len())
            .filter(|n| *n <= LIMIT)
            .is_none()
        {
            return Err("Expanded paper Text exceeds byte budget".into());
        }
        out.try_reserve(part.len())
            .map_err(|_| "Dynamic paper Text allocation failed")?;
        out.push_str(part);
        Ok(())
    };
    while i < text.len() {
        if text[i] != b'{' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < text.len() && (text[j].is_ascii_alphabetic() || text[j] == b'_') {
            j += 1;
        }
        if j > i + 1 && text.get(j) == Some(&b'}') {
            append(&mut out, &item.text[start..i])?;
            let field = item.text[i + 1..j].to_ascii_lowercase();
            let value = values
                .iter()
                .find(|(key, _)| *key == field)
                .map(|(_, v)| *v)
                .unwrap_or(&item.text[i..=j]);
            append(&mut out, value)?;
            i = j + 1;
            start = i;
        } else {
            i += 1;
        }
    }
    append(&mut out, &item.text[start..])?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::composition::{Orientation, PaperSize, TitleBlock, ViewFrame};
    #[test]
    fn actual_original_contexts_and_general_scale_controls() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/paper-text-fields-source/source.json"
        ))
        .unwrap();
        for row in source["scales"].as_array().unwrap() {
            assert_eq!(
                source_scale(row["value"].as_f64().unwrap()),
                row["formatted"].as_str().unwrap()
            );
        }
        for case in source["cases"].as_array().unwrap() {
            let r = &case["recipe"];
            let mut sheet = Sheet::new(
                r["sheet"].as_str().unwrap_or(""),
                PaperSize::A4,
                Orientation::Portrait,
            );
            sheet.items.clear();
            if let Some(title) = r.get("title") {
                let mut t = TitleBlock::default();
                t.project_name = title["proyecto"].as_str().unwrap_or("").into();
                t.author = title["autor"].as_str().unwrap_or("").into();
                t.sheet_number = title["lamina"].as_str().unwrap_or("L-01").into();
                sheet.items.push(SheetItem::TitleBlock(t));
            }
            if let Some(frames) = r["frames"].as_array() {
                for f in frames {
                    let mut frame = ViewFrame::default();
                    frame.rect.w = f["w_mm"].as_f64().unwrap();
                    frame.rect.h = f["h_mm"].as_f64().unwrap();
                    frame.scale_n = f["scale_n"].as_f64().unwrap();
                    frame.view_key = f["view_key"].as_str().unwrap().into();
                    sheet.native_items.insert(
                        frame.id,
                        serde_json::json!({"uid":f["uid"],"hidden":f["hidden"]}),
                    );
                    sheet.items.push(SheetItem::ViewFrame(frame));
                }
            }
            let item = TextItem {
                id: uuid::Uuid::new_v4(),
                text: case["text"].as_str().unwrap().into(),
                pos: (20., 30.),
                font_size_pt: 14.,
                bold: false,
            };
            sheet
                .native_items
                .insert(item.id, serde_json::json!({"frame_uid":r["frame_uid"]}));
            let path = r["path"].as_str().map(Path::new);
            let context = Context {
                index: r["index"].as_u64().map(|v| v as usize),
                total: r["total"].as_u64().unwrap_or(0) as usize,
                path,
                date: case["source_values"]["fecha"].as_str().unwrap(),
            };
            let before = sheet.clone();
            assert_eq!(
                expand(&sheet, &item, &context).unwrap(),
                case["expanded"].as_str().unwrap(),
                "{}",
                r["name"]
            );
            assert_eq!(sheet, before);
        }
    }
    #[test]
    fn expanded_text_refusal_does_not_mutate_authored_items() {
        let mut sheet = Sheet::new("Bounded", PaperSize::A4, Orientation::Portrait);
        sheet.items.clear();
        let mut title = TitleBlock::default();
        title.project_name = "x".repeat(65536);
        sheet.items.push(SheetItem::TitleBlock(title));
        let item = TextItem {
            id: uuid::Uuid::new_v4(),
            text: "{proyecto}x".into(),
            pos: (0., 0.),
            font_size_pt: 14.,
            bold: false,
        };
        let before = sheet.clone();
        assert!(expand(
            &sheet,
            &item,
            &Context {
                index: Some(0),
                total: 1,
                path: None,
                date: "10/10/2026"
            }
        )
        .is_err());
        assert_eq!(sheet, before);
        assert_eq!(item.text, "{proyecto}x");
    }
    #[test]
    fn source_caller_first_equal_sheet_and_different_position() {
        use ingetrazo_core::composition::{Orientation, PaperSize};
        let make = || {
            let mut sheet = Sheet::new("Same", PaperSize::A4, Orientation::Portrait);
            sheet.items.clear();
            sheet.items.push(SheetItem::Text(TextItem {
                id: uuid::Uuid::new_v4(),
                pos: (20., 20.),
                text: "{hoja}/{total}".into(),
                font_size_pt: 14.,
                bold: false,
            }));
            sheet
        };
        let first = make();
        let mut second = make();
        second.native_items.insert(
            second.items[0].id(),
            serde_json::json!({"vendor":{"opaque":71}}),
        );
        let mut all = vec![first, second];
        let before = all.clone();
        let values = source_indices(&all, &all[1..]).unwrap();
        assert_eq!(values[&(&all[1] as *const Sheet)], 0);
        assert_eq!(all, before);
        all[1].items[0].set_pos(21., 20.);
        let values = source_indices(&all, &all[1..]).unwrap();
        assert_eq!(values[&(&all[1] as *const Sheet)], 1);
        // Original caller's dataclass comparison includes literal Frame UID.
        for (i, sheet) in all.iter_mut().enumerate() {
            sheet.items[0].set_pos(20., 20.);
            let f = ingetrazo_core::composition::ViewFrame::default();
            sheet.native_items.insert(
                f.id,
                serde_json::json!({"uid":if i==0{"first"}else{"second"}}),
            );
            sheet.items.push(SheetItem::ViewFrame(f));
        }
        let values = source_indices(&all, &all[1..]).unwrap();
        assert_eq!(values[&(&all[1] as *const Sheet)], 1);
        for sheet in &mut all {
            let id = sheet.items[1].id();
            sheet
                .native_items
                .insert(id, serde_json::json!({"uid":"literal-same"}));
        }
        let values = source_indices(&all, &all[1..]).unwrap();
        assert_eq!(values[&(&all[1] as *const Sheet)], 0);
    }
    #[test]
    fn source_context_distinguishes_exact_large_metadata_integers() {
        use ingetrazo_core::composition::{Orientation, PaperSize};
        let mut a = Sheet::new("Same", PaperSize::A4, Orientation::Portrait);
        a.items.clear();
        a.items.push(SheetItem::Text(TextItem {
            id: uuid::Uuid::new_v4(),
            pos: (20., 20.),
            text: "{hoja}".into(),
            font_size_pt: 14.,
            bold: false,
        }));
        let mut b = a.clone();
        let id = a.items[0].id();
        a.native_items
            .insert(id, serde_json::json!({"z":9007199254740992u64}));
        b.native_items
            .insert(id, serde_json::json!({"z":9007199254740993u64}));
        let all = vec![a, b];
        assert_eq!(
            source_indices(&all, &all[1..]).unwrap()[&(&all[1] as *const Sheet)],
            1
        );
    }
    #[test]
    fn opaque_archive_payload_is_not_copied_or_admitted_for_context() {
        use ingetrazo_core::composition::{Orientation, PaperSize};
        let mut a = Sheet::new("Same", PaperSize::A4, Orientation::Portrait);
        a.items.clear();
        a.items.push(SheetItem::Text(TextItem {
            id: uuid::Uuid::new_v4(),
            pos: (20., 20.),
            text: "{hoja}".into(),
            font_size_pt: 14.,
            bold: false,
        }));
        let mut b = a.clone();
        b.native_document = serde_json::json!({"vendor": "x".repeat(5*1024*1024)});
        let all = vec![a, b];
        assert_eq!(
            source_indices(&all, &all[1..]).unwrap()[&(&all[1] as *const Sheet)],
            0
        );
    }
    #[test]
    fn inactive_border_runtime_distinguishes_authored_but_import_resets() {
        use ingetrazo_core::composition::{Orientation, PaperSize};
        let mut a = Sheet::new("Same", PaperSize::A4, Orientation::Portrait);
        a.items.clear();
        a.items.push(SheetItem::Text(TextItem {
            id: uuid::Uuid::new_v4(),
            pos: (20., 20.),
            text: "{hoja}".into(),
            font_size_pt: 14.,
            bold: false,
        }));
        let mut b = a.clone();
        let mut border=b.effective_source_border();
        border.color="#ffffff".into();
        b.set_source_border(border).unwrap();
        let all = vec![a.clone(), b.clone()];
        let before = all.clone();
        assert_eq!(source_indices(&all,&all[1..]).unwrap()[&(&all[1] as *const Sheet)],1);
        assert_eq!(all,before);
        b.source_border=None;
        let loaded=vec![a,b];
        assert_eq!(source_indices(&loaded,&loaded[1..]).unwrap()[&(&loaded[1] as *const Sheet)],0);
    }
}
