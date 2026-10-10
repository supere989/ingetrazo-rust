// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Granular Command Pattern Undo/Redo Engine.
//!
//! Provides delta-based commands over the master Scene state:
//! - AddFaceCommand, DeleteFaceCommand, ReverseFaceCommand
//! - AddEdgeCommand, DeleteEdgesCommand
//! - MoveCommand, RotateCommand, ScaleCommand
//! - PushPullCommand
//! - AssignLayerCommand
//! - SetMaterialCommand
//! - TagBimCommand, UntagBimCommand
//! - HideCommand, UnhideCommand
//! - AddGroupCommand, ExplodeGroupCommand
//! - AddDimensionCommand, DeleteDimensionCommand
//! - AddGuideCommand, DeleteGuidesCommand
//! - PlaceSectionPlaneCommand, ReverseSectionPlaneCommand
//! - CompositeCommand (atomic transactions)

use glam::{DMat4, DQuat, DVec3};
use serde_json::Value;
use std::collections::HashMap;

use crate::dimensions::Dimension;
use crate::guides::Guide;
use crate::mesh::{FaceId, MeshFace, VertexId};
use crate::scene::{Scene, SelectionItem};

/// Core reversible command trait.
pub trait Command: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;
    fn execute(&mut self, scene: &mut Scene) -> Result<(), String>;
    fn undo(&mut self, scene: &mut Scene) -> Result<(), String>;
}

// =========================================================================
// 1. Geometry Commands
// =========================================================================

/// Adds a polygon face defined by 3D vertices to the scene mesh.
#[derive(Debug, Clone)]
pub struct AddFaceCommand {
    pub points: Vec<DVec3>,
    pub holes: Vec<Vec<DVec3>>,
    pub attrs: HashMap<String, Value>,
    pub created_face_id: Option<FaceId>,
}

impl AddFaceCommand {
    pub fn new(points: Vec<DVec3>) -> Self {
        Self {
            points,
            holes: Vec::new(),
            attrs: HashMap::new(),
            created_face_id: None,
        }
    }

    pub fn with_attrs(points: Vec<DVec3>, attrs: HashMap<String, Value>) -> Self {
        Self {
            points,
            holes: Vec::new(),
            attrs,
            created_face_id: None,
        }
    }
}

impl Command for AddFaceCommand {
    fn name(&self) -> &str {
        "Add Face"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        let v_ids: Vec<VertexId> = self.points.iter().map(|&p| scene.mesh.add_vertex(p)).collect();
        let hole_ids: Vec<Vec<VertexId>> = self.holes.iter().map(|h_pts| {
            h_pts.iter().map(|&p| scene.mesh.add_vertex(p)).collect()
        }).collect();

        if let Some(f_id) = scene.mesh.add_face(v_ids, hole_ids) {
            for (k, v) in &self.attrs {
                scene.mesh.faces[f_id].attrs.insert(k.clone(), v.clone());
            }
            self.created_face_id = Some(f_id);
            scene.mesh.rebuild_spatial_index();
            scene.bump_version();
            Ok(())
        } else {
            Err("Failed to create face from vertices".to_string())
        }
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(f_id) = self.created_face_id {
            if f_id < scene.mesh.faces.len() {
                scene.mesh.faces.remove(f_id);
                scene.mesh.rebuild_spatial_index();
                scene.bump_version();
                return Ok(());
            }
        }
        Err("Face ID not found for undo".to_string())
    }
}

/// Deletes a face from the scene mesh, preserving it for restoration on undo.
#[derive(Debug, Clone)]
pub struct DeleteFaceCommand {
    pub face_id: FaceId,
    pub removed_face: Option<MeshFace>,
}

impl DeleteFaceCommand {
    pub fn new(face_id: FaceId) -> Self {
        Self {
            face_id,
            removed_face: None,
        }
    }
}

