// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Snapping & Inference Engine (Track 3).
//!
//! Provides SketchUp-parity 3D snapping and dynamic inferences:
//! - Priority-ordered snap targets:
//!   1. Axis Lock / Line Lock (Arrow keys or Shift inference lock)
//!   2. Close loop
//!   3. Endpoint (Green square)
//!   4. Intersection (Crossing X)
//!   5. Midpoint (Cyan circle)
//!   6. Center (Circle/Arc center circle)
//!   7. Construction Guide line / point
//!   8. On Edge (Red diamond)
//!   9. "From Point" tracking memory lines (along X/Y/Z from hovered point)
//!   10. "Two-Point From Point" dual-axis ray crossing
//!   11. Parallel & Perpendicular reference edge inferences (Magenta)
//!   12. Axis inferences (X red, Y green, Z blue)
//!   13. On Face (Blue diamond)
//!   14. World Origin (Orange target)

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::math::{Plane, Ray, EPSILON};
use crate::scene::Scene;

/// Original first-point fallback: standard views draw through model center;
/// oblique views keep their face/ground fallback.
pub fn first_point_work_plane(forward:DVec3,center:DVec3)->Option<(DVec3,DVec3)>{
    if !forward.is_finite()||!center.is_finite()||forward.length()<1e-9{return None;}let f=forward.normalize();if f.abs().max_element()<0.97{return None;}let normal=if f.x.abs()>=f.y.abs()&&f.x.abs()>=f.z.abs(){DVec3::X*f.x.signum()}else if f.y.abs()>=f.z.abs(){DVec3::Y*f.y.signum()}else{DVec3::Z*f.z.signum()};Some((center,normal))
}

