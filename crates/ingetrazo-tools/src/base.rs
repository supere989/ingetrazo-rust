// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Base traits and contexts for IngeTrazo interactive modeling tools.

use glam::DVec3;
use ingetrazo_core::math::Ray;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_core::scene::Scene;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolType {
    Select,
    Line,
    Freehand,
    Rectangle,
    RotatedRectangle,
    Circle,
    Polygon,
    Arc,
    PushPull,
    Offset,
    Move,
    Rotate,
    Scale,
    Tape,
    Eraser,
    Paint,
    Fillet,
    FollowMe,
    Protractor,
    Dimension,
    Text,
    SectionPlane,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub world: DVec3,
    /// Logical viewport pixels; absent for headless world-coordinate callers.
    pub screen: Option<glam::DVec2>,
    pub ray: Ray,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub alt_held: bool,
}

impl ToolContext {
    pub fn new(world: DVec3, ray: Ray) -> Self {
        Self {
            world,
            screen: None,
            ray,
            shift_held: false,
            ctrl_held: false,
            alt_held: false,
        }
    }

    pub fn with_modifiers(
        world: DVec3,
        ray: Ray,
        shift_held: bool,
        ctrl_held: bool,
        alt_held: bool,
    ) -> Self {
        Self {
            world,
            screen: None,
            ray,
            shift_held,
            ctrl_held,
            alt_held,
        }
    }
}

/// Immutable source sweep. The renderer borrows the current mesh, validates
/// face identity/revision, and derives cap/wall vertices from offset. Suppress
/// the base only when attached and offset points away from the camera; prism
/// caps instead translate the existing cap/walls. No document mutation.
#[derive(Debug, Clone, Copy)]
pub struct FaceSweepPreview {
    pub face: usize,
    pub face_identity: u64,
    pub mesh_revision: u64,
    pub anchor: DVec3,
    pub normal: DVec3,
    pub offset: DVec3,
    pub attached: bool,
    pub prism_cap: bool,
    pub keep_base: bool,
}

/// Borrowed groups-only translation preview for a placement-aware renderer.
/// Scene already contains visible transforms; never suppress revision invalidation.
/// Indices address the active editing Scene, including component-local scenes.
/// Consumers validate frozen baseline against grab_version and retire on None.
/// Rendering must preserve scene depth/alpha ordering across cached batches.
#[derive(Debug, Clone, Copy)]
pub struct PlacementPreview<'a> {
    pub grab_version: u64,
    pub groups: &'a [(usize, glam::DMat4)],
    pub world_delta: DVec3,
}

pub trait Tool: Send + Sync {
    /// Consume source bare AddEdge ownership; compound tools keep full snapshots.
    fn take_bare_edge_command(&mut self)->Option<(DVec3,DVec3)>{None}

    /// Faces authored by direct AddFace-style commands (no later topology rebuild).
    fn take_authored_draw_faces(&mut self)->Vec<usize>{Vec::new()}

    /// Source global-only previews survive leaving a group; local targets do not.
    fn preserves_global_preview_on_context_exit(&self)->bool{false}
    /// Called only after a context publication has passed validation.
    fn detach_context_preview(&mut self) {}
    /// Cold metadata collection visits object identities owned by live tools.
    fn visit_image_metadata_roots(&self,_visit:&mut dyn FnMut(&ingetrazo_core::image_plane::ImagePlane)){}
    /// Consume the source relative image command after an actual tool commit.
    fn take_image_frame_command(&mut self)->Option<ingetrazo_core::image_plane::ImageFrameCommand>{None}
    /// Actionable preparation error from the last pointer commit, if any.
    fn error_message(&self) -> Option<&str> {
        None
    }

    /// Informational source tool status, separate from a refused operation.
    fn status_message(&self) -> Option<&str> { None }
    /// Consume one informational event after an actual tool call.
    fn take_status_message(&mut self)->Option<String>{None}

