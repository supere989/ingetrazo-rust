// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Encapsulated Groups and Component Instances with local coordinate transforms.

use glam::DMat4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mesh::Mesh;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Original native payload and modeled snapshot. Unchanged fields retain their
    /// source representation, including future/extension metadata.
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,

    pub id: Uuid,
    pub name: String,
    pub mesh: Mesh,
    pub transform: DMat4,
    pub layer: Option<String>,
    pub children: Vec<Group>,
    pub ifc_tag: Option<serde_json::Value>,
    pub billboard: bool,
    pub billboard_texture: Option<String>,
    #[serde(default)]
    pub hidden: bool,
}

impl Group {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            id: Uuid::new_v4(),
            name: name.into(),
            mesh: Mesh::new(),
            transform: DMat4::IDENTITY,
            layer: None,
            children: Vec::new(),
            ifc_tag: None,
            billboard: false,
            billboard_texture: None,
            hidden: false,
        }
    }

    pub fn with_mesh(name: impl Into<String>, mesh: Mesh) -> Self {
        Self {
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            id: Uuid::new_v4(),
            name: name.into(),
            mesh,
            transform: DMat4::IDENTITY,
            layer: None,
            children: Vec::new(),
            ifc_tag: None,
            billboard: false,
            billboard_texture: None,
            hidden: false,
        }
    }
}