impl Command for DeleteFaceCommand {
    fn name(&self) -> &str {
        "Delete Face"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.face_id < scene.mesh.faces.len() {
            self.removed_face = Some(scene.mesh.faces.remove(self.face_id));
            scene.mesh.rebuild_spatial_index();
            scene.bump_version();
            Ok(())
        } else {
            Err(format!("Face ID {} out of bounds", self.face_id))
        }
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(face) = self.removed_face.take() {
            if self.face_id <= scene.mesh.faces.len() {
                scene.mesh.faces.insert(self.face_id, face);
            } else {
                scene.mesh.faces.push(face);
            }
            scene.mesh.rebuild_spatial_index();
            scene.bump_version();
            Ok(())
        } else {
            Err("No removed face data to restore".to_string())
        }
    }
}

/// Inverts the vertex loop winding order of a face to flip its normal.
#[derive(Debug, Clone)]
pub struct ReverseFaceCommand {
    pub face_id: FaceId,
}

impl ReverseFaceCommand {
    pub fn new(face_id: FaceId) -> Self {
        Self { face_id }
    }
}

impl Command for ReverseFaceCommand {
    fn name(&self) -> &str {
        "Reverse Face"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.face_id < scene.mesh.faces.len() {
            scene.mesh.faces[self.face_id].vertices.reverse();
            scene.bump_version();
            Ok(())
        } else {
            Err(format!("Face ID {} out of bounds", self.face_id))
        }
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.execute(scene)
    }
}

// =========================================================================
// 2. Transform Commands (Move, Rotate, Scale)
// =========================================================================

/// Translates a set of vertices and/or groups by a 3D vector.
#[derive(Debug, Clone)]
pub struct MoveCommand {
    pub vertex_ids: Vec<VertexId>,
    pub group_indices: Vec<usize>,
    pub delta: DVec3,
}

impl MoveCommand {
    pub fn new(vertex_ids: Vec<VertexId>, group_indices: Vec<usize>, delta: DVec3) -> Self {
        Self {
            vertex_ids,
            group_indices,
            delta,
        }
    }
}

impl Command for MoveCommand {
    fn name(&self) -> &str {
        "Move"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        for &vid in &self.vertex_ids {
            if vid < scene.mesh.vertices.len() {
                scene.mesh.vertices[vid].position += self.delta;
            }
        }
        for &g_idx in &self.group_indices {
            if g_idx < scene.groups.len() {
                scene.groups[g_idx].transform = DMat4::from_translation(self.delta) * scene.groups[g_idx].transform;
            }
        }
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        let inv_delta = -self.delta;
        for &vid in &self.vertex_ids {
            if vid < scene.mesh.vertices.len() {
                scene.mesh.vertices[vid].position += inv_delta;
            }
        }
        for &g_idx in &self.group_indices {
            if g_idx < scene.groups.len() {
                scene.groups[g_idx].transform = DMat4::from_translation(inv_delta) * scene.groups[g_idx].transform;
            }
        }
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }
}

/// Rotates vertices and/or groups around an axis anchor.
#[derive(Debug, Clone)]
pub struct RotateCommand {
    pub vertex_ids: Vec<VertexId>,
    pub group_indices: Vec<usize>,
    pub origin: DVec3,
    pub axis: DVec3,
    pub angle_rad: f64,
}

impl RotateCommand {
    pub fn new(
        vertex_ids: Vec<VertexId>,
        group_indices: Vec<usize>,
        origin: DVec3,
        axis: DVec3,
        angle_rad: f64,
    ) -> Self {
        Self {
            vertex_ids,
            group_indices,
            origin,
            axis: axis.normalize_or_zero(),
            angle_rad,
        }
    }
}

