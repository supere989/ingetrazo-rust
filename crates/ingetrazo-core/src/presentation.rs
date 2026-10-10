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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    v.as_f64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}
fn rgb(v: &serde_json::Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([number(a.first()?)?, number(a.get(1)?)?, number(a.get(2)?)?])
}
impl DocumentStyle {
    pub fn from_json(raw: &serde_json::Value) -> Self {
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
        d
    }
    pub fn effective_back_color(&self, adopted: Option<[f64; 3]>) -> [f64; 3] {
        self.back_color.or(adopted).unwrap_or([0.62, 0.70, 0.78])
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    pub enabled: bool,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub darkness: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utc_offset: Option<i32>,
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
        }
    }
}
impl ShadowSettings {
    pub fn from_json(raw: &serde_json::Value) -> Self {
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
                .and_then(number)
                .map(f64::trunc)
                .filter(|v| *v >= lo && *v <= hi)
            {
                *target = v as u8;
            }
        }
        if let Some(v) = raw.get("darkness").and_then(number) {
            d.darkness = v.clamp(0., 1.);
        }
        d.utc_offset = raw
            .get("utc_offset")
            .and_then(number)
            .map(f64::trunc)
            .filter(|v| *v >= i32::MIN as f64 && *v <= i32::MAX as f64)
            .map(|v| v as i32);
        d
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