    fn tool_type(&self) -> ToolType;
    fn name(&self) -> &'static str;
    fn shortcut(&self) -> Option<&'static str> {
        None
    }
    fn description(&self) -> &'static str {
        ""
    }
    fn vcb_label(&self) -> &'static str {
        ""
    }
    fn on_activate(&mut self, _scene: &mut Scene) {}
    fn on_deactivate(&mut self, _scene: &mut Scene) {}
    fn on_click(&mut self, _scene: &mut Scene, _ctx: &ToolContext) {}
    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.on_click(scene, ctx);
    }
    fn on_triple_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.on_click(scene, ctx);
    }
    fn on_move(&mut self, _scene: &mut Scene, _ctx: &ToolContext) {}
    fn on_release(&mut self, _scene: &mut Scene, _ctx: &ToolContext, _drag_distance: f64) {}
    /// Update transient tool state before undo. True consumes undo without a document edit.
    fn prepare_undo(&mut self, _available: bool) -> bool { false }
    /// A successful retype replaces the immediately preceding tool transaction.
    fn take_history_replacement(&mut self)->bool{false}
    /// Application transaction identity, independent of restored Scene.version.
    fn note_history_revision(&mut self,_revision:u64,_committed:bool){}
    fn note_pick_viewport(&mut self,_viewport:ingetrazo_core::screen_pick::PixelViewport,_eye:DVec3,_model_revision:u64){}
    fn note_cursor_plane(&mut self,_point:DVec3,_normal:DVec3){}
    fn snap_anchor(&self)->Option<DVec3>{self.rubber_band().map(|(a,_)|a)}
    fn snap_exclusions(&self)->Option<&ingetrazo_core::snap_screen::SnapExclusions>{None}
    fn magnetic_axis_deg(&self)->Option<f64>{None}
    fn screen_axis_px(&self)->Option<f64>{None}
    fn snap_close(&self)->Option<DVec3>{None}
    fn lock_drawing_plane(&mut self,_normal:DVec3)->bool{false}
    /// Consume an inferred-shape lock key without changing model geometry.
    fn toggle_inference_lock(&mut self)->bool{false}
    fn lock_drawing_reference(&mut self,_reference:Option<ingetrazo_core::screen_pick::HoverReference>)->bool{false}
    /// Notify modifier transitions, including frames without pointer movement.
    fn note_control_state(&mut self,_held:bool){}
    fn on_control_state(&mut self,_scene:&mut Scene,held:bool){self.note_control_state(held);}
    /// Recover the grab-time model in an already allocated undo snapshot.
    fn restore_transient_model(&self,_scene:&mut Scene){}
    fn restore_current_preview_model(&self,_scene:&mut Scene){}
    /// Retire a live transform before history can replace its target entities.
    fn end_transient_before_history(&mut self,_scene:&mut Scene){}
    /// Move drags in a vertical plane facing the camera through its grab point.
    fn prefers_vertical_drag(&self) -> bool { false }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> { None }
    /// Pure placement translation only, not mixed geometry, planes or copy.
    /// This borrowed descriptor allocates no model snapshot.
    fn placement_preview(&self) -> Option<PlacementPreview<'_>> { None }
    fn face_sweep_preview(&self)->Option<FaceSweepPreview>{None}
    fn preview_mesh(&self) -> Option<&Mesh> {
        None
    }
    fn value_label(&self) -> Option<(String, DVec3)> {
        None
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        self.rubber_band().into_iter().collect()
    }
    fn on_cancel(&mut self, _scene: &mut Scene) {}
    fn on_vcb_input(&mut self, _scene: &mut Scene, _text: &str) -> Result<(), String> {
        Ok(())
    }
    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        None
    }
}

pub use ingetrazo_core::math::ray_triangle_intersect;

/// Pick the closest face intersected by the ray.
pub fn pick_face(mesh: &Mesh, ray: &Ray) -> Option<(usize, f64, DVec3)> {
    let mut closest = None;
    let mut min_t = f64::INFINITY;
    for (face_id, _) in mesh.faces.iter().enumerate() {
        let tris = mesh.triangulate_face(face_id);
        for (v0, v1, v2) in tris {
            if let Some((t, pt)) = ray_triangle_intersect(ray, v0, v1, v2) {
                if t < min_t {
                    min_t = t;
                    closest = Some((face_id, t, pt));
                }
            }
        }
    }
    closest
}

