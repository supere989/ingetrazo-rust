// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::math::plane_basis;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Rectangle tool for drawing 4-edge planar rectangles and faces.
#[derive(Debug, Clone)]
pub struct RectangleTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub plane_normal: DVec3,
    pub from_center: bool,
    error: Option<String>,
}

impl Default for RectangleTool {
    fn default() -> Self {
        Self {
            start_point: None,
            hover_point: None,
            plane_normal: DVec3::Z, // Default XY ground plane
            from_center: false,
            error: None,
        }
    }
}

impl RectangleTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_normal(normal: DVec3) -> Self {
        Self {
            plane_normal: normal.normalize_or_zero(),
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.hover_point = None;
        self.from_center = false;
        self.plane_normal=DVec3::Z;
    }

    /// Calculate the 4 corners from start point and an opposite point.
    pub fn compute_corners(&self, start: DVec3, far: DVec3) -> Option<[DVec3; 4]> {
        let (u, v) = plane_basis(self.plane_normal);
        let delta = far - start;
        let du = delta.dot(u);
        let dv = delta.dot(v);

        if du.abs() < 1e-6 || dv.abs() < 1e-6 {
            return None;
        }

        if self.from_center {
            let half_u = u * (du.abs());
            let half_v = v * (dv.abs());
            Some([
                start - half_u - half_v,
                start + half_u - half_v,
                start + half_u + half_v,
                start - half_u + half_v,
            ])
        } else {
            Some([
                start,
                start + u * du,
                start + u * du + v * dv,
                start + v * dv,
            ])
        }
    }

    /// Preview the 4 corners during hover.
    pub fn preview_corners(&self) -> Option<[DVec3; 4]> {
        match (self.start_point, self.hover_point) {
            (Some(s), Some(h)) => self.compute_corners(s, h),
            _ => None,
        }
    }

    fn commit_corners(&mut self,corners:[DVec3;4],scene:&mut Scene)->Result<(),String>{
        let segments=(0..4).map(|i|(corners[i],corners[(i+1)%4])).collect::<Vec<_>>();
        scene.mesh=ingetrazo_core::drafting::add_segments(&scene.mesh,&segments,Some(&corners),None)?;
        scene.bump_version();self.reset();self.error=None;Ok(())
    }

}

impl Tool for RectangleTool {
    fn error_message(&self)->Option<&str>{self.error.as_deref()}
    fn drawing_plane(&self)->Option<(DVec3,DVec3)>{self.start_point.map(|p|(p,self.plane_normal))}
    fn tool_type(&self) -> ToolType {
        ToolType::Rectangle
    }