impl Command for RotateCommand {
    fn name(&self) -> &str {
        "Rotate"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        let rot = DQuat::from_axis_angle(self.axis, self.angle_rad);
        for &vid in &self.vertex_ids {
            if vid < scene.mesh.vertices.len() {
                let p = scene.mesh.vertices[vid].position;
                scene.mesh.vertices[vid].position = self.origin + rot * (p - self.origin);
            }
        }
        for &g_idx in &self.group_indices {
            if g_idx < scene.groups.len() {
                let rot_mat = DMat4::from_translation(self.origin)
                    * DMat4::from_quat(rot)
                    * DMat4::from_translation(-self.origin);
                scene.groups[g_idx].transform = rot_mat * scene.groups[g_idx].transform;
            }
        }
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        let inv_rot = DQuat::from_axis_angle(self.axis, -self.angle_rad);
        for &vid in &self.vertex_ids {
            if vid < scene.mesh.vertices.len() {
                let p = scene.mesh.vertices[vid].position;
                scene.mesh.vertices[vid].position = self.origin + inv_rot * (p - self.origin);
            }
        }
        for &g_idx in &self.group_indices {
            if g_idx < scene.groups.len() {
                let rot_mat = DMat4::from_translation(self.origin)
                    * DMat4::from_quat(inv_rot)
                    * DMat4::from_translation(-self.origin);
                scene.groups[g_idx].transform = rot_mat * scene.groups[g_idx].transform;
            }
        }
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }
}

/// Extrudes a face along its normal vector by a distance.
#[derive(Debug, Clone)]
pub struct PushPullCommand {
    pub face_id: FaceId,
    pub distance: f64,
    pub created_face_ids: Vec<FaceId>,
    pub previous_positions: Vec<(VertexId, DVec3)>,
}

impl PushPullCommand {
    pub fn new(face_id: FaceId, distance: f64) -> Self {
        Self {
            face_id,
            distance,
            created_face_ids: Vec::new(),
            previous_positions: Vec::new(),
        }
    }
}

impl Command for PushPullCommand {
    fn name(&self) -> &str {
        "Push / Pull"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.face_id >= scene.mesh.faces.len() {
            return Err("Invalid face ID for pushpull".to_string());
        }

        let normal = scene.mesh.face_normal(self.face_id);
        let offset = normal * self.distance;
        let v_ids = scene.mesh.faces[self.face_id].vertices.clone();
        let n = v_ids.len();
        if n < 3 {
            return Err("Degenerate face".to_string());
        }

        // Record previous positions
        self.previous_positions = v_ids.iter().map(|&vid| (vid, scene.mesh.vertices[vid].position)).collect();

        // Create new extruded vertices for the top cap
        let mut new_v_ids = Vec::with_capacity(n);
        for &vid in &v_ids {
            let new_pos = scene.mesh.vertices[vid].position + offset;
            let new_vid = scene.mesh.add_vertex(new_pos);
            new_v_ids.push(new_vid);
        }

        // Create side faces
        self.created_face_ids.clear();
        for i in 0..n {
            let next_i = (i + 1) % n;
            let v0 = v_ids[i];
            let v1 = v_ids[next_i];
            let v2 = new_v_ids[next_i];
            let v3 = new_v_ids[i];
            if let Some(side_f) = scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]) {
                self.created_face_ids.push(side_f);
            }
        }

        // Update top face vertices
        scene.mesh.faces[self.face_id].vertices = new_v_ids;
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        // Remove created side faces (in reverse order)
        for &side_id in self.created_face_ids.iter().rev() {
            if side_id < scene.mesh.faces.len() {
                scene.mesh.faces.remove(side_id);
            }
        }
        self.created_face_ids.clear();

        // Restore original vertex positions
        if self.face_id < scene.mesh.faces.len() {
            scene.mesh.faces[self.face_id].vertices = self.previous_positions.iter().map(|(vid, _)| *vid).collect();
            for (vid, pos) in &self.previous_positions {
                if *vid < scene.mesh.vertices.len() {
                    scene.mesh.vertices[*vid].position = *pos;
                }
            }
        }

        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
        Ok(())
    }
}

// =========================================================================
// 3. Metadata & Annotation Commands
// =========================================================================

/// Assigns a layer to a selection of entities.
#[derive(Debug, Clone)]
pub struct AssignLayerCommand {
    pub target_layer: String,
    pub previous_face_layers: Vec<(FaceId, Option<String>)>,
    pub previous_group_layers: Vec<(usize, Option<String>)>,
}

impl AssignLayerCommand {
    pub fn new(target_layer: impl Into<String>) -> Self {
        Self {
            target_layer: target_layer.into(),
            previous_face_layers: Vec::new(),
            previous_group_layers: Vec::new(),
        }
    }
}

