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
    pub ray: Ray,
    pub shift_held: bool,
    pub ctrl_held: bool,
    pub alt_held: bool,
}

impl ToolContext {
    pub fn new(world: DVec3, ray: Ray) -> Self {
        Self {
            world,
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
            ray,
            shift_held,
            ctrl_held,
            alt_held,
        }
    }
}

pub trait Tool: Send + Sync {
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
    fn on_cancel(&mut self, _scene: &mut Scene) {}
    fn on_vcb_input(&mut self, _scene: &mut Scene, _text: &str) -> Result<(), String> {
        Ok(())
    }
    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        None
    }
}

/// Möller–Trumbore ray-triangle intersection test.
pub fn ray_triangle_intersect(ray: &Ray, v0: DVec3, v1: DVec3, v2: DVec3) -> Option<(f64, DVec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = ray.dir.cross(edge2);
    let a = edge1.dot(h);
    if a.abs() < 1e-9 {
        return None;
    }
    let f = 1.0 / a;
    let s = ray.origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(edge1);
    let v = f * ray.dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * edge2.dot(q);
    if t > 1e-9 {
        Some((t, ray.at(t)))
    } else {
        None
    }
}

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
        ToolType::RotatedRectangle => Box::new(crate::rotated_rectangle::RotatedRectangleTool::default()),
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
