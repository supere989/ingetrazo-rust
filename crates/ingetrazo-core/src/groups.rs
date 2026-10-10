// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Encapsulated Groups and Component Instances with local coordinate transforms.

use glam::DMat4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mesh::Mesh;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Explicit linked-component identity; ordinary serialized prototypes remain
    /// independent groups even when their initial mesh data is deduplicated.
    #[serde(default)]
    pub component_definition: Option<Uuid>,
    /// Identity of this child slot inside its parent component definition.
    #[serde(default)]
    pub definition_member: Option<Uuid>,
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
            component_definition: None,
            definition_member: None,
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
            component_definition: None,
            definition_member: None,
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
    /// Copies placement identity while retaining component definition links.
    pub fn duplicate_instance(&self) -> Self {
        let mut copy=self.clone();
        fn reset(group:&mut Group) {
            group.definition_member=group.definition_member.or(Some(group.id));
            group.id=Uuid::new_v4();
            for child in &mut group.children { reset(child); }
        }
        reset(&mut copy);
        copy
    }

}