/// Scene-aware picking excludes the side hidden by the document cut.
pub fn pick_scene_face(scene: &Scene, ray: &Ray) -> Option<(usize, f64, DVec3)> {
    let mut closest = None;
    let mut min_t = f64::INFINITY;
    for id in 0..scene.mesh.faces.len() {
        if !scene.mesh_face_selectable(&scene.mesh,id){continue;}
        for (a, b, c) in scene.mesh.triangulate_face(id) {
            if let Some((t, p)) = ray_triangle_intersect(ray, a, b, c) {
                if t < min_t && scene.section_point_visible(p) {
                    min_t = t;
                    closest = Some((id, t, p));
                }
            }
        }
    }
    closest
}
pub fn pick_scene_edge(scene: &Scene, ray: &Ray, tolerance: f64) -> Option<usize> {
    let mut closest = None;
    let mut min_dist = tolerance;
    for (id, edge) in scene.mesh.edges.iter().enumerate() {
        if !scene.mesh_edge_selectable(&scene.mesh,id){continue;}
        let Some((a, b)) = scene.section_segment(
            scene.mesh.vertices[edge.v0].position,
            scene.mesh.vertices[edge.v1].position,
        ) else {
            continue;
        };
        let (dist, t) = ray_segment_distance(ray, a, b);
        if dist < min_dist && t > 0. {
            min_dist = dist;
            closest = Some(id);
        }
    }
    closest
}
pub fn pick_scene_vertex(scene: &Scene, ray: &Ray, tolerance: f64) -> Option<usize> {
    let mut closest = None;
    let mut min_dist = tolerance;
    for (id, vertex) in scene.mesh.vertices.iter().enumerate() {
        if !scene.mesh_vertex_selectable(&scene.mesh,id){continue;}
        let p = vertex.position;
        if !scene.section_point_visible(p) {
            continue;
        }
        let t = (p - ray.origin).dot(ray.dir);
        let d = (p - ray.at(t)).length();
        if t > 0. && d < min_dist {
            min_dist = d;
            closest = Some(id);
        }
    }
    closest
}

