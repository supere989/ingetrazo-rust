// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Saved views / Scenes: named snapshots of camera and layer visibility.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::camera::OrbitCamera;
use crate::layers::Layer;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,
    pub name: String,
    #[serde(default)]
    pub target: DVec3,
    #[serde(default = "default_distance")]
    pub distance: f64,
    #[serde(default = "default_yaw")]
    pub yaw: f64,
    #[serde(default = "default_pitch")]
    pub pitch: f64,
    #[serde(default = "default_fov")]
    pub fov_deg: f64,
    #[serde(default = "default_true", alias = "perspective")]
    pub perspective: bool,
    #[serde(default, alias = "parallel")]
    pub parallel: bool,
    #[serde(default)]
    pub hidden_layers: Vec<String>,
}

fn default_distance() -> f64 { 20.0 }
fn default_yaw() -> f64 { (-45.0_f64).to_radians() }
fn default_pitch() -> f64 { 30.0_f64.to_radians() }
fn default_fov() -> f64 { 45.0 }
fn default_true() -> bool { true }

impl SavedView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            target: DVec3::ZERO,
            distance: default_distance(),
            yaw: default_yaw(),
            pitch: default_pitch(),
            fov_deg: default_fov(),
            perspective: true,
            parallel: false,
            hidden_layers: Vec::new(),
        }
    }

    pub fn capture(name: impl Into<String>, camera: &OrbitCamera, layers: &[Layer]) -> Self {
        let hidden = layers.iter()
            .filter(|l| !l.visible)
            .map(|l| l.name.clone())
            .collect();

        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            name: name.into(),
            target: camera.target,
            distance: camera.distance,
            yaw: camera.yaw,
            pitch: camera.pitch,
            fov_deg: camera.fov_deg,
            perspective: camera.perspective,
            parallel: !camera.perspective,
            hidden_layers: hidden,
        }
    }

    pub fn apply(&self, camera: &mut OrbitCamera, layers: &mut [Layer]) {
        camera.target = self.target;
        camera.distance = self.distance;
        camera.yaw = self.yaw;
        camera.pitch = self.pitch;
        camera.fov_deg = self.fov_deg;
        camera.perspective = if self.parallel { false } else { self.perspective };

        for layer in layers.iter_mut() {
            layer.visible = !self.hidden_layers.contains(&layer.name);
        }
    }
}
