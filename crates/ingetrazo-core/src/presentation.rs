//! Source document presentation. Storage is f64; platform rendering converts at submission.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceMode {
    #[default]
    Textures,
    Shaded,
    HiddenLine,
    Monochrome,
    Wireframe,
    Xray,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentStyle {
    pub name: String,
    pub face_mode: FaceMode,
    pub edges: bool,
    pub profiles: bool,
    pub back_edges: bool,
    pub edge_color: [f64; 3],
    pub front_color: [f64; 3],
    pub back_color: Option<[f64; 3]>,
    pub background: [f64; 3],
    pub sky: bool,
    pub sky_color: [f64; 3],
    pub ground_color: [f64; 3],
    pub section_fill: bool,
    pub section_fill_color: [f64; 3],
    pub source_values: serde_json::Map<String, serde_json::Value>,
    pub source_baseline: serde_json::Map<String, serde_json::Value>,
}
impl Default for DocumentStyle {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            face_mode: FaceMode::Textures,
            edges: true,
            profiles: true,
            back_edges: false,
            edge_color: [0.13, 0.17, 0.23],
            front_color: [1.; 3],
            back_color: None,
            background: [0.90, 0.91, 0.92],
            sky: true,
            sky_color: [0.925, 0.935, 0.945],
            ground_color: [0.815, 0.820, 0.815],
            section_fill: true,
            section_fill_color: [0.35, 0.37, 0.41],
            source_values: Default::default(),
            source_baseline: Default::default(),
        }
    }
}
fn truth(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64() != Some(0.),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(a) => !a.is_empty(),
        serde_json::Value::Object(o) => !o.is_empty(),
    }
}
fn number(v: &serde_json::Value) -> Option<f64> {
    v.as_bool()
        .map(|b| if b { 1. } else { 0. })
        .or_else(|| v.as_f64())
        .or_else(|| v.as_str()?.trim().replace("_", "").parse().ok())
        .filter(|v| v.is_finite())
}
fn rgb(v: &serde_json::Value) -> Option<[f64; 3]> {
    let a = tuple(v).ok()?;
    let a: Vec<_> = a.iter().map(number).collect::<Option<_>>()?;
    Some([*a.first()?, *a.get(1)?, *a.get(2)?])
}
// Python tuple preserves every source element; the GPU view consumes three
// numeric channels separately. Source recipe edits therefore never quantize or
// truncate unrelated authored representation.
fn tuple(value: &serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    match value {
        serde_json::Value::Array(a) => Ok(a.clone()),
        serde_json::Value::String(s) => Ok(s
            .chars()
            .map(|c| serde_json::Value::String(c.to_string()))
            .collect()),
        serde_json::Value::Object(o) => Ok(o
            .keys()
            .map(|k| serde_json::Value::String(k.clone()))
            .collect()),
        _ => Err("TypeError".into()),
    }
}
fn integer(value: &serde_json::Value) -> Option<i128> {
    match value {
        serde_json::Value::Bool(b) => Some(i128::from(*b)),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(i128::from)
            .or_else(|| n.as_u64().map(i128::from))
            .or_else(|| {
                let f = n.as_f64()?;
                if f.is_finite() && f >= i128::MIN as f64 && f < i128::MAX as f64 {
                    Some(f.trunc() as i128)
                } else {
                    None
                }
            }),
        serde_json::Value::String(s) => {
            let s = s.trim();
            let s = s.strip_prefix('+').unwrap_or(s);
            let body = s.strip_prefix('-').unwrap_or(s);
            if body.is_empty()
                || !body.chars().all(|c| c.is_ascii_digit() || c == '_')
                || body.starts_with('_')
                || body.ends_with('_')
                || body.contains("__")
            {
                return None;
            }
            s.replace('_', "").parse().ok()
        }
        _ => None,
    }
}
impl Serialize for DocumentStyle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}
impl<'de> Deserialize<'de> for DocumentStyle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_from_json(&serde_json::Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
impl Serialize for ShadowSettings {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(s)
    }
}
impl<'de> Deserialize<'de> for ShadowSettings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::try_from_json(&serde_json::Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
impl DocumentStyle {
    pub fn from_json(raw: &serde_json::Value) -> Self {
        Self::try_from_json(raw).unwrap_or_default()
    }
    pub fn try_from_json(raw: &serde_json::Value) -> Result<Self, String> {
        let mut d = Self::default();
        if let Some(n) = raw.get("name").and_then(|v| v.as_str()) {
            d.name = n.into();
        }
        if let Some(mode) = raw.get("face_mode") {
            d.face_mode = serde_json::from_value(mode.clone()).unwrap_or_default();
        }
        for (key, target) in [
            ("edges", &mut d.edges),
            ("profiles", &mut d.profiles),
            ("back_edges", &mut d.back_edges),
            ("sky", &mut d.sky),
            ("section_fill", &mut d.section_fill),
        ] {
            if let Some(v) = raw.get(key) {
                *target = truth(v);
            }
        }
        for (key, target) in [
            ("edge_color", &mut d.edge_color),
            ("front_color", &mut d.front_color),
            ("background", &mut d.background),
            ("sky_color", &mut d.sky_color),
            ("ground_color", &mut d.ground_color),
            ("section_fill_color", &mut d.section_fill_color),
        ] {
            if let Some(value) = raw.get(key).and_then(rgb) {
                *target = value;
            }
        }
        d.back_color = raw.get("back_color").and_then(rgb);
        let baseline = d.typed_json();
        for key in [
            "name",
            "edge_color",
            "front_color",
            "background",
            "sky_color",
            "ground_color",
            "section_fill_color",
        ] {
            if let Some(value) = raw.get(key) {
                let preserved = if key == "name" {
                    value.clone()
                } else {
                    serde_json::Value::Array(tuple(value)?)
                };
                if preserved != baseline[key] {
                    d.source_values.insert(key.into(), preserved);
                    d.source_baseline.insert(key.into(), baseline[key].clone());
                }
            }
        }
        Ok(d)
    }
    fn typed_json(&self) -> serde_json::Value {
        serde_json::json!({"name":self.name,"face_mode":self.face_mode,"edges":self.edges,"profiles":self.profiles,"back_edges":self.back_edges,"edge_color":self.edge_color,"front_color":self.front_color,"back_color":self.back_color,"background":self.background,"sky":self.sky,"sky_color":self.sky_color,"ground_color":self.ground_color,"section_fill":self.section_fill,"section_fill_color":self.section_fill_color})
    }
    pub fn to_json(&self) -> serde_json::Value {
        let mut value = self.typed_json();
        for (k, v) in &self.source_values {
            if self.source_baseline.get(k) == value.get(k) {
                value[k] = v.clone();
            }
        }
        value
    }

    pub fn effective_back_color(&self, adopted: Option<[f64; 3]>) -> [f64; 3] {
        self.back_color.or(adopted).unwrap_or([0.62, 0.70, 0.78])
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowSettings {
    pub enabled: bool,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub darkness: f64,
    pub utc_offset: Option<i32>,
    pub source_utc_offset: Option<serde_json::Value>,
}
impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            month: 3,
            day: 21,
            hour: 12,
            minute: 0,
            darkness: 0.55,
            utc_offset: None,
            source_utc_offset: None,
        }
    }
}
impl ShadowSettings {
    pub fn from_json(raw: &serde_json::Value) -> Self {
        Self::try_from_json(raw).unwrap_or_default()
    }
    pub fn try_from_json(raw: &serde_json::Value) -> Result<Self, String> {
        let mut d = Self::default();
        if let Some(v) = raw.get("enabled") {
            d.enabled = truth(v);
        }
        for (key, target, lo, hi) in [
            ("month", &mut d.month, 1., 12.),
            ("day", &mut d.day, 1., 31.),
            ("hour", &mut d.hour, 0., 23.),
            ("minute", &mut d.minute, 0., 59.),
        ] {
            if let Some(v) = raw
                .get(key)
                .and_then(integer)
                .map(|v| v as f64)
                .filter(|v| *v >= lo && *v <= hi)
            {
                *target = v as u8;
            }
        }
        if let Some(v) = raw.get("darkness").and_then(number) {
            d.darkness = v.clamp(0., 1.);
        }
        if let Some(value) = raw.get("utc_offset").filter(|v| !v.is_null()) {
            let value = integer(value).ok_or_else(|| {
                if value.is_array() || value.is_object() {
                    "TypeError".to_string()
                } else {
                    "ValueError".to_string()
                }
            })?;
            d.utc_offset = i32::try_from(value).ok();
            if d.utc_offset.is_none() {
                d.source_utc_offset = Some(if let Ok(n) = i64::try_from(value) {
                    serde_json::json!(n)
                } else if let Ok(n) = u64::try_from(value) {
                    serde_json::json!(n)
                } else {
                    return Err("OverflowError".into());
                });
            }
        }
        Ok(d)
    }
    pub fn to_json(&self) -> serde_json::Value {
        let mut value = serde_json::json!({"enabled":self.enabled,"month":self.month,"day":self.day,"hour":self.hour,"minute":self.minute,"darkness":self.darkness});
        if let Some(offset) = self.utc_offset {
            value["utc_offset"] = serde_json::json!(offset);
        } else if let Some(offset) = &self.source_utc_offset {
            value["utc_offset"] = offset.clone();
        }
        value
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_source_tuple_and_nonstring_name_survive_unrelated_edit_and_runtime_serde() {
        let mut d=DocumentStyle::try_from_json(&serde_json::json!({"name":17,"edge_color":[0.1,0.2,0.3,0.4],"back_color":[0.1,0.2,0.3,"bad"]})).unwrap();
        d.sky = false;
        let raw = serde_json::to_value(&d).unwrap();
        assert_eq!(raw["name"], 17);
        assert_eq!(raw["edge_color"], serde_json::json!([0.1, 0.2, 0.3, 0.4]));
        assert!(raw["back_color"].is_null());
        assert_eq!(serde_json::from_value::<DocumentStyle>(raw).unwrap(), d);
        d.edge_color = [0.7, 0.8, 0.9];
        assert_eq!(
            d.to_json()["edge_color"],
            serde_json::json!([0.7, 0.8, 0.9])
        );
        let s = ShadowSettings::try_from_json(
            &serde_json::json!({"day":"12.8","minute":true,"utc_offset":4294967296_u64}),
        )
        .unwrap();
        assert_eq!(s.day, 21);
        assert_eq!(s.minute, 1);
        assert_eq!(s.to_json()["utc_offset"], 4294967296_u64);
        assert!(DocumentStyle::try_from_json(&serde_json::json!({"edge_color":null})).is_err());
        assert!(ShadowSettings::try_from_json(&serde_json::json!({"utc_offset":"-5.5"})).is_err());
    }
    #[test]
    fn defaults_and_validation_preserve_source_recipe() {
        let d = DocumentStyle::from_json(
            &serde_json::json!({"face_mode":"unsupported","back_color":[0.1,0.2,0.3,0.4],"edges":0}),
        );
        assert_eq!(d.face_mode, FaceMode::Textures);
        assert_eq!(d.back_color, Some([0.1, 0.2, 0.3]));
        assert!(!d.edges);
        let s = ShadowSettings::from_json(
            &serde_json::json!({"month":19,"day":"12","minute":-1,"darkness":1.2,"utc_offset":-5}),
        );
        assert_eq!(s.month, 3);
        assert_eq!(s.day, 12);
        assert_eq!(s.minute, 0);
        assert_eq!(s.darkness, 1.);
        assert_eq!(s.utc_offset, Some(-5));
    }
}
