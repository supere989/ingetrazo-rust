// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Encapsulated Groups and Component Instances with local coordinate transforms.

use glam::DMat4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mesh::Mesh;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// Mesh prototype identity, independent of component flags and child placement lists.
    /// Equal meshes never acquire the same token by geometry deduplication.
    #[serde(default)]
    pub mesh_share: Option<Uuid>,
    /// Explicit linked-component identity.
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
            mesh_share: None,
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
            mesh_share: None,
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
    /// The persisted placement identity, including legacy non-UUID names.
    pub fn native_uid(&self)->String {
        let current=self.id.simple().to_string();
        if self.native_baseline.get("uid").map(|v|v==&serde_json::json!(current)).unwrap_or(true){
            if let Some(uid)=self.native_document.get("uid").and_then(|v|v.as_str()){return uid.to_owned();}
        }
        current
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