// --- Colors for Snap Glyphs and Guides ---
pub const COLOR_ENDPOINT: [f32; 3] = [0.16, 0.62, 0.36];    // Green
pub const COLOR_MIDPOINT: [f32; 3] = [0.20, 0.66, 0.74];    // Cyan
pub const COLOR_INTERSECTION: [f32; 3] = [0.15, 0.15, 0.15];// Black/Dark cross
pub const COLOR_ON_EDGE: [f32; 3] = [0.86, 0.22, 0.27];     // Red
pub const COLOR_ON_FACE: [f32; 3] = [0.42, 0.46, 0.92];     // Blue / Violet
pub const COLOR_ORIGIN: [f32; 3] = [0.95, 0.45, 0.16];      // Orange
pub const COLOR_CLOSE: [f32; 3] = [0.20, 0.40, 0.78];       // Blue
pub const COLOR_AXIS_X: [f32; 3] = [0.86, 0.22, 0.27];      // Red
pub const COLOR_AXIS_Y: [f32; 3] = [0.16, 0.62, 0.36];      // Green
pub const COLOR_AXIS_Z: [f32; 3] = [0.20, 0.40, 0.78];      // Blue
pub const COLOR_REFERENCE: [f32; 3] = [0.85, 0.30, 0.80];   // Magenta
pub const COLOR_GUIDE: [f32; 3] = [0.25, 0.50, 0.90];       // Guide Blue
pub const COLOR_CENTER: [f32; 3] = [0.20, 0.66, 0.74];      // Cyan

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapKind {
    Endpoint,
    Midpoint,
    Intersection,
    Center,
    OnEdge,
    OnFace,
    Origin,
    AxisLock,
    AxisInference,
    Reference,
    FromPoint,
    TwoPointFromPoint,
    Close,
    Guide,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapResult {
    pub point: DVec3,
    pub kind: SnapKind,
    pub color: [f32; 3],
    pub label: String,
    pub axis: Option<char>,
    pub guides: Vec<(DVec3, DVec3, [f32; 3])>,
}

impl Default for SnapResult {
    fn default() -> Self {
        Self {
            point: DVec3::ZERO,
            kind: SnapKind::None,
            color: [0.0, 0.0, 0.0],
            label: String::new(),
            axis: None,
            guides: Vec::new(),
        }
    }
}

/// Snapping & inference engine with tracking memory points and reference edge support.
#[derive(Debug, Clone)]
pub struct SnapEngine {
    pub snap_distance: f64,
    pub encouraged_points: Vec<DVec3>,
    pub max_encouraged: usize,
    pub reference_edge: Option<(DVec3, DVec3)>,
    pub locked_axis: Option<char>,
    pub locked_line: Option<(DVec3, DVec3)>,
    pub(crate) center_reference: Option<crate::snap_screen::CenterReference>,
    pub(crate) hovered_center: Option<DVec3>,
    pub(crate) screen_dwell: Option<(DVec3,f64)>,
    pub(crate) acquired_point: Option<DVec3>,
    pub(crate) model_revision: Option<u64>,
    pub(crate) center_cache_dirty: bool,
}

impl Default for SnapEngine {
    fn default() -> Self {
        Self {
            snap_distance: 0.20, // 20 cm in world space
            encouraged_points: Vec::new(),
            max_encouraged: 4,
            reference_edge: None,
            locked_axis: None,
            locked_line: None,
            center_reference: None,
            hovered_center: None,
            screen_dwell: None,
            acquired_point: None,
            model_revision: None,
            center_cache_dirty: false,
        }
    }
}

impl SnapEngine {
    /// Retire operation references while retaining user snapping configuration.
    pub fn reset_operation_state(&mut self) {
        self.encouraged_points.clear();
        self.reference_edge = None;
        self.locked_axis = None;
        self.locked_line = None;
        self.center_reference = None;
        self.hovered_center = None;
        self.screen_dwell = None;
        self.acquired_point = None;
    }

    /// Application identity is monotonic even when restored Scene versions repeat.
    /// Call before screen snapping after document replacement or model mutation.
    pub fn note_model_revision(&mut self, revision: u64) {
        if self.model_revision != Some(revision) {
            self.model_revision = Some(revision);
            self.center_cache_dirty = true;
        }
    }
    pub fn new(snap_distance: f64) -> Self {
        Self {
            snap_distance,
            ..Default::default()
        }
    }

    /// Add a memory tracking point (hovered vertex/corner).
    pub fn encourage_point(&mut self, pt: DVec3) {
        // If already in list near same spot, remove to bump to newest
        self.encouraged_points.retain(|&p| (p - pt).length() > 0.05);
        self.encouraged_points.push(pt);
        if self.encouraged_points.len() > self.max_encouraged {
            self.encouraged_points.remove(0);
        }
    }

    pub fn clear_encouraged(&mut self) {
        self.encouraged_points.clear();
    }

    pub fn set_reference_edge(&mut self, a: DVec3, b: DVec3) {
        self.reference_edge = Some((a, b));
    }

    pub fn clear_reference_edge(&mut self) {
        self.reference_edge = None;
    }

    pub fn lock_axis(&mut self, axis: Option<char>) {
        self.locked_axis = axis;
    }

    pub fn lock_line(&mut self, line: Option<(DVec3, DVec3)>) {
        self.locked_line = line;
    }

    /// Master snap method: evaluates cursor ray against the full scene and active inferences.
    pub fn snap(
        &mut self,
        ray: &Ray,
        scene: &Scene,
        start_point: Option<DVec3>,
        close_point: Option<DVec3>,
    ) -> SnapResult {
        // 1. Explicit Line Lock or Axis Lock
        if let Some((p0, p1)) = self.locked_line {
            let dir = (p1 - p0).normalize_or_zero();
            if dir.length_squared() > EPSILON {
                let pt = closest_point_ray_line(ray, p0, dir);
                return SnapResult {
                    point: pt,
                    kind: SnapKind::AxisLock,
                    color: COLOR_REFERENCE,
                    label: "Locked Line".to_string(),
                    axis: None,
                    guides: vec![(p0 - dir * 100.0, p0 + dir * 100.0, COLOR_REFERENCE)],
                };
            }
        }

        let effective_axis_lock = self.locked_axis;
        if let (Some(ax), Some(start)) = (effective_axis_lock, start_point) {
            let (axis_dir, color) = match ax {
                'x' | 'X' => (DVec3::X, COLOR_AXIS_X),
                'y' | 'Y' => (DVec3::Y, COLOR_AXIS_Y),
                'z' | 'Z' => (DVec3::Z, COLOR_AXIS_Z),
                _ => (DVec3::Z, COLOR_AXIS_Z),
            };
            let pt = closest_point_ray_line(ray, start, axis_dir);
            return SnapResult {
                point: pt,
                kind: SnapKind::AxisLock,
                color,
                label: format!("On {} Axis", ax.to_ascii_uppercase()),
                axis: Some(ax.to_ascii_lowercase()),
                guides: vec![(start - axis_dir * 100.0, start + axis_dir * 100.0, color)],
            };
        }

        // Raycast to work plane (Z = 0 or plane through start)
        let work_z = start_point.map(|p| p.z).unwrap_or(0.0);
        let ground = Plane::new(DVec3::new(0.0, 0.0, work_z), DVec3::Z);
        let cursor_work = ground
            .intersect_ray(ray)
            .map(|t| ray.at(t))
            .unwrap_or_else(|| ray.at(10.0));

        let mut best_dist = self.snap_distance;
        let mut best_result = SnapResult {
            point: cursor_work,
            kind: SnapKind::None,
            color: [0.0, 0.0, 0.0],
            label: String::new(),
            axis: None,
            guides: Vec::new(),
        };

        // 2. Close Loop Snap
        if let Some(close_pt) = close_point {
            let d = (close_pt - cursor_work).length();
            if d < best_dist && scene.section_point_visible(close_pt) {
                best_dist = d;
                best_result = SnapResult {
                    point: close_pt,
                    kind: SnapKind::Close,
                    color: COLOR_CLOSE,
                    label: "Close Loop".to_string(),
                    axis: None,
                    guides: Vec::new(),
                };
            }
        }

        // 3. Endpoint Snaps (Mesh Vertices, Groups, GeoPoints)
        for(id,v)in scene.mesh.vertices.iter().enumerate(){
            if !scene.mesh_vertex_visible(&scene.mesh,id){continue;}
            let d = (v.position - cursor_work).length();
            if d < best_dist && scene.section_point_visible(v.position) {
                best_dist = d;
                best_result = SnapResult {
                    point: v.position,
                    kind: SnapKind::Endpoint,
                    color: COLOR_ENDPOINT,
                    label: "Endpoint".to_string(),
                    axis: None,
                    guides: Vec::new(),
                };
            }
        }
        fn group_points(groups:&[crate::groups::Group],parent:glam::DMat4,scene:&Scene,points:&mut Vec<DVec3>){for group in groups{if group.hidden||!scene.is_layer_visible(group.layer.as_deref()){continue;}let transform=parent*group.transform;points.extend(group.mesh.vertices.iter().enumerate().filter(|(id,_)|scene.mesh_vertex_visible(&group.mesh,*id)).map(|(_,v)|transform.transform_point3(v.position)));children(&group.children,transform,scene,points);}}
        fn children(children:&[crate::groups::Group],parent:glam::DMat4,scene:&Scene,points:&mut Vec<DVec3>){group_points(children,parent,scene,points)}
        let mut group_vertices=vec![];group_points(&scene.groups,glam::DMat4::IDENTITY,scene,&mut group_vertices);
        for wp in group_vertices{let d=(wp-cursor_work).length();if d<best_dist&&scene.section_point_visible(wp){best_dist=d;best_result=SnapResult{point:wp,kind:SnapKind::Endpoint,color:COLOR_ENDPOINT,label:"Endpoint in Group".into(),axis:None,guides:vec![]};}}
        for gp in &scene.geo_points {
            let d = (gp.position - cursor_work).length();
            if d < best_dist && scene.section_point_visible(gp.position) {
                best_dist = d;
                best_result = SnapResult {
                    point: gp.position,
                    kind: SnapKind::Endpoint,
                    color: COLOR_ENDPOINT,
                    label: if gp.name.is_empty() {
                        "Survey Point".to_string()
                    } else {
                        format!("Point {}", gp.name)
                    },
                    axis: None,
                    guides: Vec::new(),
                };
            }
        }

        // Automatically encourage endpoint if very close
        if best_result.kind == SnapKind::Endpoint {
            self.encourage_point(best_result.point);
            return best_result;
        }

        // 4. Intersection Snaps (where mesh edges or guides cross)
        let mut edges = Vec::new();
        for(id,e)in scene.mesh.edges.iter().enumerate(){
            if !scene.mesh_edge_visible(&scene.mesh,id){continue;}
            edges.push((scene.mesh.vertices[e.v0].position, scene.mesh.vertices[e.v1].position));
        }
        for g in &scene.guides {
            match g.kind {
                crate::guides::GuideType::Point(_) => {}
                crate::guides::GuideType::InfiniteLine { point, dir } => {
                    edges.push((point - dir * 50.0, point + dir * 50.0));
                }
                crate::guides::GuideType::Segment { a, b } => {
                    edges.push((a, b));
                }
            }
        }

        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                if let Some(x_pt) = line_segment_intersection_3d(edges[i].0, edges[i].1, edges[j].0, edges[j].1) {
                    let d = (x_pt - cursor_work).length();
                    if d < best_dist && scene.section_point_visible(x_pt) {
                        best_dist = d;
                        best_result = SnapResult {
                            point: x_pt,
                            kind: SnapKind::Intersection,
                            color: COLOR_INTERSECTION,
                            label: "Intersection".to_string(),
                            axis: None,
                            guides: Vec::new(),
                        };
                    }
                }
            }
        }
        if best_result.kind == SnapKind::Intersection {
            return best_result;
        }

        // 5. Midpoint Snaps
        for(id,e)in scene.mesh.edges.iter().enumerate(){
            if !scene.mesh_edge_visible(&scene.mesh,id){continue;}
            let p0 = scene.mesh.vertices[e.v0].position;
            let p1 = scene.mesh.vertices[e.v1].position;
            let mid = (p0 + p1) * 0.5;
            let d = (mid - cursor_work).length();
            if d < best_dist && scene.section_point_visible(mid) {
                best_dist = d;
                best_result = SnapResult {
                    point: mid,
                    kind: SnapKind::Midpoint,
                    color: COLOR_MIDPOINT,
                    label: "Midpoint".to_string(),
                    axis: None,
                    guides: Vec::new(),
                };
            }
        }
        if best_result.kind == SnapKind::Midpoint {
            self.encourage_point(best_result.point);
            return best_result;
        }

        // 6. Arc / Circle Center Snaps
        for(id,f)in scene.mesh.faces.iter().enumerate(){
            if !scene.mesh_face_visible(&scene.mesh,id){continue;}
            if f.vertices.len() >= 6 {
                let pts: Vec<DVec3> = f.vertices.iter().map(|&v| scene.mesh.vertices[v].position).collect();
                if let Some(center) = fit_circle_center(&pts) {
                    let d = (center - cursor_work).length();
                    if d < best_dist && scene.section_point_visible(center) {
                        best_dist = d;
                        best_result = SnapResult {
                            point: center,
                            kind: SnapKind::Center,
                            color: COLOR_CENTER,
                            label: "Center".to_string(),
                            axis: None,
                            guides: Vec::new(),
                        };
                    }
                }
            }
        }
        if best_result.kind == SnapKind::Center {
            return best_result;
        }

        // 7. Construction Guide Snap
        for g in &scene.guides {
            match g.kind {
                crate::guides::GuideType::Point(pt) => {
                    let d = (pt - cursor_work).length();
                    if d < best_dist && scene.section_point_visible(pt) {
                        best_dist = d;
                        best_result = SnapResult {
                            point: pt,
                            kind: SnapKind::Guide,
                            color: COLOR_GUIDE,
                            label: "Guide Point".to_string(),
                            axis: None,
                            guides: Vec::new(),
                        };
                    }
                }
                crate::guides::GuideType::InfiniteLine { point, dir } => {
                    let proj = point + dir * (cursor_work - point).dot(dir);
                    let d = (proj - cursor_work).length();
                    if d < best_dist && scene.section_point_visible(proj) {
                        best_dist = d;
                        best_result = SnapResult {
                            point: proj,
                            kind: SnapKind::Guide,
                            color: COLOR_GUIDE,
                            label: "On Guide Line".to_string(),
                            axis: None,
                            guides: vec![(point - dir * 100.0, point + dir * 100.0, COLOR_GUIDE)],
                        };
                    }
                }
                crate::guides::GuideType::Segment { a, b } => {
                    let ab = b - a;
                    let len_sq = ab.length_squared();
                    if len_sq > EPSILON {
                        let t = ((cursor_work - a).dot(ab) / len_sq).clamp(0.0, 1.0);
                        let proj = a + ab * t;
                        let d = (proj - cursor_work).length();
                        if d < best_dist && scene.section_point_visible(proj) {
                            best_dist = d;
                            best_result = SnapResult {
                                point: proj,
                                kind: SnapKind::Guide,
                                color: COLOR_GUIDE,
                                label: "On Guide Segment".to_string(),
                                axis: None,
                                guides: vec![(a, b, COLOR_GUIDE)],
                            };
                        }
                    }
                }
            }
        }

        // 8. On Edge Snaps
        for(id,e)in scene.mesh.edges.iter().enumerate(){
            if !scene.mesh_edge_visible(&scene.mesh,id){continue;}
            let p0 = scene.mesh.vertices[e.v0].position;
            let p1 = scene.mesh.vertices[e.v1].position;
            let ab = p1 - p0;
            let len_sq = ab.length_squared();
            if len_sq > EPSILON {
                let t = ((cursor_work - p0).dot(ab) / len_sq).clamp(0.0, 1.0);
                let proj = p0 + ab * t;
                let d = (proj - cursor_work).length();
                if d < best_dist && scene.section_point_visible(proj) {
                    best_dist = d;
                    best_result = SnapResult {
                        point: proj,
                        kind: SnapKind::OnEdge,
                        color: COLOR_ON_EDGE,
                        label: "On Edge".to_string(),
                        axis: None,
                        guides: Vec::new(),
                    };
                }
            }
        }

        // 9. Two-Point From Point Crossing (Higher specificity than single axis)
        if self.encouraged_points.len() >= 2 {
            let p_a = self.encouraged_points[self.encouraged_points.len() - 1];
            let p_b = self.encouraged_points[self.encouraged_points.len() - 2];
            // Cross X of p_a with Y of p_b
            let cross1 = DVec3::new(p_a.x, p_b.y, (p_a.z + p_b.z) * 0.5);
            let d1 = (cross1 - cursor_work).length();
            if d1 < best_dist * 0.85 {
                best_dist = d1;
                best_result = SnapResult {
                    point: cross1,
                    kind: SnapKind::TwoPointFromPoint,
                    color: COLOR_ENDPOINT,
                    label: "From Points Crossing".to_string(),
                    axis: None,
                    guides: vec![
                        (p_a, cross1, COLOR_AXIS_Y),
                        (p_b, cross1, COLOR_AXIS_X),
                    ],
                };
            }
            // Cross Y of p_a with X of p_b
            let cross2 = DVec3::new(p_b.x, p_a.y, (p_a.z + p_b.z) * 0.5);
            let d2 = (cross2 - cursor_work).length();
            if d2 < best_dist * 0.85 {
                best_dist = d2;
                best_result = SnapResult {
                    point: cross2,
                    kind: SnapKind::TwoPointFromPoint,
                    color: COLOR_ENDPOINT,
                    label: "From Points Crossing".to_string(),
                    axis: None,
                    guides: vec![
                        (p_a, cross2, COLOR_AXIS_X),
                        (p_b, cross2, COLOR_AXIS_Y),
                    ],
                };
            }
        }

        // 10. Single Encouraged Point: "From Point" Axis Tracking Memory
        if best_result.kind != SnapKind::TwoPointFromPoint {
            for &enc_pt in &self.encouraged_points {
                for &(axis_dir, axis_char, col) in &[
                    (DVec3::X, 'x', COLOR_AXIS_X),
                    (DVec3::Y, 'y', COLOR_AXIS_Y),
                    (DVec3::Z, 'z', COLOR_AXIS_Z),
                ] {
                    let diff = cursor_work - enc_pt;
                    let t = diff.dot(axis_dir);
                    if t.abs() > 0.1 {
                        let proj = enc_pt + axis_dir * t;
                        let d = (proj - cursor_work).length();
                        if d < best_dist * 0.75 && scene.section_point_visible(proj) {
                            best_dist = d;
                            best_result = SnapResult {
                                point: proj,
                                kind: SnapKind::FromPoint,
                                color: COLOR_ENDPOINT,
                                label: format!("From Point ({} Axis)", axis_char.to_ascii_uppercase()),
                                axis: Some(axis_char),
                                guides: vec![(enc_pt, proj, col)],
                            };
                        }
                    }
                }
            }
        }

        // 11. Parallel & Perpendicular Reference Inferences
        if let (Some(start), Some((ra, rb))) = (start_point, self.reference_edge) {
            let ref_dir = (rb - ra).normalize_or_zero();
            if ref_dir.length_squared() > EPSILON {
                // Parallel
                let proj_par = start + ref_dir * (cursor_work - start).dot(ref_dir);
                let d_par = (proj_par - cursor_work).length();
                if d_par < best_dist * 0.75 {
                    best_dist = d_par;
                    best_result = SnapResult {
                        point: proj_par,
                        kind: SnapKind::Reference,
                        color: COLOR_REFERENCE,
                        label: "Parallel to Edge".to_string(),
                        axis: None,
                        guides: vec![(start - ref_dir * 100.0, start + ref_dir * 100.0, COLOR_REFERENCE)],
                    };
                }
                // Perpendicular in horizontal/ground plane
                let ref_perp = DVec3::new(-ref_dir.y, ref_dir.x, 0.0).normalize_or_zero();
                if ref_perp.length_squared() > EPSILON {
                    let proj_perp = start + ref_perp * (cursor_work - start).dot(ref_perp);
                    let d_perp = (proj_perp - cursor_work).length();
                    if d_perp < best_dist * 0.75 {
                        best_dist = d_perp;
                        best_result = SnapResult {
                            point: proj_perp,
                            kind: SnapKind::Reference,
                            color: COLOR_REFERENCE,
                            label: "Perpendicular to Edge".to_string(),
                            axis: None,
                            guides: vec![(start - ref_perp * 100.0, start + ref_perp * 100.0, COLOR_REFERENCE)],
                        };
                    }
                }
            }
        }

        // 12. Dynamic Axis Inferences from start_point
        if let Some(start) = start_point {
            for &(axis_dir, axis_char, col) in &[
                (DVec3::X, 'x', COLOR_AXIS_X),
                (DVec3::Y, 'y', COLOR_AXIS_Y),
                (DVec3::Z, 'z', COLOR_AXIS_Z),
            ] {
                let diff = cursor_work - start;
                let t = diff.dot(axis_dir);
                if t.abs() > 0.1 {
                    let proj = start + axis_dir * t;
                    let d = (proj - cursor_work).length();
                    if d < best_dist * 0.65 && scene.section_point_visible(proj) {
                        best_dist = d;
                        best_result = SnapResult {
                            point: proj,
                            kind: SnapKind::AxisInference,
                            color: col,
                            label: format!("On {} Axis", axis_char.to_ascii_uppercase()),
                            axis: Some(axis_char),
                            guides: vec![(start - axis_dir * 100.0, start + axis_dir * 100.0, col)],
                        };
                    }
                }
            }
        }

        // 13. Origin Snap
        let origin_d = cursor_work.length();
        if origin_d < best_dist {
            best_result = SnapResult {
                point: DVec3::ZERO,
                kind: SnapKind::Origin,
                color: COLOR_ORIGIN,
                label: "Origin".to_string(),
                axis: None,
                guides: Vec::new(),
            };
        }

        best_result
    }
}

