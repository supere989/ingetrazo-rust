// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::{DMat4, DQuat, DVec3};
use ingetrazo_core::scene::{Scene, SelectionItem};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::base::{Tool, ToolContext, ToolType};

/// Rotate tool with 3-click protractor workflow around plane normal.
#[derive(Debug, Clone)]
pub struct RotateTool {
    pub center: Option<DVec3>,
    pub ref_arm: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub is_copy_mode: bool,
    pub target_vertices: Vec<usize>,
    pub target_groups: Vec<usize>,
    pub target_planes: Vec<usize>,
    pub target_images: Vec<usize>,
    original_images: Vec<(usize, ingetrazo_core::image_plane::ImagePlane)>,
    image_command: Option<ingetrazo_core::image_plane::ImageFrameCommand>,
    image_snapshot_taken: bool,
    image_epoch: u64,
    image_indices: HashMap<u128, usize>,
    pub image_membership_rebuilds: u64,
    pub image_membership_scanned: u64,
    original_version: u64,
    preview_version: u64,
    control_down: bool,
    pub last_angle_rad: Option<f64>,
}

impl Default for RotateTool {
    fn default() -> Self {
        Self {
            center: None,
            ref_arm: None,
            hover_point: None,
            plane_normal: DVec3::Z,
            is_copy_mode: false,
            target_vertices: vec![],
            target_groups: vec![],
            target_planes: vec![],
            target_images: vec![],
            original_images: vec![],
            image_command: None,
            image_snapshot_taken: false,
            image_epoch: 0,
            image_indices: HashMap::new(),
            image_membership_rebuilds: 0,
            image_membership_scanned: 0,
            original_version: 0,
            preview_version: 0,
            control_down: false,
            last_angle_rad: None,
        }
    }
}
impl RotateTool {
    pub fn new() -> Self {
        Self {
            plane_normal: DVec3::Z,
            ..Self::default()
        }
    }

    pub fn with_normal(normal: DVec3) -> Self {
        Self {
            plane_normal: normal.normalize_or_zero(),
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.center = None;
        self.ref_arm = None;
        self.hover_point = None;
        self.is_copy_mode = false;
        self.target_vertices.clear();
        self.target_groups.clear();
        self.target_planes.clear();
        self.target_images.clear();
        self.original_images.clear();
        self.image_snapshot_taken = false;
    }

    pub fn gather_targets(&mut self, scene: &Scene, ctx: &ToolContext) {
        self.image_command = None;
        self.image_snapshot_taken = false;
        self.target_images = crate::base::gather_image_targets(scene, ctx);
        self.original_images = self
            .target_images
            .iter()
            .map(|&id| (id, scene.image_planes[id].clone()))
            .collect();
        self.original_version = scene.version;
        self.preview_version = scene.version;
        let mut vert_set = HashSet::new();
        let mut group_set = HashSet::new();
        let mut plane_set = HashSet::new();

        if !scene.selection.is_empty() {
            for item in &scene.selection {
                match item {
                    SelectionItem::Vertex(vid) => {
                        vert_set.insert(*vid);
                    }
                    SelectionItem::Edge(eid) => {
                        if *eid < scene.mesh.edges.len() {
                            let e = &scene.mesh.edges[*eid];
                            vert_set.insert(e.v0);
                            vert_set.insert(e.v1);
                        }
                    }
                    SelectionItem::Face(fid) => {
                        if *fid < scene.mesh.faces.len() {
                            for &v in &scene.mesh.faces[*fid].vertices {
                                vert_set.insert(v);
                            }
                        }
                    }
                    SelectionItem::Group(gid) => {
                        group_set.insert(*gid);
                    }
                    SelectionItem::SectionPlane(id) => {
                        plane_set.insert(*id);
                    }
                    SelectionItem::Guide(_) | SelectionItem::Dimension(_) | SelectionItem::TextLabel(_) | SelectionItem::ContextFace{..} | SelectionItem::ContextGroup{..} | SelectionItem::ImagePlane(_)
                    | SelectionItem::GeoPoint(_)
                    | SelectionItem::GeoPath(_) => {}
                }
            }
        } else if let Some(id) = crate::base::pick_section_plane(scene, &ctx.ray, 0.2) {
            plane_set.insert(id);
        } else if let Some(vid) = crate::base::pick_scene_vertex(scene, &ctx.ray, 0.2) {
            vert_set.insert(vid);
        } else if let Some(eid) = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2) {
            let e = &scene.mesh.edges[eid];
            vert_set.insert(e.v0);
            vert_set.insert(e.v1);
        } else if let Some((fid, _, _)) = crate::base::pick_scene_face(scene, &ctx.ray) {
            for &v in &scene.mesh.faces[fid].vertices {
                vert_set.insert(v);
            }
        }

        self.target_vertices = vert_set.into_iter().collect();
        self.target_groups = group_set.into_iter().collect();
        self.target_planes = plane_set.into_iter().collect();
    }