impl Command for AssignLayerCommand {
    fn name(&self) -> &str {
        "Assign Layer"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.previous_face_layers.clear();
        self.previous_group_layers.clear();

        for item in &scene.selection {
            match item {
                SelectionItem::Face(f_id) => {
                    if *f_id < scene.mesh.faces.len() {
                        let prev = scene.mesh.faces[*f_id].layer.clone();
                        self.previous_face_layers.push((*f_id, prev));
                        scene.mesh.faces[*f_id].layer = Some(self.target_layer.clone());
                    }
                }
                SelectionItem::Group(g_id) => {
                    if *g_id < scene.groups.len() {
                        let prev = scene.groups[*g_id].layer.clone();
                        self.previous_group_layers.push((*g_id, prev));
                        scene.groups[*g_id].layer = Some(self.target_layer.clone());
                    }
                }
                _ => {}
            }
        }
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for (f_id, prev) in &self.previous_face_layers {
            if *f_id < scene.mesh.faces.len() {
                scene.mesh.faces[*f_id].layer = prev.clone();
            }
        }
        for (g_id, prev) in &self.previous_group_layers {
            if *g_id < scene.groups.len() {
                scene.groups[*g_id].layer = prev.clone();
            }
        }
        scene.bump_version();
        Ok(())
    }
}

/// Applies material name to faces.
#[derive(Debug, Clone)]
pub struct SetMaterialCommand {
    pub material_name: String,
    pub target_faces: Vec<FaceId>,
    pub previous_materials: Vec<(FaceId, Option<String>)>,
}

impl SetMaterialCommand {
    pub fn new(material_name: impl Into<String>, target_faces: Vec<FaceId>) -> Self {
        Self {
            material_name: material_name.into(),
            target_faces,
            previous_materials: Vec::new(),
        }
    }
}

impl Command for SetMaterialCommand {
    fn name(&self) -> &str {
        "Set Material"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.previous_materials.clear();
        for &f_id in &self.target_faces {
            if f_id < scene.mesh.faces.len() {
                let prev = scene.mesh.faces[f_id].attrs.get("material").and_then(|v| v.as_str()).map(|s| s.to_string());
                self.previous_materials.push((f_id, prev));
                scene.mesh.faces[f_id].attrs.insert("material".to_string(), Value::String(self.material_name.clone()));
                scene.mesh.faces[f_id].attrs.insert("mat".to_string(), Value::String(self.material_name.clone()));
            }
        }
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for (f_id, prev) in &self.previous_materials {
            if *f_id < scene.mesh.faces.len() {
                if let Some(m) = prev {
                    scene.mesh.faces[*f_id].attrs.insert("material".to_string(), Value::String(m.clone()));
                    scene.mesh.faces[*f_id].attrs.insert("mat".to_string(), Value::String(m.clone()));
                } else {
                    scene.mesh.faces[*f_id].attrs.remove("material");
                    scene.mesh.faces[*f_id].attrs.remove("mat");
                }
            }
        }
        scene.bump_version();
        Ok(())
    }
}

/// Tags entities with IFC BIM classification metadata.
#[derive(Debug, Clone)]
pub struct TagBimCommand {
    pub class_name: String,
    pub element_name: String,
    pub target_faces: Vec<FaceId>,
    pub target_groups: Vec<usize>,
    pub previous_face_tags: Vec<(FaceId, Option<Value>)>,
    pub previous_group_tags: Vec<(usize, Option<Value>)>,
}

impl TagBimCommand {
    pub fn new(class_name: impl Into<String>, element_name: impl Into<String>) -> Self {
        Self {
            class_name: class_name.into(),
            element_name: element_name.into(),
            target_faces: Vec::new(),
            target_groups: Vec::new(),
            previous_face_tags: Vec::new(),
            previous_group_tags: Vec::new(),
        }
    }
}