// --- Geometric Math Helpers for Snapping ---

fn closest_point_ray_line(ray: &Ray, line_origin: DVec3, line_dir: DVec3) -> DVec3 {
    let diff = ray.origin - line_origin;
    let a = ray.dir.dot(ray.dir);
    let b = ray.dir.dot(line_dir);
    let c = line_dir.dot(line_dir);
    let d = ray.dir.dot(diff);
    let e = line_dir.dot(diff);

    let denom = a * c - b * b;
    let t_line = if denom.abs() > EPSILON {
        (b * d - a * e) / denom
    } else {
        0.0
    };
    line_origin + line_dir * t_line
}

fn line_segment_intersection_3d(a0: DVec3, a1: DVec3, b0: DVec3, b1: DVec3) -> Option<DVec3> {
    let u = a1 - a0;
    let v = b1 - b0;
    let w = a0 - b0;

    let a = u.dot(u);
    let b = u.dot(v);
    let c = v.dot(v);
    let d = u.dot(w);
    let e = v.dot(w);

    let denom = a * c - b * b;
    if denom.abs() < 1e-9 {
        return None; // parallel or colinear
    }

    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;

    if s < 0.0 || s > 1.0 || t < 0.0 || t > 1.0 {
        return None; // outside segments
    }

    let pt_a = a0 + u * s;
    let pt_b = b0 + v * t;

    if (pt_a - pt_b).length() < 0.05 {
        Some((pt_a + pt_b) * 0.5)
    } else {
        None // skew in 3D
    }
}

