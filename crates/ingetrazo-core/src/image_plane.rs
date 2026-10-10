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
    pub fn metadata_count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
    pub fn retain_ids(&self, ids: &std::collections::HashSet<uuid::Uuid>) -> usize {
        let mut objects = self.0.lock().unwrap();
        let before = objects.len();
        objects.retain(|id, _| ids.contains(id));
        before - objects.len()
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
    pub frame_command: Option<(ImageFrameCommand, bool)>,
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
            frame_command: None,
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
            frame_command: self
                .frame_command
                .as_ref()
                .map(|(c, redo)| (c.clone(), !redo)),
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
        if let Some((command, redo)) = &self.frame_command {
            command.apply(&mut target, objects, *redo);
            objects.publish(&target);
            return target;
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

/// Source image commands hold object identity, including temporarily detached
/// objects. Relative operations compose on do/undo; Transform captures its
/// complete old frame afresh on every do.
#[derive(Debug, Clone)]
pub struct ImageFrameCommand {
    pub images: Vec<ImagePlane>,
    pub operation: ImageFrameOperation,
}
#[derive(Debug, Clone)]
pub enum ImageFrameOperation {
    Move(DVec3),
    Rotate {
        centre: DVec3,
        axis: DVec3,
        degrees: f64,
    },
    Scale {
        anchor: DVec3,
        factor: DVec3,
        axes: Option<[DVec3; 3]>,
    },
    Transform {
        new: [Option<DVec3>; 3],
        old: Arc<Mutex<Option<[DVec3; 3]>>>,
    },
}
impl ImageFrameCommand {
    pub fn apply(&self, live: &mut [ImagePlane], objects: &ImageObjects, redo: bool) {
        for seed in &self.images {
            let mut image = objects.held(seed);
            match &self.operation {
                ImageFrameOperation::Move(delta) => {
                    image.origin = (image.origin.as_vec3()
                        + if redo {
                            delta.as_vec3()
                        } else {
                            -delta.as_vec3()
                        })
                    .as_dvec3()
                }
                ImageFrameOperation::Rotate {
                    centre,
                    axis,
                    degrees,
                } => {
                    let matrix = ImageFrameMatrix::rotate(
                        *centre,
                        *axis,
                        if redo { *degrees } else { -degrees },
                    );
                    image.origin = matrix.map(image.origin, true);
                    image.u = matrix.map(image.u, false);
                    image.v = matrix.map(image.v, false);
                }
                ImageFrameOperation::Scale {
                    anchor,
                    factor,
                    axes,
                } => {
                    let f = if redo {
                        *factor
                    } else {
                        factor.map(|x| if x.abs() > 1e-12 { 1. / x } else { 1. })
                    };
                    let matrix = ImageFrameMatrix::scale(*anchor, f, *axes);
                    image.origin = matrix.map(image.origin, true);
                    image.u = matrix.map(image.u, false);
                    image.v = matrix.map(image.v, false);
                }
                ImageFrameOperation::Transform { new, old } => {
                    if redo {
                        *old.lock().unwrap() = Some([image.origin, image.u, image.v]);
                        if let Some(x) = new[0] {
                            image.origin = x.as_vec3().as_dvec3();
                        }
                        if let Some(x) = new[1] {
                            image.u = x.as_vec3().as_dvec3();
                        }
                        if let Some(x) = new[2] {
                            image.v = x.as_vec3().as_dvec3();
                        }
                    } else if let Some([o, u, v]) = *old.lock().unwrap() {
                        image.origin = o;
                        image.u = u;
                        image.v = v;
                    }
                }
            }
            objects.0.lock().unwrap().insert(image.id, image.clone());
            if let Some(p) = live.iter_mut().find(|p| p.id == image.id) {
                *p = image;
            }
        }
    }
}
// f32 scalar arithmetic mirrors QVector3D/QMatrix4x4; world placement images
// remain global and are never inverse-fit to a component local frame.
struct ImageFrameMatrix {
    cols: [[f32; 3]; 3],
    translation: [f32; 3],
    kind: u8,
}
impl ImageFrameMatrix {
    fn map(&self, point: DVec3, position: bool) -> DVec3 {
        let p = point.as_vec3().to_array();
        let mut v = [0.; 3];
        for r in 0..3 {
            v[r] = if self.kind == 0 {
                p[r] * self.cols[r][r]
            } else if position && self.kind == 1 && r < 2 {
                p[0] * self.cols[0][r] + p[1] * self.cols[1][r]
            } else if position && self.kind == 1 {
                p[2] * self.cols[2][r]
            } else {
                p[0] * self.cols[0][r] + p[1] * self.cols[1][r] + p[2] * self.cols[2][r]
            };
            if position {
                v[r] += self.translation[r];
            }
        }
        glam::Vec3::from_array(v).as_dvec3()
    }
    fn centered(cols: [[f32; 3]; 3], centre: DVec3, kind: u8) -> Self {
        let c = centre.as_vec3().to_array();
        let mut translation = c;
        for r in 0..3 {
            translation[r] += if kind == 0 {
                -c[r] * cols[r][r]
            } else if kind == 1 && r < 2 {
                -c[0] * cols[0][r] - c[1] * cols[1][r]
            } else if kind == 1 {
                -c[2] * cols[2][r]
            } else {
                -c[0] * cols[0][r] - c[1] * cols[1][r] - c[2] * cols[2][r]
            };
        }
        Self {
            cols,
            translation,
            kind,
        }
    }
    fn rotate(centre: DVec3, axis: DVec3, degrees: f64) -> Self {
        let angle = degrees as f32;
        let (s, c) = match angle {
            90. | -270. => (1., 0.),
            -90. | 270. => (-1., 0.),
            180. | -180. => (0., -1.),
            _ => {
                let a = angle * (std::f64::consts::PI / 180.) as f32;
                (a.sin(), a.cos())
            }
        };
        let a = axis.as_vec3();
        let len = a.length();
        let a = if (len - 1.).abs() <= 1e-5 {
            a
        } else if len.abs() <= 1e-5 {
            glam::Vec3::ZERO
        } else {
            a / len
        };
        let [mut x, mut y, mut z] = a.to_array();
        let (cols, kind) = if x == 0. && y == 0. && z != 0. {
            let s = if z < 0. { -s } else { s };
            ([[c, s, 0.], [-s, c, 0.], [0., 0., 1.]], 1)
        } else if x == 0. && z == 0. && y != 0. {
            let s = if y < 0. { -s } else { s };
            ([[c, 0., -s], [0., 1., 0.], [s, 0., c]], 2)
        } else if y == 0. && z == 0. && x != 0. {
            let s = if x < 0. { -s } else { s };
            ([[1., 0., 0.], [0., c, s], [0., -s, c]], 2)
        } else {
            let len = (x as f64) * (x as f64) + (y as f64) * (y as f64) + (z as f64) * (z as f64);
            if (len - 1.).abs() > 1e-12 * len.min(1.) && len.abs() > 1e-12 {
                let l = len.sqrt();
                x = (x as f64 / l) as f32;
                y = (y as f64 / l) as f32;
                z = (z as f64 / l) as f32;
            }
            let k = 1. - c;
            (
                [
                    [x * x * k + c, y * x * k + z * s, x * z * k - y * s],
                    [x * y * k - z * s, y * y * k + c, y * z * k + x * s],
                    [x * z * k + y * s, y * z * k - x * s, z * z * k + c],
                ],
                2,
            )
        };
        Self::centered(cols, centre, kind)
    }
    fn scale(anchor: DVec3, factor: DVec3, axes: Option<[DVec3; 3]>) -> Self {
        let f = factor.as_vec3().to_array();
        if let Some(axes) = axes {
            let a = axes.map(|a| a.as_vec3().to_array());
            let mut cols = [[0.; 3]; 3];
            for c in 0..3 {
                for r in 0..3 {
                    cols[c][r] = (a[0][r] * f[0]) * a[0][c]
                        + (a[1][r] * f[1]) * a[1][c]
                        + (a[2][r] * f[2]) * a[2][c];
                }
            }
            Self::centered(cols, anchor, 2)
        } else {
            Self::centered([[f[0], 0., 0.], [0., f[1], 0.], [0., 0., f[2]]], anchor, 0)
        }
    }
}