/// Distance and projection from point `p` to segment `ab`.
pub fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> (f64, DVec3) {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 {
        return ((p - a).length(), a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let proj = a + ab * t;
    ((p - proj).length(), proj)
}

/// Ray to segment distance.
pub fn ray_segment_distance(ray: &Ray, a: DVec3, b: DVec3) -> (f64, f64) {
    let u = ray.dir;
    let v = b - a;
    let w = ray.origin - a;
    let a_dot = u.dot(u); // 1.0
    let b_dot = u.dot(v);
    let c_dot = v.dot(v);
    let d_dot = u.dot(w);
    let e_dot = v.dot(w);
    let denom = a_dot * c_dot - b_dot * b_dot;

    let s = if denom.abs() < 1e-9 {
        0.0
    } else {
        ((a_dot * e_dot - b_dot * d_dot) / denom).clamp(0.0, 1.0)
    };

    let t = (b_dot * s - d_dot) / a_dot;
    let t_clamped = t.max(0.0);

    let pt_ray = ray.origin + u * t_clamped;
    let pt_seg = a + v * s;
    ((pt_ray - pt_seg).length(), t_clamped)
}

/// Pick the closest edge within screen/world tolerance.
pub fn pick_edge(mesh: &Mesh, ray: &Ray, tolerance: f64) -> Option<usize> {
    let mut closest = None;
    let mut min_dist = tolerance;
    for (edge_id, edge) in mesh.edges.iter().enumerate() {
        let p0 = mesh.vertices[edge.v0].position;
        let p1 = mesh.vertices[edge.v1].position;
        let (dist, t_ray) = ray_segment_distance(ray, p0, p1);
        if dist < min_dist && t_ray > 0.0 {
            min_dist = dist;
            closest = Some(edge_id);
        }
    }
    closest
}

/// Pick the closest vertex within screen/world tolerance.
pub fn pick_vertex(mesh: &Mesh, ray: &Ray, tolerance: f64) -> Option<usize> {
    let mut closest = None;
    let mut min_dist = tolerance;
    for (vert_id, vert) in mesh.vertices.iter().enumerate() {
        let p = vert.position;
        let diff = p - ray.origin;
        let t = diff.dot(ray.dir);
        if t > 0.0 {
            let pt_on_ray = ray.origin + ray.dir * t;
            let dist = (p - pt_on_ray).length();
            if dist < min_dist {
                min_dist = dist;
                closest = Some(vert_id);
            }
        }
    }
    closest
}

/// Factory to construct a tool instance for any ToolType.
pub fn create_tool(tool_type: ToolType) -> Box<dyn Tool> {
    match tool_type {
        ToolType::Select => Box::new(crate::select::SelectTool::default()),
        ToolType::Line => Box::new(crate::line::LineTool::default()),
        ToolType::Freehand => Box::new(crate::freehand::FreehandTool::default()),
        ToolType::Rectangle => Box::new(crate::rectangle::RectangleTool::default()),
        ToolType::RotatedRectangle => {
            Box::new(crate::rotated_rectangle::RotatedRectangleTool::default())
        }
        ToolType::Circle => Box::new(crate::circle::CircleTool::default()),
        ToolType::Polygon => Box::new(crate::polygon::PolygonTool::default()),
        ToolType::Arc => Box::new(crate::arc::ArcTool::default()),
        ToolType::PushPull => Box::new(crate::pushpull::PushPullTool::default()),
        ToolType::Offset => Box::new(crate::offset::OffsetTool::default()),
        ToolType::Move => Box::new(crate::move_tool::MoveTool::default()),
        ToolType::Rotate => Box::new(crate::rotate_tool::RotateTool::default()),
        ToolType::Scale => Box::new(crate::scale_tool::ScaleTool::default()),
        ToolType::Tape => Box::new(crate::tape::TapeTool::default()),
        ToolType::Eraser => Box::new(crate::eraser::EraserTool::default()),
        ToolType::Paint => Box::new(crate::paint::PaintTool::default()),
        ToolType::Fillet => Box::new(crate::fillet::FilletTool::default()),
        ToolType::FollowMe => Box::new(crate::followme::FollowMeTool::default()),
        ToolType::Protractor => Box::new(crate::protractor::ProtractorTool::default()),
        ToolType::Dimension => Box::new(crate::dimension::DimensionTool::default()),
        ToolType::Text => Box::new(crate::text::TextTool::default()),
        ToolType::SectionPlane => Box::new(crate::section::SectionPlaneTool::default()),
    }
}

#[cfg(test)]
mod section_pick_tests {
    use super::*;
    use ingetrazo_core::section::SectionPlane;
    #[test]
    fn hidden_front_face_does_not_mask_visible_rear_face() {
        let mut scene = Scene::new();
        for z in [1., -1.] {
            let ids = [
                DVec3::new(-1., -1., z),
                DVec3::new(1., -1., z),
                DVec3::new(0., 1., z),
            ]
            .map(|p| scene.mesh.add_vertex(p));
            scene.mesh.add_face(ids.to_vec(), vec![]);
        }
        scene
            .section_planes
            .push(SectionPlane::new(DVec3::ZERO, DVec3::Z, "A", "A"));
        scene.show_section_cuts = true;
        let ray = Ray::new(DVec3::new(0., 0., 3.), -DVec3::Z);
        assert_eq!(pick_scene_face(&scene, &ray).unwrap().0, 1);
        scene.show_section_cuts = false;
        assert_eq!(pick_scene_face(&scene, &ray).unwrap().0, 0);
    }
    #[test]
    fn crossing_edge_can_only_be_picked_on_visible_portion() {
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(-1., 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(1., 0., 0.));
        scene.mesh.add_edge(a, b);
        scene
            .section_planes
            .push(SectionPlane::new(DVec3::ZERO, DVec3::X, "A", "A"));
        scene.show_section_cuts = true;
        assert_eq!(
            pick_scene_edge(&scene, &Ray::new(DVec3::new(-0.5, 0., 2.), -DVec3::Z), 0.1),
            Some(0)
        );
        assert_eq!(
            pick_scene_edge(&scene, &Ray::new(DVec3::new(0.5, 0., 2.), -DVec3::Z), 0.1),
            None
        );
        assert_eq!(
            pick_scene_vertex(&scene, &Ray::new(DVec3::new(1., 0., 2.), -DVec3::Z), 0.1),
            None
        );
    }
}

pub fn pick_section_plane(scene: &Scene, ray: &Ray, tolerance: f64) -> Option<usize> {
    if !scene.show_section_planes {
        return None;
    }
    let mut closest = None;
    let mut distance = tolerance;
    for id in 0..scene.section_planes.len() {
        let frame = scene.section_frame(id)?;
        for edge in 0..4 {
            let (d, t) = ray_segment_distance(ray, frame[edge], frame[(edge + 1) % 4]);
            if d < distance && t > 0. {
                distance = d;
                closest = Some(id);
            }
        }
    }
    closest
}

#[cfg(test)]
mod section_entity_tests {
    use super::*;
    use ingetrazo_core::{scene::SelectionItem, section::SectionPlane};
    #[test]
    fn plane_frame_select_move_rotate_preserve_uid_and_leave_mesh_untouched() {
        let mut scene = Scene::new();
        scene
            .section_planes
            .push(SectionPlane::new(DVec3::ZERO, DVec3::Z, "A", "A"));
        scene.show_section_planes = true;
        let uid = scene.section_planes[0].id;
        let frame = scene.section_frame(0).unwrap();
        let point = (frame[0] + frame[1]) * 0.5;
        let ctx = ToolContext::new(point, Ray::new(point + DVec3::Z * 3., -DVec3::Z));
        let mut select = crate::select::SelectTool::new();
        select.on_click(&mut scene, &ctx);
        assert!(scene.selection.contains(&SelectionItem::SectionPlane(0)));
        let mut movement = crate::move_tool::MoveTool::new();
        movement.gather_targets(&scene, &ctx);
        movement.execute_move(DVec3::X, &mut scene);
        assert_eq!(scene.section_planes[0].point, DVec3::X);
        let mut rotation = crate::rotate_tool::RotateTool::with_normal(DVec3::Y);
        rotation.gather_targets(&scene, &ctx);
        rotation.center = Some(DVec3::ZERO);
        rotation.execute_rotation(std::f64::consts::FRAC_PI_2, &mut scene);
        assert!((scene.section_planes[0].point + DVec3::Z).length() < 1e-10);
        assert!((scene.section_planes[0].normal - DVec3::X).length() < 1e-10);
        assert_eq!(scene.section_planes[0].id, uid);
        assert!(scene.mesh.vertices.is_empty());
        let before = scene.clone();
        movement.is_copy_mode = true;
        movement.execute_move(DVec3::Y, &mut scene);
        rotation.is_copy_mode = true;
        rotation.execute_rotation(0.5, &mut scene);
        assert_eq!(scene, before);
        scene.show_section_planes = false;
        assert!(pick_section_plane(&scene, &ctx.ray, 0.2).is_none());
    }
}

/// Root placement containing the nearest visible nested group geometry.
pub fn pick_scene_group(scene: &Scene, ray: &Ray, tolerance: f64) -> Option<(usize, f64)> {
    fn distance(
        group: &ingetrazo_core::groups::Group,
        parent: glam::DMat4,
        scene: &Scene,
        ray: &Ray,
        tolerance: f64,
        best: &mut f64,
    ) {
        if !scene.group_selectable(group) {
            return;
        }
        // The source closed-group picker applies the placement layer; internal
        // face/edge tags acquire their own meaning after entering that context.
        let transform = parent * group.transform;
        for (id, face) in group.mesh.faces.iter().enumerate() {
            if face.attrs.get("hidden").and_then(|v| v.as_bool()) == Some(true)
            {
                continue;
            }
            for (a, b, c) in group.mesh.triangulate_face(id) {
                if let Some((t, p)) = ray_triangle_intersect(
                    ray,
                    transform.transform_point3(a),
                    transform.transform_point3(b),
                    transform.transform_point3(c),
                ) {
                    if t < *best && scene.section_point_visible(p) {
                        *best = t;
                    }
                }
            }
        }
        for edge in &group.mesh.edges {
            if edge.hidden {
                continue;
            }
            if !edge.faces.is_empty()
                && !edge.faces.iter().any(|&id| {
                    group.mesh.faces.get(id).is_some_and(|face| {
                        face.attrs.get("hidden").and_then(|v| v.as_bool()) != Some(true)
                    })
                })
            {
                continue;
            }
            let a = transform.transform_point3(group.mesh.vertices[edge.v0].position);
            let b = transform.transform_point3(group.mesh.vertices[edge.v1].position);
            if let Some((a, b)) = scene.section_segment(a, b) {
                let (d, t) = ray_segment_distance(ray, a, b);
                if d < tolerance && t > 0. && t < *best {
                    *best = t;
                }
            }
        }
        for child in &group.children {
            distance(child, transform, scene, ray, tolerance, best);
        }
    }
    let mut closest = None;
    let mut best = f64::INFINITY;
    for (id, group) in scene.groups.iter().enumerate() {
        let mut d = f64::INFINITY;
        distance(group, glam::DMat4::IDENTITY, scene, ray, tolerance, &mut d);
        if d < best {
            best = d;
            closest = Some((id, d));
        }
    }
    closest
}

#[cfg(test)]
mod group_visibility_tests {
    use super::*;
    use ingetrazo_core::{groups::Group, layers::Layer, mesh::Mesh};
    #[test]
    fn hidden_faces_and_placement_layers_control_closed_group_picks() {
        let mut mesh = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]);
        let mut scene = Scene::new();
        scene.groups.push(Group::with_mesh("part", mesh));
        let boundary = Ray::new(DVec3::new(0.5, 0., 3.), -DVec3::Z);
        assert!(pick_scene_group(&scene, &boundary, 0.1).is_some());
        scene.groups[0].mesh.faces[0]
            .attrs
            .insert("hidden".into(), serde_json::json!(true));
        assert!(pick_scene_group(&scene, &boundary, 0.1).is_none());
        scene.groups[0].mesh.faces[0].attrs.remove("hidden");
        scene.groups[0].mesh.faces[0]
            .attrs
            .insert("layer".into(), serde_json::json!("Hidden"));
        let mut layer = Layer::new("Hidden");
        layer.visible = false;
        scene.layers.push(layer);
        // The actual source picker ignores internal tags until context entry;
        // see the group-face-named and group-named original viewport controls.
        assert!(pick_scene_group(&scene, &boundary, 0.1).is_some());
        scene.groups[0].layer = Some("Hidden".into());
        assert!(pick_scene_group(&scene, &boundary, 0.1).is_none());
        scene.groups[0].layer = None;
        let a = scene.groups[0].mesh.add_vertex(DVec3::new(2., 0., 0.));
        let b = scene.groups[0].mesh.add_vertex(DVec3::new(3., 0., 0.));
        scene.groups[0].mesh.add_edge(a, b);
        assert!(
            pick_scene_group(&scene, &Ray::new(DVec3::new(2.5, 0., 3.), -DVec3::Z), 0.1).is_some()
        );
    }
}

