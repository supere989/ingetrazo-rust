//! The original render_blender document payload and its admission rules.
use super::*;
use serde_json::{json, Value};
#[derive(Clone, Debug, PartialEq)]
pub struct BlenderDocument {
    pub ambience: Ambience,
    pub sun_scale: f64,
    pub lights: Vec<BlenderLight>,
}
impl BlenderDocument {
    pub fn from_json(raw: &Value) -> Self {
        let ambience = match raw.get("ambience").and_then(Value::as_str) {
            Some("night") => Ambience::Night,
            Some("overcast") => Ambience::Overcast,
            _ => Ambience::Day,
        };
        let sun = raw.get("sun_scale").and_then(number).unwrap_or(1.);
        Self {
            ambience,
            sun_scale: if sun.is_nan() { 2. } else { sun.clamp(0.1, 2.) },
            lights: clean_lights(&raw["lights"]),
        }
    }
    pub fn to_json(&self) -> Value {
        json!({"ambience":self.ambience.as_str(),"sun_scale":self.sun_scale,"lights":self.lights.iter().map(|lt|json!({"kind":lt.kind.as_str(),"pos":lt.pos,"dir":lt.dir,"kelvin":lt.kelvin,"color":lt.color,"power":lt.power,"angle":lt.angle,"on":lt.on,"name":lt.name})).collect::<Vec<_>>()})
    }
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_bool().map(|b| if b { 1. } else { 0. }))
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}
fn triple(v: &Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    if a.len() < 3 {
        return None;
    }
    Some([number(&a[0])?, number(&a[1])?, number(&a[2])?])
}
fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.),
        Value::String(v) => !v.is_empty(),
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
    }
}
fn name(v: &Value) -> String {
    match v {
        Value::String(v) => v.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        _ => v.to_string(),
    }
}
pub fn clean_lights(raw: &Value) -> Vec<BlenderLight> {
    raw.as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let kind = match v.get("kind").map_or(Some("point"), Value::as_str)? {
                "point" => LightKind::Point,
                "spot" => LightKind::Spot,
                _ => return None,
            };
            let position = v.get("pos")?;
            for x in position.as_array()? {
                number(x)?;
            }
            let pos = triple(position)?;
            let mut dir = if let Some(d) = v.get("dir") {
                for x in d.as_array()? {
                    number(x)?;
                }
                triple(d)?
            } else {
                [0., 0., -1.]
            };
            if !pos.iter().chain(dir.iter()).all(|v| v.is_finite()) {
                return None;
            }
            if dir.iter().map(|v| v * v).sum::<f64>().sqrt() < 1e-9 {
                dir = [0., 0., -1.];
            }
            let power = if let Some(p) = v.get("power") {
                number(p)?
            } else {
                if kind == LightKind::Point {
                    400.
                } else {
                    1500.
                }
            };
            let angle = if let Some(p) = v.get("angle") {
                number(p)?
            } else {
                60.
            };
            let mut kelvin = Some(DEFAULT_KELVIN);
            let mut rgb = None;
            if let Some(k) = v.get("kelvin").filter(|v| !v.is_null()) {
                if let Some(k) = number(k) {
                    kelvin = Some(if k.is_nan() {
                        MAX_KELVIN as u32
                    } else {
                        k.clamp(MIN_KELVIN, MAX_KELVIN) as u32
                    });
                }
            } else {
                match v.get("color") {
                    None | Some(Value::Null) => kelvin = Some(2700),
                    Some(Value::String(s)) => {
                        kelvin = Some(match s.as_str() {
                            "warm" => 2700,
                            "neutral" => 4000,
                            "cool" => 6500,
                            _ => DEFAULT_KELVIN,
                        })
                    }
                    Some(color) => {
                        if let Some(c) = triple(color) {
                            rgb = Some(c.map(|c| if c.is_nan() { 1. } else { c.clamp(0., 1.) }));
                            kelvin = None;
                        }
                    }
                }
            }
            let color =
                rgb.unwrap_or_else(|| kelvin_to_rgb(f64::from(kelvin.unwrap_or(DEFAULT_KELVIN))));
            Some(BlenderLight {
                kind,
                pos,
                dir,
                kelvin,
                color,
                power: if power.is_nan() {
                    0.
                } else {
                    power.clamp(0., 1e6)
                },
                angle: if angle.is_nan() {
                    1.
                } else {
                    angle.clamp(1., 179.)
                },
                on: v.get("on").map_or(true, truth),
                name: v.get("name").map_or(String::new(), name),
            })
        })
        .collect()
}