    fn name(&self) -> &'static str {
        "Rectangle"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("R")
    }

    fn description(&self) -> &'static str {
        "Draw a rectangle from two opposite corners."
    }

    fn vcb_label(&self) -> &'static str {
        "Dimensions"
    }

    fn on_activate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
        if ctx.ctrl_held {
            self.from_center = true;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.from_center = ctx.ctrl_held;
        if self.start_point.is_none() {
            if let Some((face,_,_))=crate::base::pick_scene_face(scene,&ctx.ray){self.plane_normal=scene.mesh.face_normal(face);}
            self.start_point = Some(ctx.world);
        } else {
            let start = self.start_point.unwrap();
            if let Some(corners) = self.compute_corners(start, ctx.world) {
                if let Err(error)=self.commit_corners(corners,scene){self.error=Some(error);}
            }
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let Some(start) = self.start_point else {
            return Err("No active start point for rectangle dimensions".into());
        };

        let trimmed = text.trim();
        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        if parts.len() != 2 {
            return Err("Rectangle dimensions format: 'width, height'".into());
        }

        let w: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid width: {e}"))?;
        let h: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid height: {e}"))?;

        if !w.is_finite()||!h.is_finite()||w <= 0. || h <= 0. {
            return Err("Dimensions must be finite and positive".into());
        }

        let (u, v) = plane_basis(self.plane_normal);
        let (sign_u, sign_v) = if let Some(hover) = self.hover_point {
            let d = hover - start;
            (
                if d.dot(u) < 0.0 { -1.0 } else { 1.0 },
                if d.dot(v) < 0.0 { -1.0 } else { 1.0 },
            )
        } else {
            (1.0, 1.0)
        };

        let size=if self.from_center{0.5}else{1.};
        let far = start + u * (w * sign_u * size) + v * (h * sign_v * size);
        if let Some(corners) = self.compute_corners(start, far) {
            self.commit_corners(corners, scene)?;
            Ok(())
        } else {
            Err("Failed to generate rectangle corners".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]fn center_typed_dimensions_are_whole_width_and_height(){
        let mut scene=Scene::new();let mut tool=RectangleTool::new();tool.start_point=Some(DVec3::ZERO);tool.hover_point=Some(DVec3::X+DVec3::Y);tool.from_center=true;tool.on_vcb_input(&mut scene,"4,2").unwrap();assert!((scene.mesh.face_area(0)-8.).abs()<1e-9);assert!((scene.mesh.vertices.iter().map(|v|v.position.x).fold(f64::NEG_INFINITY,f64::max)-2.).abs()<1e-9);
        let mut tool=RectangleTool::new();tool.start_point=Some(DVec3::ZERO);let before=scene.clone();assert!(tool.on_vcb_input(&mut scene,"-4,2").is_err());assert_eq!(scene,before);
    }
    #[test]fn first_face_pick_locks_a_vertical_work_plane(){
        let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::new(4.,0.,0.),DVec3::new(4.,0.,4.),DVec3::new(0.,0.,4.)].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);let mut tool=RectangleTool::new();let ctx=ToolContext::new(DVec3::new(1.,0.,1.),Ray::new(DVec3::new(1.,-3.,1.),DVec3::Y));tool.on_click(&mut scene,&ctx);assert!(tool.plane_normal.dot(DVec3::Y).abs()>0.999);assert_eq!(tool.drawing_plane().unwrap().0,ctx.world);let corners=tool.compute_corners(ctx.world,DVec3::new(2.,0.,3.)).unwrap();assert!(corners.iter().all(|p|p.y.abs()<1e-9));tool.on_cancel(&mut scene);assert!(tool.drawing_plane().is_none());assert_eq!(tool.plane_normal,DVec3::Z);
    }
    #[test]fn rectangle_band_splits_closed_body_and_remains_pushable(){
        use crate::pushpull::PushPullTool;
        let mut scene=Scene::new();let points=[DVec3::ZERO,DVec3::new(4.,0.,0.),DVec3::new(4.,3.,0.),DVec3::new(0.,3.,0.)];let ids=points.map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]);PushPullTool::extrude_face(&mut scene,0,2.).unwrap();
        let mut tool=RectangleTool::new();tool.commit_corners([DVec3::new(1.,0.,2.),DVec3::new(2.,0.,2.),DVec3::new(2.,3.,2.),DVec3::new(1.,3.,2.)],&mut scene).unwrap();
        assert!(scene.mesh.is_manifold_solid());let mut areas=scene.mesh.faces.iter().enumerate().filter(|(_,f)|f.vertices.iter().all(|&v|(scene.mesh.vertices[v].position.z-2.).abs()<1e-9)).map(|(i,_)|scene.mesh.face_area(i)).collect::<Vec<_>>();areas.sort_by(|a,b|a.total_cmp(b));assert_eq!(areas,vec![3.,3.,6.]);
        let band=scene.mesh.faces.iter().enumerate().find(|(i,f)|f.vertices.iter().all(|&v|(scene.mesh.vertices[v].position.z-2.).abs()<1e-9)&&scene.mesh.face_area(*i)<3.01&&f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.x>=1.-1e-9&&scene.mesh.vertices[v].position.x<=2.+1e-9)).unwrap().0;
        PushPullTool::extrude_face(&mut scene,band,-0.5).unwrap();assert!(scene.mesh.is_manifold_solid());assert!((scene.mesh.volume().unwrap()-22.5).abs()<1e-8);
    }
    #[test]
    fn test_rectangle_tool_two_clicks() {
        let mut tool = RectangleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx_hover = ToolContext::new(DVec3::new(3.0, 4.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);
        assert!(tool.preview_corners().is_some());

        let ctx1 = ToolContext::new(DVec3::new(3.0, 4.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices.len(), 4);
        assert_eq!(scene.mesh.edges.len(), 4);
        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 12.0).abs() < 1e-4);
        assert!(tool.start_point.is_none());
    }

    #[test]
    fn test_rectangle_tool_vcb_input() {
        let mut tool = RectangleTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx_hover = ToolContext::new(DVec3::new(2.0, 2.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);

        assert!(tool.on_vcb_input(&mut scene, "5.0, 2.0").is_ok());
        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_rectangle_tool_xz_plane() {
        let mut tool = RectangleTool::with_normal(DVec3::Y);
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Y));
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(2.0, 0.0, 3.0), Ray::new(DVec3::ZERO, DVec3::Y));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.faces.len(), 1);
        assert!((scene.mesh.face_area(0) - 6.0).abs() < 1e-4);
    }
}
