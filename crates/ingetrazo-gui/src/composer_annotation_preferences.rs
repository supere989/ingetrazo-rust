//! Remembered dimension defaults belong to application preferences, not sheet Undo.
use crate::session_state::Preferences;
use serde_json::{Map, Value};
const FIELDS: [&str; 10] = [
    "text_mm",
    "decimals",
    "ends",
    "stroke_mm",
    "color",
    "offset_mm",
    "text_color",
    "text_bg",
    "text_bg_opacity",
    "units",
];
const KEY: &str = "composer/default_cota_style_3";
fn read(prefs: &Preferences, key: &str) -> Map<String, Value> {
    match prefs.extra.get(key) {
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
        Some(Value::Object(o)) => o.clone(),
        _ => Map::new(),
    }
}
pub(super) fn load(prefs: &Preferences) -> Map<String, Value> {
    let mut style = read(prefs, KEY);
    for (key, dropped) in [
        ("composer/default_cota_style_2", &["offset_mm"][..]),
        (
            "composer/default_cota_style",
            &["ends", "text_along", "text_pos", "offset_mm"][..],
        ),
    ] {
        if !style.is_empty() {
            break;
        }
        style = read(prefs, key);
        for field in dropped {
            style.remove(*field);
        }
    }
    style.retain(|k, _| FIELDS.contains(&k.as_str()));
    style
}
pub(super) fn remember(prefs: &mut Preferences, model: &Value) -> Map<String, Value> {
    let style = FIELDS
        .into_iter()
        .filter_map(|k| model.get(k).map(|v| (k.to_string(), v.clone())))
        .collect::<Map<_, _>>();
    // These admitted scalar values serialize without failure; preserve the source JSON setting shape.
    prefs.extra.insert(
        KEY.into(),
        Value::String(Value::Object(style.clone()).to_string()),
    );
    style
}
