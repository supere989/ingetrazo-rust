// SPDX-License-Identifier: GPL-3.0-or-later
//! Native 3D text groups and the original document's regeneration contract.
use glam::DMat4;
use ingetrazo_core::groups::Group;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod font;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Params {
    pub text: String,
    pub font: String,
    pub bold: bool,
    pub italic: bool,
    pub height: f64,
    pub thickness: f64,
}
impl Default for Params {
    fn default() -> Self {
        Self {
            text: "IngeTrazo".into(),
            font: String::new(),
            bold: true,
            italic: false,
            height: 0.25,
            thickness: 0.05,
        }
    }
}
impl Params {
    pub fn validate(&self) -> Result<(), String> {
        if self.text.trim().is_empty() {
            return Err("Enter text to build geometry".into());
        }
        if !self.height.is_finite() || !(0.001..=1000.).contains(&self.height) {
            return Err("Text height must be between 0.001 and 1000 metres".into());
        }
        if !self.thickness.is_finite() || !(0.0..=1000.).contains(&self.thickness) {
            return Err("Text extrusion must be between 0 and 1000 metres".into());
        }
        Ok(())
    }
}

pub fn params(group: &Group) -> Option<Params> {
    let record = group.native_document.get("text3d")?.as_object()?;
    if record.is_empty() {
        return None;
    }
    serde_json::from_value(Value::Object(record.clone())).ok()
}

fn letter_groups(params: &Params, transform: DMat4) -> Result<Vec<Group>, String> {
    params.validate()?;
    let children = font::letters(params)?
        .into_iter()
        .map(|(name, mesh)| {
            let mut group = Group::with_mesh(name, mesh);
            group.transform = transform;
            group
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        return Err("Could not build geometry for that text".into());
    }
    Ok(children)
}

pub fn make_text_group(params: &Params) -> Result<Group, String> {
    let children = letter_groups(params, DMat4::IDENTITY)?;
    let mut group = Group::new(params.text.trim().chars().take(24).collect::<String>());
    group.native_document = json!({"component": false, "text3d": text_state(params, &children)?});
    group.children = children;
    Ok(group)
}

/// Prepare all letters before replacing any part of the existing container.
/// The container keeps its identity, own mesh, placement and opaque metadata.
pub fn rebuild_text_group(group: &Group, params: &Params) -> Result<Vec<Group>, String> {
    letter_groups(
        params,
        group
            .children
            .first()
            .map(|g| g.transform)
            .unwrap_or(DMat4::IDENTITY),
    )
}

pub fn text_state(params: &Params, children: &[Group]) -> Result<Value, String> {
    let mut value = serde_json::to_value(params).map_err(|e| e.to_string())?;
    value["hash"] = Value::String(letters_fingerprint(children)?);
    Ok(value)
}

pub fn text_is_pristine(group: &Group) -> bool {
    let Some(hash) = group
        .native_document
        .get("text3d")
        .and_then(|s| s.get("hash"))
    else {
        return true;
    };
    if match hash {
        Value::Null => true,
        Value::Bool(v) => !*v,
        Value::Number(v) => v.as_f64() == Some(0.),
        Value::String(v) => v.is_empty(),
        Value::Array(v) => v.is_empty(),
        Value::Object(v) => v.is_empty(),
    } {
        return true;
    }
    let Some(expected) = hash.as_str() else {
        return false;
    };
    if letters_fingerprint(&group.children).as_deref() != Ok(expected) {
        return false;
    }
    let frame = group
        .children
        .first()
        .map(|g| g.transform)
        .unwrap_or(DMat4::IDENTITY);
    group.children.iter().all(|g| g.transform == frame)
}

// Python round(value, digits) rounds the exact binary input to a decimal
// precision. Formatting then parsing avoids the premature binary multiply
// rounding in (value * 10000).round(), and preserves signed zero.
fn rounded(value: f64, digits: usize) -> Result<f64, String> {
    if !value.is_finite() {
        return Err("Text geometry contains non-finite values".into());
    }
    format!("{value:.digits$}")
        .parse::<f64>()
        .map_err(|e| e.to_string())
}

// The legacy fingerprint hashes json.dumps defaults, including ASCII escapes,
// spaces and floating-point .0. Its paint dictionaries use sorted keys.
fn python_json(value: &Value) -> String {
    match value {
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(python_json)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(k, v)| format!(
                        "{}: {}",
                        python_json(&Value::String(k.clone())),
                        python_json(v)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        Value::String(value) => {
            let encoded = serde_json::to_string(value).expect("string JSON");
            let mut result = String::with_capacity(encoded.len());
            for ch in encoded.chars() {
                if ch >= '\u{7f}' {
                    for unit in ch.encode_utf16(&mut [0; 2]).iter() {
                        result.push_str(&format!("\\u{unit:04x}"));
                    }
                } else {
                    result.push(ch);
                }
            }
            result
        }
        Value::Number(number) => {
            let raw = number.to_string();
            // Python pads exponents to two digits and explicitly prints +.
            if let Some((mantissa, exp)) = raw.split_once('e') {
                if let Ok(exp) = exp.parse::<i32>() {
                    return format!("{mantissa}e{exp:+03}");
                }
            }
            raw
        }
        _ => value.to_string(),
    }
}

pub fn letters_fingerprint(children: &[Group]) -> Result<String, String> {
    let mut parts = Vec::with_capacity(children.len());
    for child in children {
        let mut cloud = child
            .mesh
            .vertices
            .iter()
            .map(|v| {
                // The original hashes QVector3D coordinates, including after IGZ load.
                Ok([
                    rounded(v.position.x as f32 as f64, 4)?,
                    rounded(v.position.y as f32 as f64, 4)?,
                    rounded(v.position.z as f32 as f64, 4)?,
                ])
            })
            .collect::<Result<Vec<[f64; 3]>, String>>()?;
        cloud.sort_by(|a, b| a.partial_cmp(b).expect("finite coordinates"));
        let mut paint = Vec::with_capacity(child.mesh.faces.len());
        for face in &child.mesh.faces {
            let color = if let Some(values) = face.attrs.get("color").filter(|v| !v.is_null()) {
                Value::Array(
                    values
                        .as_array()
                        .ok_or("Text face color is not an array")?
                        .iter()
                        .map(|v| {
                            Ok(json!(rounded(
                                v.as_f64().ok_or("Text face color is not numeric")?,
                                3
                            )?))
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                )
            } else {
                Value::Null
            };
            paint.push(python_json(
                &json!({"color":color,"mat":face.attrs.get("mat"),
                "texture":face.attrs.get("texture").is_some_and(|v|!v.is_null()),
                "back":face.attrs.get("back").is_some_and(|v|!v.is_null())}),
            ));
        }
        paint.sort();
        parts.push(json!([
            child.name,
            child.mesh.faces.len(),
            child.mesh.edges.len(),
            cloud,
            paint
        ]));
    }
    Ok(
        sha1_smol::Sha1::from(python_json(&Value::Array(parts)).as_bytes())
            .digest()
            .to_string()[..16]
            .into(),
    )
}