fn fit_circle_center(pts: &[DVec3]) -> Option<DVec3> {
    if pts.len() < 3 {
        return None;
    }
    let p0 = pts[0];
    let p1 = pts[pts.len() / 3];
    let p2 = pts[pts.len() * 2 / 3];

    let v1 = p1 - p0;
    let v2 = p2 - p0;

    let v1_cross_v2 = v1.cross(v2);
    if v1_cross_v2.length_squared() < 1e-8 {
        return None;
    }

    let v1_sq = v1.dot(v1);
    let v2_sq = v2.dot(v2);

    let num = v1_cross_v2.cross(v1 * v2_sq - v2 * v1_sq);
    let denom = 2.0 * v1_cross_v2.length_squared();

    let offset = num / denom;
    Some(p0 - offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_endpoints_and_midpoints() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(10.0, 0.0, 0.0));
        scene.mesh.add_edge(v0, v1);

        let mut engine = SnapEngine::new(0.5);

        // Near endpoint
        let ray_endpoint = Ray::new(DVec3::new(0.1, 0.1, 5.0), -DVec3::Z);
        let res_ep = engine.snap(&ray_endpoint, &scene, None, None);
        assert_eq!(res_ep.kind, SnapKind::Endpoint);
        assert_eq!(res_ep.point, DVec3::ZERO);

        // Near midpoint
        let ray_midpoint = Ray::new(DVec3::new(5.05, 0.05, 5.0), -DVec3::Z);
        let res_mid = engine.snap(&ray_midpoint, &scene, None, None);
        assert_eq!(res_mid.kind, SnapKind::Midpoint);
        assert!((res_mid.point - DVec3::new(5.0, 0.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn test_encouraged_point_tracking_guide() {
        let scene = Scene::new();
        let mut engine = SnapEngine::new(0.5);
        let ref_pt = DVec3::new(10.0, 10.0, 0.0);
        engine.encourage_point(ref_pt);

        // Move cursor along Y axis from encouraged point: (10, 20)
        let ray = Ray::new(DVec3::new(10.05, 20.0, 5.0), -DVec3::Z);
        let res = engine.snap(&ray, &scene, None, None);

        assert_eq!(res.kind, SnapKind::FromPoint);
        assert_eq!(res.axis, Some('y'));
        assert_eq!(res.guides.len(), 1);
        assert_eq!(res.guides[0].0, ref_pt);
    }

    #[test]
    fn test_parallel_reference_inference() {
        let scene = Scene::new();
        let mut engine = SnapEngine::new(0.5);

        let ref_a = DVec3::new(0.0, 0.0, 0.0);
        let ref_b = DVec3::new(1.0, 1.0, 0.0);
        engine.set_reference_edge(ref_a, ref_b);

        let start = DVec3::new(5.0, 0.0, 0.0);
        // Cursor near parallel line through start
        let ray = Ray::new(DVec3::new(10.05, 5.05, 5.0), -DVec3::Z);
        let res = engine.snap(&ray, &scene, Some(start), None);

        assert_eq!(res.kind, SnapKind::Reference);
        assert!(res.label.contains("Parallel"));
    }

    #[test]
    fn test_two_point_crossing() {
        let scene = Scene::new();
        let mut engine = SnapEngine::new(0.5);
        engine.encourage_point(DVec3::new(10.0, 0.0, 0.0));
        engine.encourage_point(DVec3::new(0.0, 20.0, 0.0));

        let ray = Ray::new(DVec3::new(10.02, 20.03, 5.0), -DVec3::Z);
        let res = engine.snap(&ray, &scene, None, None);

        assert_eq!(res.kind, SnapKind::TwoPointFromPoint);
        assert!((res.point - DVec3::new(10.0, 20.0, 0.0)).length() < 0.1);
        assert_eq!(res.guides.len(), 2);
    }    #[test]fn section_hides_nearest_endpoint_and_reveals_next_candidate(){let mut scene=Scene::new();scene.mesh.add_vertex(DVec3::new(0.02,0.,0.));scene.mesh.add_vertex(DVec3::new(-0.2,0.,0.));scene.section_planes.push(crate::section::SectionPlane::new(DVec3::ZERO,DVec3::X,"A","A"));scene.show_section_cuts=true;let mut engine=SnapEngine::new(0.5);let ray=Ray::new(DVec3::new(0.02,0.,3.),-DVec3::Z);let snapped=engine.snap(&ray,&scene,None,None);assert_eq!(snapped.kind,SnapKind::Endpoint);assert_eq!(snapped.point,DVec3::new(-0.2,0.,0.));scene.show_section_cuts=false;assert_eq!(engine.snap(&ray,&scene,None,None).point,DVec3::new(0.02,0.,0.));}
    #[test]fn nested_group_snap_uses_world_placement_and_active_cut(){let mut scene=Scene::new();let mut mesh=crate::mesh::Mesh::new();mesh.add_vertex(DVec3::new(0.02,0.,0.));mesh.add_vertex(DVec3::new(-0.2,0.,0.));let mut child=crate::groups::Group::with_mesh("child",mesh);child.transform=glam::DMat4::from_translation(-DVec3::X);let mut parent=crate::groups::Group::new("parent");parent.transform=glam::DMat4::from_translation(DVec3::X);parent.children.push(child);scene.groups.push(parent);scene.section_planes.push(crate::section::SectionPlane::new(DVec3::ZERO,DVec3::X,"A","A"));scene.show_section_cuts=true;let mut engine=SnapEngine::new(0.5);let ray=Ray::new(DVec3::new(0.02,0.,3.),-DVec3::Z);let result=engine.snap(&ray,&scene,None,None);assert_eq!(result.kind,SnapKind::Endpoint);assert!((result.point-DVec3::new(-0.2,0.,0.)).length()<1e-10);scene.groups[0].hidden=true;assert_ne!(engine.snap(&ray,&scene,None,None).kind,SnapKind::Endpoint);}
    #[test]fn hidden_face_boundaries_are_not_endpoint_midpoint_or_edge_snap_targets(){let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::Y].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);scene.mesh.faces[0].attrs.insert("hidden".into(),serde_json::json!(true));let mut engine=SnapEngine::new(0.2);for point in[DVec3::ZERO,DVec3::X*0.5]{let result=engine.snap(&Ray::new(point+DVec3::Z*3.,-DVec3::Z),&scene,None,None);assert!(!matches!(result.kind,SnapKind::Endpoint|SnapKind::Midpoint|SnapKind::OnEdge|SnapKind::Intersection|SnapKind::Center));}scene.mesh.faces[0].attrs.remove("hidden");assert_eq!(engine.snap(&Ray::new(DVec3::Z*3.,-DVec3::Z),&scene,None,None).kind,SnapKind::Endpoint);}

}
