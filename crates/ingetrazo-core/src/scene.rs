// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Scene container backed by a shared-vertex Mesh.
//!
//! Master document state coordinating geometry, groups, layers, materials,
//! guides, dimensions, selection, and viewport camera.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::camera::OrbitCamera;
use crate::dimensions::Dimension;
use crate::groups::Group;
use crate::guides::Guide;
use crate::layers::Layer;
use crate::materials::Material;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::units::LengthUnit;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelectionItem {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
    Group(usize),
    GeoPoint(usize),
    GeoPath(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    #[serde(default)]
    pub component_definitions: HashMap<uuid::Uuid, crate::components::ComponentDefinition>,
    /// Original native payload and modeled snapshot. Unchanged fields retain their
    /// source representation, including future/extension metadata.
    #[serde(default)]
    pub native_document: serde_json::Value,
    #[serde(default)]
    pub native_baseline: serde_json::Value,

    pub mesh: Mesh,
    pub selection: HashSet<SelectionItem>,
    /// View-only insertion order for directional operations such as cutter/target.
    #[serde(default)]
    pub selection_order: Vec<SelectionItem>,
    pub version: u64,
    pub view_version: u64,
    pub groups: Vec<Group>,
    pub dimensions: Vec<Dimension>,
    pub guides: Vec<Guide>,
    pub layers: Vec<Layer>,
    pub materials: HashMap<String, Material>,
    pub saved_views: Vec<crate::saved_views::SavedView>,
    pub camera: OrbitCamera,
    pub unit: LengthUnit,
    pub embedded_textures: HashMap<String, Vec<u8>>,
    pub text_labels: Vec<crate::textlabel::TextLabel>,
    pub section_planes: Vec<crate::section::SectionPlane>,
    pub show_section_cuts: bool,
    pub show_section_planes: bool,
    pub show_section_fill: bool,
    pub geo_datum: Option<crate::georef::SceneDatum>,
    pub geo_points: Vec<crate::georef::GeoPoint>,
    pub geo_paths: Vec<crate::georef::GeoPath>,
    pub terrain: Option<crate::georef::TerrainObject>,
    pub sheets: Vec<crate::composition::Sheet>,
    pub active_sheet_index: Option<usize>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Self {
        let mut layers = Vec::new();
        layers.push(Layer::default());

        Self {
            component_definitions: HashMap::new(),
            native_document: serde_json::Value::Null,
            native_baseline: serde_json::Value::Null,
            mesh: Mesh::new(),
            selection: HashSet::new(),
            selection_order: Vec::new(),
            version: 0,
            view_version: 0,
            groups: Vec::new(),
            dimensions: Vec::new(),
            guides: Vec::new(),
            layers,
            materials: HashMap::new(),
            saved_views: Vec::new(),
            camera: OrbitCamera::new(),
            unit: LengthUnit::Meters,
            embedded_textures: HashMap::new(),
            text_labels: Vec::new(),
            section_planes: Vec::new(),
            show_section_cuts: true,
            show_section_planes: true,
            show_section_fill: true,
            geo_datum: None,
            geo_points: Vec::new(),
            geo_paths: Vec::new(),
            terrain: None,
            sheets: Vec::new(),
            active_sheet_index: None,
        }
    }

    pub fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    pub fn bump_view_version(&mut self) {
        self.view_version = self.view_version.wrapping_add(1);
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_order.clear();
        self.bump_view_version();
    }

    pub fn select(&mut self, item: SelectionItem) {
        if self.selection.insert(item.clone()) {
            self.selection_order.push(item);
        }
        self.bump_view_version();
    }

    pub fn deselect(&mut self, item: &SelectionItem) {
        self.selection.remove(item);
        self.selection_order.retain(|selected| selected != item);
        self.bump_view_version();
    }

    pub fn selected_groups_in_order(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for item in &self.selection_order {
            if self.selection.contains(item) {
                if let SelectionItem::Group(index) = item {
                    if !result.contains(index) { result.push(*index); }
                }
            }
        }
        // Legacy deserialization or callers inserting directly in the public set.
        let mut remainder: Vec<_> = self.selection.iter().filter_map(|item| {
            if let SelectionItem::Group(index) = item {
                if !result.contains(index) { Some(*index) } else { None }
            } else { None }
        }).collect();
        remainder.sort_unstable();
        result.extend(remainder);
        result
    }

    pub fn is_selected(&self, item: &SelectionItem) -> bool {
        self.selection.contains(item)
    }

    pub fn is_layer_visible(&self, layer_name: Option<&str>) -> bool {
        match layer_name {
            Some(name) => {
                for l in &self.layers {
                    if l.name == name {
                        return l.visible;
                    }
                }
                true
            }
            None => {
                for l in &self.layers {
                    if l.name == crate::layers::DEFAULT_LAYER || l.name == "Capa 0" || l.name == "Layer0" || l.name == "Layer 0" {
                        return l.visible;
                    }
                }
                true
            }
        }
    }

    pub fn purge_unused_layers(&mut self) -> usize {
        let mut used_layers = std::collections::HashSet::new();
        for f in &self.mesh.faces {
            if let Some(l) = f.layer.as_deref().or_else(|| f.attrs.get("layer").and_then(|v| v.as_str())) {
                used_layers.insert(l.to_string());
            }
        }
        for e in &self.mesh.edges {
            if let Some(l) = &e.layer {
                used_layers.insert(l.clone());
            }
        }
        for g in &self.groups {
            if let Some(l) = &g.layer {
                used_layers.insert(l.clone());
            }
        }
        let before_count = self.layers.len();
        self.layers.retain(|l| {
            l.name == crate::layers::DEFAULT_LAYER
                || l.name == "Capa 0"
                || l.name == "Layer0"
                || l.name == "Layer 0"
                || used_layers.contains(&l.name)
        });
        if self.layers.is_empty() {
            self.layers.push(crate::layers::Layer::default());
        }
        self.bump_version();
        before_count.saturating_sub(self.layers.len())
    }

    pub fn assign_selection_to_layer(&mut self, layer_name: &str) -> usize {
        let mut count = 0;
        for item in &self.selection {
            match item {
                SelectionItem::Face(f_id) => {
                    if *f_id < self.mesh.faces.len() {
                        self.mesh.faces[*f_id].layer = Some(layer_name.to_string());
                        self.mesh.faces[*f_id].attrs.insert("layer".to_string(), serde_json::Value::String(layer_name.to_string()));
                        count += 1;
                    }
                }
                SelectionItem::Edge(e_id) => {
                    if *e_id < self.mesh.edges.len() {
                        self.mesh.edges[*e_id].layer = Some(layer_name.to_string());
                        count += 1;
                    }
                }
                SelectionItem::Group(g_id) => {
                    if *g_id < self.groups.len() {
                        self.groups[*g_id].layer = Some(layer_name.to_string());
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        self.bump_version();
        count
    }
}

