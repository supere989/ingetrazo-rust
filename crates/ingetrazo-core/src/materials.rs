// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Named materials, RGBA colors, and texture definitions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    pub name: String,
    pub color: [f32; 4],
    pub texture_path: Option<String>,
    pub texture_embed: Option<String>,
    pub roughness: f32,
    pub opacity: f32,
}

impl Material {
    pub fn new(name: impl Into<String>, color: [f32; 4]) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            color,
            texture_path: None,
            texture_embed: None,
            roughness: 0.5,
            opacity: color[3],
        }
    }

    pub fn with_texture(name: impl Into<String>, texture_path: impl Into<String>) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            color: [1.0, 1.0, 1.0, 1.0],
            texture_path: Some(texture_path.into()),
            texture_embed: None,
            roughness: 0.5,
            opacity: 1.0,
        }
    }
}

impl Material {
    /// Modeled archive fields. Optional source properties remain in the raw
    /// document when this baseline has not changed.
    pub fn archive_model(&self) -> serde_json::Value {
        serde_json::json!({"name":self.name,"color":self.color,"opacity":self.opacity,
            "roughness":self.roughness,"texture":{"path":self.texture_path,"embed":self.texture_embed}})
    }

    /// Admit a source material recipe without defaulting absent paint keys.
    pub fn from_recipe(recipe: serde_json::Value) -> Result<Self, String> {
        let object = recipe
            .as_object()
            .ok_or("Material recipe must be an object")?;
        let name = object
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|n| !n.trim().is_empty())
            .ok_or("Material name must not be empty")?;
        let mut material = Self::new(name, [1.; 4]);
        if let Some(color) = object.get("color").filter(|v| !v.is_null()) {
            let values = color
                .as_array()
                .filter(|c| c.len() == 3)
                .ok_or("Material color must have three channels")?;
            for (i, value) in values.iter().enumerate() {
                let value = value
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0. && *v <= 1.)
                    .ok_or("Material color must be finite and between zero and one")?;
                material.color[i] = value as f32;
            }
        }
        if let Some(opacity) = object.get("opacity").filter(|v| !v.is_null()) {
            let value = opacity
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 0. && *v <= 1.)
                .ok_or("Material opacity must be finite and between zero and one")?;
            material.opacity = value as f32;
            material.color[3] = material.opacity;
        }
        if let Some(texture) = object.get("texture").filter(|v| !v.is_null()) {
            let texture = texture
                .as_object()
                .ok_or("Material texture must be an object")?;
            for key in ["path", "embed"] {
                if texture.get(key).is_some_and(|v| !v.is_string()) {
                    return Err(format!("Material texture {key} must be a string"));
                }
            }
            for key in ["sw", "sh"] {
                if texture
                    .get(key)
                    .is_some_and(|v| v.as_f64().is_none_or(|v| !v.is_finite() || v <= 0.))
                {
                    return Err(format!("Material tile {key} must be positive and finite"));
                }
            }
            if texture
                .get("rot")
                .is_some_and(|v| v.as_f64().is_none_or(|v| !v.is_finite()))
            {
                return Err("Material rotation must be finite".into());
            }
            material.texture_path = texture
                .get("path")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            material.texture_embed = texture
                .get("embed")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
        }
        material.native_baseline = material.archive_model();
        material.native_document = recipe;
        Ok(material)
    }

    pub fn registry_recipe(&self) -> serde_json::Value {
        let mut result = self
            .native_document
            .as_object()
            .cloned()
            .unwrap_or_default();
        result.insert("name".into(), self.name.clone().into());
        let stamp = self.face_recipe();
        for key in ["color", "texture", "opacity"] {
            if let Some(v) = stamp.get(key) {
                result.insert(key.into(), v.clone());
            } else {
                result.remove(key);
            }
        }
        result.into()
    }

    /// The source paint stamp excludes finish and opaque registry properties.
    /// Preserve the original decimal values while their modeled fields match.
    pub fn face_recipe(&self) -> serde_json::Value {
        let current = self.archive_model();
        let mut result = serde_json::Map::new();
        result.insert("mat".into(), self.name.clone().into());
        for key in ["color", "texture", "opacity"] {
            if self.native_document.is_object() && current.get(key) == self.native_baseline.get(key)
            {
                if let Some(value) = self.native_document.get(key).filter(|v| !v.is_null()) {
                    result.insert(key.into(), value.clone());
                }
            } else if key == "color" {
                result.insert(key.into(), serde_json::json!(&self.color[..3]));
            } else if key == "opacity" {
                if self.opacity != 1. {
                    result.insert(key.into(), serde_json::json!(self.opacity));
                }
            } else if self.texture_path.is_some() || self.texture_embed.is_some() {
                let mut texture = self
                    .native_document
                    .get("texture")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                for (key, value) in [("path", &self.texture_path), ("embed", &self.texture_embed)] {
                    if let Some(value) = value {
                        texture.insert(key.into(), value.clone().into());
                    } else {
                        texture.remove(key);
                    }
                }
                result.insert("texture".into(), texture.into());
            }
        }
        result.into()
    }
}

/// Cold material command payload; topology and presentation are not owned.
#[derive(Debug, Clone, PartialEq)]
pub struct RestampHistory {
    pub name: String,
    pub registry: Vec<(Vec<uuid::Uuid>, String, Option<Material>, Option<Material>)>,
    pub faces: Vec<FacePaintPatch>,
    pub groups: Vec<GroupPaintPatch>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct FacePaintPatch {
    pub view: Vec<uuid::Uuid>,
    pub context: Vec<uuid::Uuid>,
    pub vertices: Vec<uuid::Uuid>,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}
#[derive(Debug, Clone, PartialEq)]
pub struct GroupPaintPatch {
    pub path: Vec<uuid::Uuid>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}
impl RestampHistory {
    /// Fold chronologically newer inverse patches into an older inverse. Keep
    /// the oldest destination and the newest required current paint.
    pub fn absorb_inverse(&mut self, newer: &Self) {
        for (c, n, b, a) in &newer.registry {
            if let Some(old) = self
                .registry
                .iter_mut()
                .find(|(oc, on, _, _)| oc == c && on == n)
            {
                old.2 = b.clone();
            } else {
                self.registry
                    .push((c.clone(), n.clone(), b.clone(), a.clone()));
            }
        }
        for p in &newer.faces {
            if let Some(old) = self.faces.iter_mut().find(|old| {
                old.view == p.view && old.context == p.context && old.vertices == p.vertices
            }) {
                old.before = p.before.clone();
            } else {
                self.faces.push(p.clone());
            }
        }
        for p in &newer.groups {
            if let Some(old) = self.groups.iter_mut().find(|old| old.path == p.path) {
                old.before = p.before.clone();
            } else {
                self.groups.push(p.clone());
            }
        }
    }
    pub fn inverse(&self) -> Self {
        Self {
            name: self.name.clone(),
            registry: self
                .registry
                .iter()
                .map(|(c, n, a, b)| (c.clone(), n.clone(), b.clone(), a.clone()))
                .collect(),
            faces: self
                .faces
                .iter()
                .map(|p| FacePaintPatch {
                    view: p.view.clone(),
                    context: p.context.clone(),
                    vertices: p.vertices.clone(),
                    before: p.after.clone(),
                    after: p.before.clone(),
                })
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|p| GroupPaintPatch {
                    path: p.path.clone(),
                    before: p.after.clone(),
                    after: p.before.clone(),
                })
                .collect(),
        }
    }
}
