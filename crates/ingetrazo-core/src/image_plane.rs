//! Display-only reference images; never topology or mesh export.
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePlane {
    #[serde(skip, default = "uuid::Uuid::new_v4")]
    pub id: uuid::Uuid,
    pub path: String,
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
    pub aspect: f64,
    pub name: String,
    pub opacity: f64,
    pub layer: Option<String>,
    pub locked: bool,
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
}
impl PartialEq for ImagePlane {
    fn eq(&self, o: &Self) -> bool {
        self.path == o.path
            && self.origin == o.origin
            && self.u == o.u
            && self.v == o.v
            && self.aspect == o.aspect
            && self.name == o.name
            && self.opacity == o.opacity
            && self.layer == o.layer
            && self.locked == o.locked
            && self.native_document == o.native_document
            && self.native_baseline == o.native_baseline
    }
}
impl ImagePlane {
    pub fn new(path: impl Into<String>, origin: DVec3, u: DVec3, v: DVec3) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            path: path.into(),
            origin,
            u,
            v,
            aspect: 1.,
            name: String::new(),
            opacity: 1.,
            layer: Some("Images".into()),
            locked: false,
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
        }
    }
    pub fn corners(&self) -> [DVec3; 4] {
        [
            self.origin,
            self.origin + self.u,
            self.origin + self.u + self.v,
            self.origin + self.v,
        ]
    }
    pub fn to_json(&self) -> serde_json::Value {
        let mut x = serde_json::json!({"texture":{"path":self.path},"origin":self.origin,"u":self.u,"v":self.v,"aspect":self.aspect});
        if !self.name.is_empty() {
            x["name"] = serde_json::json!(self.name);
        }
        if self.opacity != 1. {
            x["opacity"] = serde_json::json!(self.opacity);
        }
        match self.layer.as_deref() {
            None => x["layer"] = serde_json::Value::Null,
            Some("Images" | "") => {}
            Some(name) => x["layer"] = serde_json::json!(name),
        }
        if self.locked {
            x["locked"] = serde_json::json!(true);
        }
        x
    }
    pub fn from_json(raw: &serde_json::Value) -> Result<Self, String> {
        fn vector(raw: &serde_json::Value, k: &str) -> Result<DVec3, String> {
            let a = raw[k]
                .as_array()
                .ok_or_else(|| format!("Image plane {k} must be a vector"))?;
            if a.len() != 3 {
                return Err(format!("Image plane {k} must have three coordinates"));
            }
            let mut v = [0.; 3];
            for (i, x) in a.iter().enumerate() {
                v[i] = x
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("Image plane {k} is not finite"))?
                    as f32 as f64;
            }
            Ok(DVec3::from_array(v))
        }
        let path = raw
            .get("texture")
            .filter(|v| v.is_object())
            .map(|v| v.get("embed").or_else(|| v.get("path")))
            .unwrap_or_else(|| raw.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut p = Self::new(
            path,
            vector(raw, "origin")?,
            vector(raw, "u")?,
            vector(raw, "v")?,
        );
        p.aspect = if let Some(v) = raw.get("aspect") {
            crate::presentation::number(v).ok_or_else(|| "Invalid image aspect".to_string())?
        } else {
            1.
        };
        if p.aspect <= 0. {
            p.aspect = 1.;
        }
        p.name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .into();
        p.opacity = if let Some(v) = raw.get("opacity") {
            crate::presentation::number(v).ok_or_else(|| "Invalid image opacity".to_string())?
        } else {
            1.
        };
        if let Some(v) = raw.get("layer") {
            p.layer = if v.is_null() {
                None
            } else {
                v.as_str().map(str::to_string)
            };
        }
        p.locked = raw
            .get("locked")
            .map(crate::presentation::truth)
            .unwrap_or(false);
        p.native_document = raw.clone();
        p.native_baseline = p.to_json();
        Ok(p)
    }
}
/// Mirrors original command-held object lifetime without coordinate matching.
#[derive(Debug, Clone, Default)]
pub struct ImageObjects(pub Arc<Mutex<HashMap<uuid::Uuid, ImagePlane>>>);
impl PartialEq for ImageObjects {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl ImageObjects {
    pub fn publish(&self, planes: &[ImagePlane]) {
        let mut objects = self.0.lock().unwrap();
        for p in planes {
            objects.insert(p.id, p.clone());
        }
    }
    fn held(&self, p: &ImagePlane) -> ImagePlane {
        self.0
            .lock()
            .unwrap()
            .get(&p.id)
            .cloned()
            .unwrap_or_else(|| p.clone())
    }
}
#[derive(Debug, Clone, Default)]
pub struct ImageHistory(pub Option<ImageDelta>);
impl PartialEq for ImageHistory {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
#[derive(Debug, Clone)]
pub struct ImageDelta {
    pub add_command: bool,
    /// Original AddImagePlaneCommand refreshes _made_layer on every do.
    pub add_layer_created: Option<Arc<Mutex<bool>>>,
    pub added: Vec<(usize, ImagePlane)>,
    pub removed: Vec<(usize, ImagePlane)>,
    pub changed: Vec<(ImagePlane, ImagePlane)>,
}
impl ImageDelta {
    pub fn between(before: &[ImagePlane], after: &[ImagePlane]) -> Self {
        Self {
            add_layer_created: None,
            add_command: after.iter().any(|a| !before.iter().any(|b| b.id == a.id)),
            added: after
                .iter()
                .enumerate()
                .filter(|(_, p)| !before.iter().any(|b| b.id == p.id))
                .map(|(i, p)| (i, p.clone()))
                .collect(),
            removed: before
                .iter()
                .enumerate()
                .filter(|(_, p)| !after.iter().any(|b| b.id == p.id))
                .map(|(i, p)| (i, p.clone()))
                .collect(),
            changed: before
                .iter()
                .filter_map(|b| {
                    after
                        .iter()
                        .find(|a| a.id == b.id && *a != b)
                        .map(|a| (b.clone(), a.clone()))
                })
                .collect(),
        }
    }
    pub fn inverse(&self) -> Self {
        Self {
            add_command: self.add_command,
            add_layer_created: self.add_layer_created.clone(),
            added: self.removed.clone(),
            removed: self.added.clone(),
            changed: self
                .changed
                .iter()
                .map(|(b, a)| (a.clone(), b.clone()))
                .collect(),
        }
    }
    pub fn restore(&self, current: &[ImagePlane], objects: &ImageObjects) -> Vec<ImagePlane> {
        objects.publish(current);
        let mut target = current.to_vec();
        target.retain(|p| !self.added.iter().any(|(_, a)| a.id == p.id));
        for (i, p) in &self.removed {
            if !target.iter().any(|t| t.id == p.id) {
                target.insert((*i).min(target.len()), objects.held(p));
            }
        }
        for (before, after) in &self.changed {
            let mut p = objects.held(after);
            macro_rules! restore{($($field:ident),*)=>{$(if before.$field!=after.$field{p.$field=before.$field.clone();})*};}
            restore!(origin, u, v, opacity, path, aspect, name, layer, locked);
            objects.0.lock().unwrap().insert(p.id, p.clone());
            if let Some(t) = target.iter_mut().find(|t| t.id == p.id) {
                *t = p;
            }
        }
        objects.publish(&target);
        target
    }
}
