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
    Guide(usize),
    Dimension(usize),
    TextLabel(usize),
    Edge(EdgeId),
    Face(FaceId),
    /// Face selected from a parked mesh while another immutable edit target is active.
    ContextFace { context: Vec<uuid::Uuid>, face: FaceId, vertices: Vec<uuid::Uuid> },
    ContextGroup { context: Vec<uuid::Uuid>, group: uuid::Uuid },
    Group(usize),
    GeoPoint(usize),
    GeoPath(usize),
    SectionPlane(usize),
    ImagePlane(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// Runtime BIM activation. Draw plans allocate identities at commit; never persisted.
    #[serde(skip)]pub active_ifc:Option<serde_json::Value>,
    /// Original active edit allocates from the root loose mesh, not the exposed group mesh.
    #[serde(skip)]pub bim_draw_id:Option<Result<i64,String>>,
    #[serde(default)]pub image_planes:crate::image_plane::ImagePlaneList,
    #[serde(skip)]pub image_objects:crate::image_plane::ImageObjects,
    #[serde(skip)]pub history_images:crate::image_plane::ImageHistory,
    #[serde(default)]
    pub display_style:crate::presentation::DocumentStyle,
    #[serde(default)]
    pub shadows:crate::presentation::ShadowSettings,
    #[serde(skip)]
    pub dimension_frame:glam::DMat4,
    /// Original world-space context frame; runtime only, independent of the
    /// active mesh coordinate map in dimension_frame.
    #[serde(skip)]
    pub drawing_frame: Option<glam::DMat4>,
    /// Runtime command mesh identity. Empty targets the document; each stable
    /// placement ID identifies an active nested edit view. Never persisted.
    #[serde(skip)]
    pub history_context: Vec<uuid::Uuid>,
    /// Command-specific prototype restore target, carried only by history snapshots.
    #[serde(skip)] pub history_annotation_delete:bool,
    #[serde(skip)] pub history_annotation_move:bool,
    #[serde(skip)]
    pub history_shareback: Option<uuid::Uuid>,
    #[serde(skip)]pub history_axes:Option<crate::change_axes::Command>,
    /// Direct BIM metadata target patches, carried only by history snapshots.
    #[serde(skip)]
    pub history_bim:Option<crate::bim::MetadataHistory>,
    #[serde(skip)]pub history_material:Option<crate::materials::RestampHistory>,
    /// Opt-in interpreter command owner and VM stack depth; history snapshots only.
    #[serde(skip)]
    pub history_python: Option<(uuid::Uuid, usize)>,
    #[serde(skip)]
    pub history_views: crate::saved_views::ViewHistory,
    #[serde(skip)]
    pub history_group_visibility: Option<crate::saved_views::GroupVisibilityCommand>,
    #[serde(skip)]
    pub history_terrain:crate::georef::TerrainHistory,
    #[serde(skip)]
    pub history_layers:crate::layers::LayerHistory,
    #[serde(default)]
    pub component_definitions: HashMap<uuid::Uuid, crate::components::ComponentDefinition>,
    /// Authoritative shared Mesh prototypes; placement topology is independent.
    #[serde(default)]
    pub mesh_shares: HashMap<uuid::Uuid, Mesh>,
    /// Original native payload and modeled snapshot. Unchanged fields retain their
    /// source representation, including future/extension metadata.
    #[serde(default)]
    pub native_document: serde_json::Value,
    /// Original extension payloads, independently editable and preserved.
    #[serde(default)]
    pub plugin_data: serde_json::Value,
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
    #[serde(default)] pub show_hidden_objects: bool,
    #[serde(default)] pub show_hidden_geometry: bool,
    pub geo_datum: Option<crate::georef::SceneDatum>,
    pub geo_points: Vec<crate::georef::GeoPoint>,
    pub geo_paths: Vec<crate::georef::GeoPath>,
    #[serde(default)]
    pub tile_layer: Option<crate::georef::TileCapture>,
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
            active_ifc:None,bim_draw_id:None,
            image_planes:Default::default(),image_objects:Default::default(),history_images:Default::default(),
            dimension_frame:glam::DMat4::IDENTITY,
            drawing_frame:None,
            history_context:vec![],
            display_style:Default::default(),shadows:Default::default(),history_annotation_delete:false,history_annotation_move:false,history_shareback:None,history_axes:None,history_bim:None,history_material:None,history_python:None,history_views:Default::default(),history_group_visibility:None,history_terrain:Default::default(),history_layers:Default::default(),
            component_definitions: HashMap::new(),
            mesh_shares: HashMap::new(),
            native_document: serde_json::Value::Null,
            plugin_data: serde_json::json!({}),
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
            show_hidden_objects: false,
            show_hidden_geometry: false,
            geo_datum: None,
            geo_points: Vec::new(),
            geo_paths: Vec::new(),
            tile_layer: None,
            terrain: None,
            sheets: Vec::new(),
            active_sheet_index: None,
        }
    }

    pub fn drawing_axes(&self) -> crate::drawing_axes::DrawingAxes {
        crate::drawing_axes::DrawingAxes::from_frame(self.drawing_frame)
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
        let name = crate::layers::entity_layer_name(layer_name);
        self.layers.iter().find(|layer| layer.name == name).map_or(true, |layer| layer.visible)
    }

    /// Locking does not change visibility. Ordinary picking also rejects locks;
    /// unknown layer names remain visible and unlocked, as in the source.
    pub fn is_layer_selectable(&self, layer_name: Option<&str>) -> bool {
        let name = crate::layers::entity_layer_name(layer_name);
        self.layers.iter().find(|layer| layer.name == name).is_none_or(|layer| layer.visible && !layer.locked)
    }

    pub fn used_layer_names(&self)->std::collections::HashSet<String>{
        fn mesh(mesh:&Mesh,used:&mut HashSet<String>){for f in &mesh.faces{if let Some(name)=f.layer.as_deref().or_else(||f.attrs.get("layer").and_then(|v|v.as_str())){used.insert(name.into());}}for e in &mesh.edges{if let Some(name)=&e.layer{used.insert(name.clone());}}}
        fn groups(gs:&[Group],used:&mut HashSet<String>){for g in gs{if let Some(name)=&g.layer{used.insert(name.clone());}mesh(&g.mesh,used);groups(&g.children,used);}}
        let mut used=HashSet::new();mesh(&self.mesh,&mut used);groups(&self.groups,&mut used);
        for d in &self.dimensions{if let Some(name)=&d.layer{used.insert(name.clone());}}for t in &self.text_labels{if let Some(name)=&t.layer{used.insert(name.clone());}}for p in &self.section_planes{if let Some(name)=p.native_document.get("layer").and_then(|v|v.as_str()){used.insert(name.into());}}for v in &self.saved_views{used.extend(v.hidden_layers.iter().cloned());}used
    }
    pub fn purge_unused_layers(&mut self)->usize{let used=self.used_layer_names();let before=self.layers.len();self.layers.retain(|l|l.name==crate::layers::DEFAULT_LAYER||used.contains(&l.name));let count=before-self.layers.len();if count>0{self.bump_version();}count}

    pub fn assign_selection_to_layer(&mut self, layer_name: &str) -> usize {
        let mut count = 0;
        let assigned=(layer_name!=crate::layers::DEFAULT_LAYER).then(||layer_name.to_owned());
        for item in &self.selection {
            match item {
                SelectionItem::Face(f_id) => {
                    if *f_id < self.mesh.faces.len() {
                        self.mesh.faces[*f_id].layer = assigned.clone();
                        if let Some(name)=&assigned{self.mesh.faces[*f_id].attrs.insert("layer".into(),name.clone().into());}else{self.mesh.faces[*f_id].attrs.remove("layer");}
                        count += 1;
                    }
                }
                SelectionItem::Edge(e_id) => {
                    if *e_id < self.mesh.edges.len() {
                        self.mesh.edges[*e_id].layer = assigned.clone();
                        count += 1;
                    }
                }
                SelectionItem::Group(g_id) => {
                    if *g_id < self.groups.len() {
                        self.groups[*g_id].layer = assigned.clone();
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


impl Scene{
 pub fn mesh_face_visible(&self,mesh:&Mesh,id:usize)->bool{mesh.faces.get(id).is_some_and(|face|!face.attrs.get("hidden").is_some_and(crate::presentation::truth)&&self.is_layer_visible(face.layer.as_deref().or_else(||face.attrs.get("layer").and_then(|v|v.as_str()))))}
 pub fn mesh_edge_visible(&self,mesh:&Mesh,id:usize)->bool{mesh.edges.get(id).is_some_and(|edge|!edge.hidden&&self.is_layer_visible(edge.layer.as_deref())&&(edge.faces.is_empty()||edge.faces.iter().any(|&id|self.mesh_face_visible(mesh,id))))}
 pub fn mesh_vertex_visible(&self,mesh:&Mesh,id:usize)->bool{mesh.vertices.get(id).is_some_and(|vertex|vertex.edges.is_empty()||vertex.edges.iter().any(|&id|self.mesh_edge_visible(mesh,id)))}
}

impl Scene {
    /// Source hidden-view action: turning either switch off clears hidden selections.
    pub fn set_hidden_view(&mut self,objects:bool,on:bool)->bool {
        let flag=if objects {&mut self.show_hidden_objects}else{&mut self.show_hidden_geometry};
        if *flag==on{return false;} *flag=on;
        if !on {
            fn group<'a>(groups:&'a [Group],id:uuid::Uuid)->Option<&'a Group>{
                for g in groups {if g.id==id{return Some(g);}if let Some(found)=group(&g.children,id){return Some(found);}}None
            }
            let mesh=&self.mesh;let groups=&self.groups;
            let hidden=|item:&SelectionItem|match item {
                SelectionItem::Face(id)=>mesh.faces.get(*id).is_some_and(|f|f.attrs.get("hidden").is_some_and(crate::presentation::truth)),
                SelectionItem::Edge(id)=>mesh.edges.get(*id).is_some_and(|e|e.hidden),
                SelectionItem::Group(id)=>groups.get(*id).is_some_and(|g|g.hidden),
                SelectionItem::ContextGroup{context,group:id}=>context.iter().any(|id|group(groups,*id).is_some_and(|g|g.hidden))||group(groups,*id).is_some_and(|g|g.hidden),
                SelectionItem::ContextFace{context,face,..}=>context.last().and_then(|id|group(groups,*id)).and_then(|g|g.mesh.faces.get(*face)).is_some_and(|f|f.attrs.get("hidden").is_some_and(crate::presentation::truth)),
                _=>false,
            };
            self.selection.retain(|item|!hidden(item));
            self.selection_order.retain(|item|!hidden(item));
        }
        self.bump_version();true
    }
    pub fn mesh_vertex_selectable(&self,mesh:&Mesh,id:usize)->bool { mesh.vertices.get(id).is_some_and(|v|v.edges.is_empty()||v.edges.iter().any(|&e|self.mesh_edge_selectable(mesh,e))) }
    pub fn group_selectable(&self, group:&Group)->bool { (!group.hidden || self.show_hidden_objects) && self.is_layer_selectable(group.layer.as_deref()) }
    pub fn mesh_face_selectable(&self, mesh: &Mesh, id: usize) -> bool {
        mesh.faces.get(id).is_some_and(|face| {
            (self.show_hidden_geometry || !face.attrs.get("hidden").is_some_and(crate::presentation::truth))
                && self.is_layer_selectable(face.layer.as_deref().or_else(|| {
                    face.attrs.get("layer").and_then(|v| v.as_str())
                }))
        })
    }

    pub fn mesh_edge_selectable(&self, mesh: &Mesh, id: usize) -> bool {
        mesh.edges.get(id).is_some_and(|edge| {
            (!edge.hidden || self.show_hidden_geometry) && self.is_layer_selectable(edge.layer.as_deref())
                && (edge.faces.is_empty() || edge.faces.iter().any(|&id| self.mesh_face_selectable(mesh, id)))
        })
    }
}

#[cfg(test)] mod hidden_view_tests {
 use super::*;
 #[test] fn ghost_view_changes_selection_without_normal_visibility() {
  let mut scene=Scene::new();let vertices=[glam::DVec3::ZERO,glam::DVec3::X,glam::DVec3::Y].map(|p|scene.mesh.add_vertex(p));
  let face=scene.mesh.add_face(vertices.to_vec(),vec![]).unwrap();scene.mesh.faces[face].attrs.insert("hidden".into(),serde_json::json!(true));
  let edge=scene.mesh.add_edge(vertices[0],vertices[1]).unwrap();scene.mesh.edges[edge].hidden=true;
  let mut group=Group::new("hidden");group.hidden=true;
  assert!(!scene.mesh_face_selectable(&scene.mesh,face));assert!(!scene.mesh_edge_selectable(&scene.mesh,edge));assert!(!scene.group_selectable(&group));
  scene.show_hidden_geometry=true;scene.show_hidden_objects=true;
  assert!(scene.mesh_face_selectable(&scene.mesh,face));assert!(scene.mesh_edge_selectable(&scene.mesh,edge));assert!(scene.group_selectable(&group));
  assert!(!scene.mesh_face_visible(&scene.mesh,face));assert!(!scene.mesh_edge_visible(&scene.mesh,edge));
  scene.layers[0].locked=true;assert!(!scene.mesh_face_selectable(&scene.mesh,face));assert!(!scene.group_selectable(&group));
  scene.layers[0].locked=false;scene.layers[0].visible=false;assert!(!scene.mesh_edge_selectable(&scene.mesh,edge));assert!(!scene.group_selectable(&group));
 }
}