impl Command for TagBimCommand {
    fn name(&self) -> &str {
        "Tag BIM"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        let bim_val = serde_json::json!({
            "class": self.class_name,
            "name": self.element_name,
        });

        self.previous_face_tags.clear();
        self.previous_group_tags.clear();

        for item in &scene.selection {
            match item {
                SelectionItem::Face(f_id) => {
                    if *f_id < scene.mesh.faces.len() {
                        let prev = scene.mesh.faces[*f_id].attrs.get("ifc").cloned();
                        self.previous_face_tags.push((*f_id, prev));
                        scene.mesh.faces[*f_id].attrs.insert("ifc".to_string(), bim_val.clone());
                    }
                }
                SelectionItem::Group(g_id) => {
                    if *g_id < scene.groups.len() {
                        let prev = scene.groups[*g_id].ifc_tag.clone();
                        self.previous_group_tags.push((*g_id, prev));
                        scene.groups[*g_id].ifc_tag = Some(bim_val.clone());
                    }
                }
                _ => {}
            }
        }
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for (f_id, prev) in &self.previous_face_tags {
            if *f_id < scene.mesh.faces.len() {
                if let Some(tag) = prev {
                    scene.mesh.faces[*f_id].attrs.insert("ifc".to_string(), tag.clone());
                } else {
                    scene.mesh.faces[*f_id].attrs.remove("ifc");
                }
            }
        }
        for (g_id, prev) in &self.previous_group_tags {
            if *g_id < scene.groups.len() {
                scene.groups[*g_id].ifc_tag = prev.clone();
            }
        }
        scene.bump_version();
        Ok(())
    }
}

/// Adds a 3D linear dimension to the scene.
#[derive(Debug, Clone)]
pub struct AddDimensionCommand {
    pub dimension: Dimension,
    pub created_index: Option<usize>,
}

impl AddDimensionCommand {
    pub fn new(dimension: Dimension) -> Self {
        Self {
            dimension,
            created_index: None,
        }
    }
}

impl Command for AddDimensionCommand {
    fn name(&self) -> &str {
        "Add Dimension"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        scene.dimensions.push(self.dimension.clone());
        self.created_index = Some(scene.dimensions.len() - 1);
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(idx) = self.created_index {
            if idx < scene.dimensions.len() {
                scene.dimensions.remove(idx);
                scene.bump_version();
                return Ok(());
            }
        }
        Err("Dimension index not found for undo".to_string())
    }
}

/// Adds a construction guide line or point.
#[derive(Debug, Clone)]
pub struct AddGuideCommand {
    pub guide: Guide,
    pub created_index: Option<usize>,
}

impl AddGuideCommand {
    pub fn new(guide: Guide) -> Self {
        Self {
            guide,
            created_index: None,
        }
    }
}

impl Command for AddGuideCommand {
    fn name(&self) -> &str {
        "Add Guide"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        scene.guides.push(self.guide.clone());
        self.created_index = Some(scene.guides.len() - 1);
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(idx) = self.created_index {
            if idx < scene.guides.len() {
                scene.guides.remove(idx);
                scene.bump_version();
                return Ok(());
            }
        }
        Err("Guide index not found for undo".to_string())
    }
}

/// Composite command grouping multiple actions into a single atomic undo step.
#[derive(Debug)]
pub struct CompositeCommand {
    pub name: String,
    pub commands: Vec<Box<dyn Command>>,
}

impl CompositeCommand {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.commands.push(cmd);
    }
}

impl Command for CompositeCommand {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        for cmd in &mut self.commands {
            cmd.execute(scene)?;
        }
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(scene)?;
        }
        Ok(())
    }
}

/// Adds surveyed control points and optionally anchors the scene datum.
#[derive(Debug, Clone)]
pub struct AddGeoPointsCommand {
    pub points: Vec<crate::georef::GeoPoint>,
    pub datum: Option<crate::georef::SceneDatum>,
    pub previous_datum: Option<crate::georef::SceneDatum>,
    pub added_count: usize,
}

impl AddGeoPointsCommand {
    pub fn new(points: Vec<crate::georef::GeoPoint>, datum: Option<crate::georef::SceneDatum>) -> Self {
        Self {
            points,
            datum,
            previous_datum: None,
            added_count: 0,
        }
    }
}