    /// Calculate signed angle in radians around normal between ref_arm and current arm.
    pub fn compute_angle(&self, arm_point: DVec3) -> f64 {
        let (Some(c), Some(r)) = (self.center, self.ref_arm) else {
            return 0.0;
        };

        let v1 = (r - c).normalize_or_zero();
        let v2 = (arm_point - c).normalize_or_zero();

        let dot = v1.dot(v2).clamp(-1.0, 1.0);
        let cross = v1.cross(v2);
        let sign = if cross.dot(self.plane_normal) >= 0.0 {
            1.0
        } else {
            -1.0
        };

        sign * dot.acos()
    }

    pub fn execute_rotation(&mut self, angle_rad: f64, scene: &mut Scene) {
        if self.is_copy_mode && self.target_vertices.is_empty() && self.target_groups.is_empty() {
            self.restore_images(scene);
            return;
        }
        let Some(center) = self.center else { return };
        self.restore_images(scene);
        if !angle_rad.is_finite() || angle_rad.to_degrees().abs() <= 1e-9 {
            return;
        }
        let quat = DQuat::from_axis_angle(self.plane_normal, angle_rad);

        if self.is_copy_mode {
            for &vid in &self.target_vertices {
                let old_pos = scene.mesh.vertices[vid].position;
                let rel = old_pos - center;
                let new_pos = center + quat * rel;
                scene.mesh.add_vertex(new_pos);
            }
        } else {
            for &vid in &self.target_vertices {
                let old_pos = scene.mesh.vertices[vid].position;
                let rel = old_pos - center;
                scene.mesh.vertices[vid].position = center + quat * rel;
            }
            let rot_mat = DMat4::from_translation(center)
                * DMat4::from_quat(quat)
                * DMat4::from_translation(-center);
            for &gid in &self.target_groups {
                if gid < scene.groups.len() {
                    scene.groups[gid].transform = rot_mat * scene.groups[gid].transform;
                }
            }
        }

        if !self.is_copy_mode && !self.original_images.is_empty() {
            scene.image_objects.publish(&scene.image_planes);
            let command = ingetrazo_core::image_plane::ImageFrameCommand {
                images: self
                    .original_images
                    .iter()
                    .map(|(_, image)| image.clone())
                    .collect(),
                operation: ingetrazo_core::image_plane::ImageFrameOperation::Rotate {
                    centre: center,
                    axis: self.plane_normal,
                    degrees: angle_rad.to_degrees(),
                },
            };
            command.apply(scene.image_planes.poses_mut(), &scene.image_objects, true);
            self.image_command = Some(command);
        }
        // Original copy builders exclude section planes.
        if !self.is_copy_mode {
            for &id in &self.target_planes {
                if let Some(plane) = scene.section_planes.get_mut(id) {
                    plane.point = center + quat * (plane.point - center);
                    plane.normal = (quat * plane.normal).normalize();
                }
            }
        }
        self.last_angle_rad = Some(angle_rad);
        scene.bump_version();
    }
}

