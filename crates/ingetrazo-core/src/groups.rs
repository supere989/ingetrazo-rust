// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Encapsulated Groups and Component Instances with local coordinate transforms.

use glam::DMat4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mesh::Mesh;

/// Keep source spellings distinct; only canonical native UUID32 is its own key.
pub fn source_uid_key(id: &str) -> Uuid {
    match Uuid::parse_str(id) {
        Ok(key) if key.simple().to_string() == id => key,
        _ => Uuid::new_v5(&Uuid::NAMESPACE_URL, id.as_bytes()),
    }
}

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
    /// Placement-object identity for native command ownership. Authored UIDs
    /// can repeat; snapshots retain this token, new placement copies do not.
    #[serde(skip, default = "Uuid::new_v4")]
    pub history_id: Uuid,
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
            history_id: Uuid::new_v4(),
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
            history_id: Uuid::new_v4(),
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
    /// Original stored axes live in the Mesh coordinate space; retain their
    /// affine matrix (including scale/reflection), not a normalized basis.
    pub fn axes_matrix(&self)->Option<DMat4> {
        let values=self.native_document.get("axes")?.as_array()?;
        if values.len()!=16 {return None;}
        let mut array=[0.;16];for (index,value) in values.iter().enumerate(){array[index]=value.as_f64()?;}
        Some(DMat4::from_cols_array(&array))
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
            group.history_id=Uuid::new_v4();
            for child in &mut group.children { reset(child); }
        }
        reset(&mut copy);
        copy
    }

}