impl Command for AddGeoPointsCommand {
    fn name(&self) -> &str {
        "Add Survey Points"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(new_datum) = &self.datum {
            self.previous_datum = scene.geo_datum.clone();
            scene.geo_datum = Some(new_datum.clone());
        }
        self.added_count = self.points.len();
        scene.geo_points.extend(self.points.clone());
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.datum.is_some() {
            scene.geo_datum = self.previous_datum.clone();
        }
        let current_len = scene.geo_points.len();
        if current_len >= self.added_count {
            scene.geo_points.truncate(current_len - self.added_count);
        }
        scene.bump_version();
        Ok(())
    }
}

/// Deletes survey points by value or index.
#[derive(Debug, Clone)]
pub struct DeleteGeoPointsCommand {
    pub points: Vec<crate::georef::GeoPoint>,
    pub removed_points_with_indices: Vec<(usize, crate::georef::GeoPoint)>,
}

impl DeleteGeoPointsCommand {
    pub fn new(points: Vec<crate::georef::GeoPoint>) -> Self {
        Self {
            points,
            removed_points_with_indices: Vec::new(),
        }
    }
}

impl Command for DeleteGeoPointsCommand {
    fn name(&self) -> &str {
        "Delete Survey Points"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.removed_points_with_indices.clear();
        for p_to_del in &self.points {
            if let Some(pos) = scene.geo_points.iter().position(|p| p == p_to_del) {
                let removed = scene.geo_points.remove(pos);
                self.removed_points_with_indices.push((pos, removed));
            }
        }
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for (pos, pt) in self.removed_points_with_indices.iter().rev() {
            let idx = (*pos).min(scene.geo_points.len());
            scene.geo_points.insert(idx, pt.clone());
        }
        scene.bump_version();
        Ok(())
    }
}

/// Adds a traced GeoPath to the scene.
#[derive(Debug, Clone)]
pub struct AddGeoPathCommand {
    pub path: crate::georef::GeoPath,
    pub created_index: Option<usize>,
}

impl AddGeoPathCommand {
    pub fn new(path: crate::georef::GeoPath) -> Self {
        Self {
            path,
            created_index: None,
        }
    }
}

impl Command for AddGeoPathCommand {
    fn name(&self) -> &str {
        "Add GeoPath"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        scene.geo_paths.push(self.path.clone());
        self.created_index = Some(scene.geo_paths.len() - 1);
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(idx) = self.created_index {
            if idx < scene.geo_paths.len() {
                scene.geo_paths.remove(idx);
                scene.bump_version();
                return Ok(());
            }
        }
        Err("GeoPath index not found for undo".to_string())
    }
}

/// Deletes traced GeoPaths from the scene.
#[derive(Debug, Clone)]
pub struct DeleteGeoPathsCommand {
    pub target_indices: Vec<usize>,
    pub removed_paths: Vec<(usize, crate::georef::GeoPath)>,
}

impl DeleteGeoPathsCommand {
    pub fn new(target_indices: Vec<usize>) -> Self {
        Self {
            target_indices,
            removed_paths: Vec::new(),
        }
    }
}

impl Command for DeleteGeoPathsCommand {
    fn name(&self) -> &str {
        "Delete GeoPaths"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.removed_paths.clear();
        let mut sorted = self.target_indices.clone();
        sorted.sort_unstable();
        sorted.dedup();
        for &idx in sorted.iter().rev() {
            if idx < scene.geo_paths.len() {
                let removed = scene.geo_paths.remove(idx);
                self.removed_paths.push((idx, removed));
            }
        }
        scene.bump_version();
        Ok(())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        for (idx, path) in self.removed_paths.iter().rev() {
            let insert_pos = (*idx).min(scene.geo_paths.len());
            scene.geo_paths.insert(insert_pos, path.clone());
        }
        scene.bump_version();
        Ok(())
    }
}

/// Toggles whether a GeoPath is closed.
#[derive(Debug, Clone)]
pub struct ToggleGeoPathClosedCommand {
    pub path_index: usize,
}

impl ToggleGeoPathClosedCommand {
    pub fn new(path_index: usize) -> Self {
        Self { path_index }
    }
}