impl RotateTool {
    fn refresh_image_membership(&mut self, scene: &Scene) {
        if self.image_epoch == scene.image_planes.membership_epoch() {
            return;
        }
        self.image_epoch = scene.image_planes.membership_epoch();
        self.image_membership_rebuilds += 1;
        self.image_membership_scanned += scene.image_planes.len() as u64;
        self.image_indices.clear();
        self.image_indices.extend(
            scene
                .image_planes
                .iter()
                .enumerate()
                .map(|(i, p)| (p.id.as_u128(), i)),
        );
    }
    fn restore_images_cold(&self, scene: &mut Scene) {
        if !self.image_snapshot_taken {
            return;
        }
        // Snapshot/history restoration is cold and may receive a new list owner.
        let indices = scene
            .image_planes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_u128(), i))
            .collect::<HashMap<_, _>>();
        for (_, image) in &self.original_images {
            let frame = [image.origin, image.u, image.v];
            if let Some(&index) = indices.get(&image.id.as_u128()) {
                scene.image_planes.pose_mut(index).unwrap().set_frame(frame);
            } else {
                scene.image_objects.set_held_frame(image, frame);
            }
        }
    }
    fn restore_images(&mut self, scene: &mut Scene) {
        if !self.image_snapshot_taken { return; }
        self.refresh_image_membership(scene);
        for (_, image) in &self.original_images {
            let frame = [image.origin,image.u,image.v];
            if let Some(&index) = self.image_indices.get(&image.id.as_u128()) {
                scene.image_planes.pose_mut(index).unwrap().set_frame(frame);
            } else { scene.image_objects.set_held_frame(image,frame); }
        }
    }
    fn pose_images(&mut self, scene: &mut Scene, point: DVec3) {
        if self.is_copy_mode {
            return;
        }
        let Some(center) = self.center else { return };
        if self.ref_arm.is_none() {
            return;
        }
        let degrees = self.compute_angle(point).to_degrees();
        if !degrees.is_finite() {
            return;
        }
        self.refresh_image_membership(scene);
        if !self.image_snapshot_taken {
            for (_, image) in &mut self.original_images {
                let frame = if let Some(&index) = self.image_indices.get(&image.id.as_u128()) {
                    let p = &scene.image_planes[index];
                    [p.origin, p.u, p.v]
                } else {
                    scene.image_objects.held_frame(image)
                };
                [image.origin, image.u, image.v] = frame;
            }
            self.image_snapshot_taken = true;
        }
        let matrix =
            ingetrazo_core::image_plane::ImageRotation::new(center, self.plane_normal, degrees);
        let mut changed = false;
        for (_, image) in &self.original_images {
            let frame = matrix.frame(image);
            if let Some(&index) = self.image_indices.get(&image.id.as_u128()) {
                let mut live = scene.image_planes.pose_mut(index).unwrap();
                changed |= [live.origin, live.u, live.v] != frame;
                live.set_frame(frame);
            } else {
                scene.image_objects.set_held_frame(image, frame);
            }
        }
        if changed {
            self.preview_version = scene.version;
        }
    }
}
impl Tool for RotateTool {
    fn preserves_global_preview_on_context_exit(&self) -> bool {
        // Original end_group_edit leaves the active Rotate and its global
        // image object snapshot alive. Local geometry requires separate
        // detached-object ownership rather than index rebinding.
        self.center.is_some() && !self.is_copy_mode && !self.original_images.is_empty()
            && self.target_vertices.is_empty() && self.target_groups.is_empty()
            && self.target_planes.is_empty()
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.center
    }
    fn visit_image_metadata_roots(
        &self,
        visit: &mut dyn FnMut(&ingetrazo_core::image_plane::ImagePlane),
    ) {
        for (_, image) in &self.original_images {
            visit(image);
        }
    }
    fn take_image_frame_command(
        &mut self,
    ) -> Option<ingetrazo_core::image_plane::ImageFrameCommand> {
        self.image_command.take()
    }
    fn restore_transient_model(&self, scene: &mut Scene) {
        if self.center.is_some() {
            self.restore_images_cold(scene);
            scene.version = self.original_version;
        }
    }
    fn end_transient_before_history(&mut self, scene: &mut Scene) {
        if self.center.is_some() {
            self.on_cancel(scene);
        }
    }
    fn restore_current_preview_model(&self, scene: &mut Scene) {
        if let (Some(center), Some(hover)) = (self.center, self.hover_point) {
            if self.image_snapshot_taken && self.ref_arm.is_some() && !self.is_copy_mode {
                let degrees = self.compute_angle(hover).to_degrees();
                let indices = scene
                    .image_planes
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.id.as_u128(), i))
                    .collect::<HashMap<_, _>>();
                let matrix = ingetrazo_core::image_plane::ImageRotation::new(
                    center,
                    self.plane_normal,
                    degrees,
                );
                for (_, image) in &self.original_images {
                    let frame = matrix.frame(image);
                    if let Some(&index) = indices.get(&image.id.as_u128()) {
                        scene.image_planes.pose_mut(index).unwrap().set_frame(frame);
                    } else {
                        scene.image_objects.set_held_frame(image, frame);
                    }
                }
                scene.version = self.preview_version;
            }
        }
    }

    fn note_control_state(&mut self, held: bool) {
        if held && !self.control_down {
            self.is_copy_mode = !self.is_copy_mode;
        }
        self.control_down = held;
    }
    fn on_control_state(&mut self, scene: &mut Scene, held: bool) {
        let was_copy = self.is_copy_mode;
        self.note_control_state(held);
        if was_copy == self.is_copy_mode { return; }
        if self.is_copy_mode {
            self.restore_images(scene);
        } else if let Some(hover) = self.hover_point {
            self.pose_images(scene, hover);
        }
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Rotate
    }

    fn name(&self) -> &'static str {
        "Rotate"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("Q")
    }

    fn description(&self) -> &'static str {
        "Turn the selection around a centre with the protractor; Ctrl leaves a copy behind."
    }

    fn vcb_label(&self) -> &'static str {
        "Angle"
    }

    fn on_activate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_deactivate(&mut self, scene: &mut Scene) {
        self.restore_images(scene);
        self.reset();
    }

    fn on_cancel(&mut self, scene: &mut Scene) {
        self.restore_images(scene);
        self.reset();
    }

    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
        self.note_control_state(ctx.ctrl_held);
        self.pose_images(scene, ctx.world);
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.note_control_state(ctx.ctrl_held);

        if self.center.is_none() {
            self.gather_targets(scene, ctx);
            if !self.target_vertices.is_empty()
                || !self.target_groups.is_empty()
                || !self.target_planes.is_empty()
                || !self.target_images.is_empty()
            {
                self.center = Some(ctx.world);
            }
        } else if self.ref_arm.is_none() {
            if (ctx.world - self.center.unwrap()).length() >= 1e-6 {
                self.ref_arm = Some(ctx.world);
            }
        } else {
            let angle = self.compute_angle(ctx.world);
            self.execute_rotation(angle, scene);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let deg=ingetrazo_core::units::vcb::parse_angle(text)?;
        let rad = deg * PI / 180.0;
        self.execute_rotation(rad, scene);
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn factory_rotation_uses_source_default_z_axis() {
        let mut scene = Scene::new();
        let v = scene.mesh.add_vertex(DVec3::X);
        scene.select(SelectionItem::Vertex(v));
        let mut tool = crate::base::create_tool(ToolType::Rotate);
        for point in [DVec3::ZERO, DVec3::X, DVec3::Y] {
            tool.on_click(
                &mut scene,
                &ToolContext::new(point, Ray::new(DVec3::Z * 10., -DVec3::Z)),
            );
        }
        assert!((scene.mesh.vertices[v].position - DVec3::Y).length() < 1e-12);
    }

    #[test]
    fn test_rotate_tool_90_degrees() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = RotateTool::new();
        // Click 1: center at origin
        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Click 2: ref arm along X
        let ctx1 = ToolContext::new(DVec3::new(1.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Click 3: target arm along Y (90 degrees)
        let ctx2 = ToolContext::new(DVec3::new(0.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx2);

        let final_pos = scene.mesh.vertices[v0].position;
        assert!((final_pos - DVec3::new(0.0, 1.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn test_rotate_tool_vcb_degrees() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = RotateTool::new();
        let ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        // Rotate 180 deg via VCB
        assert!(tool.on_vcb_input(&mut scene, "180.0").is_ok());

        let final_pos = scene.mesh.vertices[v0].position;
        assert!((final_pos - DVec3::new(-2.0, 0.0, 0.0)).length() < 1e-4);
    }
}