/// Source gather_images keeps explicit selection even when subsequently locked
/// or hidden; only implicit cursor acquisition applies selectable guards.
pub fn gather_image_targets(scene:&Scene,ctx:&ToolContext)->Vec<usize>{
    let selected=scene.selection.iter().filter_map(|item|match item{ingetrazo_core::scene::SelectionItem::ImagePlane(id) if *id<scene.image_planes.len()=>Some(*id),_=>None}).collect::<Vec<_>>();
    if selected.is_empty()&&scene.selection.is_empty(){scene.image_plane_hit(&ctx.ray,ingetrazo_core::image_plane::ImagePickMode::Selectable).map(|(id,_)|vec![id]).unwrap_or_default()}else{selected}
}

#[cfg(test)] mod image_acquisition_tests{
 use super::*;use ingetrazo_core::scene::SelectionItem;
 #[test] fn selected_hidden_locked_images_keep_source_target_ownership(){
  let mut scene=Scene::new();let mut image=ingetrazo_core::image_plane::ImagePlane::new("scan",DVec3::ZERO,DVec3::X,DVec3::Y);image.locked=true;scene.image_planes.push(image);
  let ctx=ToolContext::new(DVec3::ZERO,ingetrazo_core::math::Ray::new(DVec3::new(0.5,0.5,1.),-DVec3::Z));
  assert!(gather_image_targets(&scene,&ctx).is_empty());scene.select(SelectionItem::ImagePlane(0));assert_eq!(gather_image_targets(&scene,&ctx),vec![0]);
 }
}