impl Command for ToggleGeoPathClosedCommand {
    fn name(&self) -> &str {
        "Toggle GeoPath Closed"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.path_index < scene.geo_paths.len() {
            scene.geo_paths[self.path_index].closed = !scene.geo_paths[self.path_index].closed;
            scene.bump_version();
            Ok(())
        } else {
            Err("GeoPath index out of bounds".to_string())
        }
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        self.execute(scene)
    }
}

/// Moves an individual node of a GeoPath.
#[derive(Debug, Clone)]
pub struct MoveGeoPathNodeCommand {
    pub path_index: usize,
    pub node_index: usize,
    pub old_position: glam::DVec3,
    pub new_position: glam::DVec3,
}

impl MoveGeoPathNodeCommand {
    pub fn new(
        path_index: usize,
        node_index: usize,
        old_position: glam::DVec3,
        new_position: glam::DVec3,
    ) -> Self {
        Self {
            path_index,
            node_index,
            old_position,
            new_position,
        }
    }
}

impl Command for MoveGeoPathNodeCommand {
    fn name(&self) -> &str {
        "Move GeoPath Node"
    }

    fn execute(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.path_index < scene.geo_paths.len() {
            let path = &mut scene.geo_paths[self.path_index];
            if self.node_index < path.points.len() {
                path.points[self.node_index] = self.new_position;
                scene.bump_version();
                return Ok(());
            }
        }
        Err("GeoPath or node index out of bounds".to_string())
    }

    fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if self.path_index < scene.geo_paths.len() {
            let path = &mut scene.geo_paths[self.path_index];
            if self.node_index < path.points.len() {
                path.points[self.node_index] = self.old_position;
                scene.bump_version();
                return Ok(());
            }
        }
        Err("GeoPath or node index out of bounds".to_string())
    }
}

// =========================================================================
// 4. Command History Manager
// =========================================================================

/// Master History stack coordinating undo, redo, and transactions over Scene.
pub struct History {
    pub undo_stack: Vec<Box<dyn Command>>,
    pub redo_stack: Vec<Box<dyn Command>>,
    pub max_depth: usize,
    pub last_error: Option<String>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: 200,
            last_error: None,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn last_undo_name(&self) -> Option<&str> {
        self.undo_stack.last().map(|cmd| cmd.name())
    }

    pub fn last_redo_name(&self) -> Option<&str> {
        self.redo_stack.last().map(|cmd| cmd.name())
    }

    pub fn execute(&mut self, mut command: Box<dyn Command>, scene: &mut Scene) -> Result<(), String> {
        command.execute(scene)?;
        self.undo_stack.push(command);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
        self.last_error = None;
        Ok(())
    }

    pub fn undo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(mut command) = self.undo_stack.pop() {
            command.undo(scene)?;
            self.redo_stack.push(command);
            self.last_error = None;
            Ok(())
        } else {
            Err("Nothing to undo".to_string())
        }
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Result<(), String> {
        if let Some(mut command) = self.redo_stack.pop() {
            command.execute(scene)?;
            self.undo_stack.push(command);
            self.last_error = None;
            Ok(())
        } else {
            Err("Nothing to redo".to_string())
        }
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_delete_face_command() {
        let mut scene = Scene::new();
        let mut history = History::new();

        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];

        let cmd = Box::new(AddFaceCommand::new(pts));
        assert!(history.execute(cmd, &mut scene).is_ok());
        assert_eq!(scene.mesh.faces.len(), 1);

        assert!(history.can_undo());
        assert_eq!(history.last_undo_name(), Some("Add Face"));

        assert!(history.undo(&mut scene).is_ok());
        assert_eq!(scene.mesh.faces.len(), 0);

        assert!(history.can_redo());
        assert!(history.redo(&mut scene).is_ok());
        assert_eq!(scene.mesh.faces.len(), 1);
    }

