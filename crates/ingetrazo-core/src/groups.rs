// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Encapsulated Groups and Component Instances with local coordinate transforms.

use glam::DMat4;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::mesh::Mesh;

// Shared source automatic-name owner: loaded names reserve the same counter.
static SOURCE_GROUP_NAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub fn next_source_group_name()->Result<String,&'static str> {
    SOURCE_GROUP_NAME.fetch_update(std::sync::atomic::Ordering::Relaxed,std::sync::atomic::Ordering::Relaxed,|n|n.checked_add(1)).map(|n|format!("Group {n}")).map_err(|_|"Automatic group name counter exhausted; document unchanged")
}
pub fn reserve_source_group_names(groups:&[Group]) {
    fn maximum(groups:&[Group])->u64 {
        groups.iter().map(|group|{
            let own=group.name.strip_prefix("Group ").filter(|s|!s.is_empty()&&s.chars().all(|c|c.is_ascii_digit())).and_then(|s|s.parse::<u64>().ok()).unwrap_or(0);
            own.max(maximum(&group.children))
        }).max().unwrap_or(0)
    }
    SOURCE_GROUP_NAME.fetch_max(maximum(groups).saturating_add(1),std::sync::atomic::Ordering::Relaxed);
}

/// Keep source spellings distinct; only canonical native UUID32 is its own key.
pub fn source_uid_key(id: &str) -> Uuid {
    match Uuid::parse_str(id) {
        Ok(key) if key.simple().to_string() == id => key,
        _ => Uuid::new_v5(&Uuid::NAMESPACE_URL, id.as_bytes()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// Semantic document equality deliberately excludes the transient command token.
// Exhaustive destructuring makes adding a modeled field require updating this policy.
impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        let Self { history_id: _, mesh_share, component_definition, definition_member, native_document, native_baseline, id, name, mesh, transform, layer, children, ifc_tag, billboard, billboard_texture, hidden } = self;
        mesh_share == &other.mesh_share
            && component_definition == &other.component_definition
            && definition_member == &other.definition_member
            && native_document == &other.native_document
            && native_baseline == &other.native_baseline
            && id == &other.id
            && name == &other.name
            && mesh == &other.mesh
            && transform == &other.transform
            && layer == &other.layer
            && children == &other.children
            && ifc_tag == &other.ifc_tag
            && billboard == &other.billboard
            && billboard_texture == &other.billboard_texture
            && hidden == &other.hidden
    }
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

#[cfg(test)]
mod history_identity_tests {
    use super::*;
    #[test]
    fn definition_refresh_preserves_each_existing_placement_token() {
        let mut scene = crate::scene::Scene::new();
        let mut parent = Group::new("parent");
        parent.children.push(Group::new("child"));
        let id = parent.id;
        scene.groups.push(parent);
        let definition = scene.make_component(id).unwrap();
        scene.groups.push(scene.groups[0].duplicate_instance());
        let tokens: Vec<_> = scene.groups.iter().map(|g|g.children[0].history_id).collect();
        assert_ne!(tokens[0],tokens[1]);
        let replacement=scene.component_definitions[&definition].clone();
        scene.replace_component_definition(definition,replacement).unwrap();
        assert_eq!(tokens,scene.groups.iter().map(|g|g.children[0].history_id).collect::<Vec<_>>());
    }
    #[test]
    fn command_tokens_do_not_change_serialized_document_equality() {
        let mut group = Group::new("authored");
        group.children.push(Group::new("child"));
        let restored: Group = serde_json::from_value(serde_json::to_value(&group).unwrap()).unwrap();
        assert_eq!(group, restored);
        assert_ne!(group.history_id, restored.history_id);
        assert_ne!(group.children[0].history_id, restored.children[0].history_id);
        let snapshot = group.clone();
        assert_eq!(group.history_id, snapshot.history_id);
        let copy = group.duplicate_instance();
        assert_ne!(group.history_id, copy.history_id);
        assert_ne!(group.children[0].history_id, copy.children[0].history_id);
        let mut scene = crate::scene::Scene::new();
        scene.groups.push(group);
        let restored: crate::scene::Scene = serde_json::from_value(serde_json::to_value(&scene).unwrap()).unwrap();
        assert_eq!(scene, restored);
    }
}