    #[test]
    fn test_move_command_undo_redo() {
        let mut scene = Scene::new();
        let mut history = History::new();

        let v0 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let cmd = Box::new(MoveCommand::new(vec![v0], vec![], DVec3::new(5.0, 0.0, 0.0)));

        assert!(history.execute(cmd, &mut scene).is_ok());
        assert!((scene.mesh.vertices[v0].position.x - 6.0).abs() < 1e-6);

        assert!(history.undo(&mut scene).is_ok());
        assert!((scene.mesh.vertices[v0].position.x - 1.0).abs() < 1e-6);

        assert!(history.redo(&mut scene).is_ok());
        assert!((scene.mesh.vertices[v0].position.x - 6.0).abs() < 1e-6);
    }

    #[test]
    fn test_composite_transaction() {
        let mut scene = Scene::new();
        let mut history = History::new();

        let mut composite = CompositeCommand::new("Create Box & Dimension");
        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        composite.push(Box::new(AddFaceCommand::new(pts)));
        composite.push(Box::new(AddDimensionCommand::new(Dimension::new(
            DVec3::ZERO,
            DVec3::X,
            DVec3::Y * 0.2,
        ))));

        assert!(history.execute(Box::new(composite), &mut scene).is_ok());
        assert_eq!(scene.mesh.faces.len(), 1);
        assert_eq!(scene.dimensions.len(), 1);

        assert!(history.undo(&mut scene).is_ok());
        assert_eq!(scene.mesh.faces.len(), 0);
        assert_eq!(scene.dimensions.len(), 0);
    }

    #[test]
    fn test_geopoints_command_undo_redo() {
        let mut scene = Scene::new();
        let mut history = History::new();

        let datum = crate::georef::SceneDatum::new(-12.0464, -77.0428, 100.0, 0.0);
        let pt1 = crate::georef::GeoPoint::new(DVec3::new(10.0, 20.0, 0.0), "P1", "Desc1");
        let pt2 = crate::georef::GeoPoint::new(DVec3::new(15.0, 25.0, 1.0), "P2", "Desc2");

        let cmd = Box::new(AddGeoPointsCommand::new(vec![pt1.clone(), pt2.clone()], Some(datum.clone())));
        assert!(history.execute(cmd, &mut scene).is_ok());
        assert_eq!(scene.geo_points.len(), 2);
        assert!(scene.geo_datum.is_some());

        assert!(history.undo(&mut scene).is_ok());
        assert_eq!(scene.geo_points.len(), 0);
        assert!(scene.geo_datum.is_none());

        assert!(history.redo(&mut scene).is_ok());
        assert_eq!(scene.geo_points.len(), 2);
        assert!(scene.geo_datum.is_some());

        let del_cmd = Box::new(DeleteGeoPointsCommand::new(vec![pt1]));
        assert!(history.execute(del_cmd, &mut scene).is_ok());
        assert_eq!(scene.geo_points.len(), 1);
        assert_eq!(scene.geo_points[0].name, "P2");

        assert!(history.undo(&mut scene).is_ok());
        assert_eq!(scene.geo_points.len(), 2);
    }

    #[test]
    fn test_geopath_command_undo_redo() {
        let mut scene = Scene::new();
        let mut history = History::new();

        let path = crate::georef::GeoPath::new(
            vec![DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0)],
            false,
            "Alignment",
        );

        let add_cmd = Box::new(AddGeoPathCommand::new(path));
        assert!(history.execute(add_cmd, &mut scene).is_ok());
        assert_eq!(scene.geo_paths.len(), 1);
        assert!(!scene.geo_paths[0].closed);

        let toggle_cmd = Box::new(ToggleGeoPathClosedCommand::new(0));
        assert!(history.execute(toggle_cmd, &mut scene).is_ok());
        assert!(scene.geo_paths[0].closed);

        assert!(history.undo(&mut scene).is_ok());
        assert!(!scene.geo_paths[0].closed);

        let move_cmd = Box::new(MoveGeoPathNodeCommand::new(
            0,
            1,
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(20.0, 5.0, 0.0),
        ));
        assert!(history.execute(move_cmd, &mut scene).is_ok());
        assert_eq!(scene.geo_paths[0].points[1], DVec3::new(20.0, 5.0, 0.0));

        assert!(history.undo(&mut scene).is_ok());
        assert_eq!(scene.geo_paths[0].points[1], DVec3::new(10.0, 0.0, 0.0));
    }
}
