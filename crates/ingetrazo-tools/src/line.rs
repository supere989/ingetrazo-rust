// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;


use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Line tool for drawing segments and closing planar faces.
#[derive(Debug, Default, Clone)]
pub struct LineTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub chain_first_point: Option<DVec3>,
    pub chain_vertices: Vec<DVec3>,
    error: Option<String>,
    work_plane: Option<(DVec3,DVec3)>,
}

impl LineTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.hover_point = None;
        self.chain_first_point = None;
        self.chain_vertices.clear();self.work_plane=None;self.error=None;
    }

    fn commit_segment(&mut self, next_point: DVec3, scene: &mut Scene) -> Result<(),String> {
        let Some(start) = self.start_point else { return Ok(()) };
        if (next_point - start).length() < 1e-9 {
            return Ok(());
        }

        let candidate=ingetrazo_core::drafting::add_segments(&scene.mesh,&[(start,next_point)],None,None)?;
        scene.mesh=candidate;
        // Check if closing polygon at start of current chain
        let closed_chain = self
            .chain_first_point
            .map_or(false, |first| (first - next_point).length() < 1e-4);

        if closed_chain {
            self.reset();
        } else {
            self.chain_vertices.push(next_point);
            self.start_point = Some(next_point);
        }

        scene.bump_version();
        self.error=None;Ok(())
    }
}

impl Tool for LineTool {
    fn error_message(&self)->Option<&str>{self.error.as_deref()}
    fn drawing_plane(&self)->Option<(DVec3,DVec3)>{self.work_plane}
    fn prepare_undo(&mut self,available:bool)->bool{
        if self.start_point.is_none(){return false;}if self.chain_vertices.len()>1&&available{self.chain_vertices.pop();self.start_point=self.chain_vertices.last().copied();if self.chain_vertices.len()==1{self.chain_first_point=self.start_point;}false}else{self.reset();true}
    }

    fn tool_type(&self) -> ToolType {
        ToolType::Line
    }

    fn name(&self) -> &'static str {
        "Line"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("L")
    }

    fn description(&self) -> &'static str {
        "Draw edges point by point; closing a loop makes a face."
    }

    fn vcb_label(&self) -> &'static str {
        "Length"
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
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.start_point.is_none() {
            self.work_plane=crate::base::pick_scene_face(scene,&ctx.ray).map(|(i,_,_)|(ctx.world,scene.mesh.face_normal(i)));
            self.start_point = Some(ctx.world);
            self.chain_first_point = Some(ctx.world);
            self.chain_vertices = vec![ctx.world];
        } else {
            if let Err(error)=self.commit_segment(ctx.world,scene){self.error=Some(error);}
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let Some(start) = self.start_point else {
            return Err("No active start point".into());
        };

        let trimmed = text.trim();
        // Check for 3D delta format: "dx;dy;dz" or "dx,dy,dz" or "dx dy dz"
        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        if parts.len() == 3 {
            let dx: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid dx: {e}"))?;
            let dy: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid dy: {e}"))?;
            let dz: f64 = parts[2].trim().parse().map_err(|e| format!("Invalid dz: {e}"))?;
            let target = start + DVec3::new(dx, dy, dz);
            self.commit_segment(target, scene)?;
            self.hover_point = Some(target);
            return Ok(());
        }

        // Otherwise length along current hover direction
        let length: f64 = trimmed.parse().map_err(|e| format!("Invalid length: {e}"))?;
        let Some(hover) = self.hover_point else {
            return Err("No direction available for length input".into());
        };

        let delta = hover - start;
        if delta.length_squared() < 1e-9 {
            return Err("Direction vector is degenerate".into());
        }
        let dir = delta.normalize();
        let target = start + dir * length;
        self.commit_segment(target, scene)?;
        self.hover_point = Some(target);
        Ok(())
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        match (self.start_point, self.hover_point) {
            (Some(s), Some(h)) => Some((s, h)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn test_line_tool_draw_and_rubber_band() {
        let mut tool = LineTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(0.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.start_point, Some(DVec3::ZERO));

        let ctx_hover = ToolContext::new(DVec3::new(5.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);
        assert_eq!(tool.rubber_band(), Some((DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0))));

        let ctx1 = ToolContext::new(DVec3::new(5.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);
        assert_eq!(scene.mesh.edges.len(), 1);
        assert_eq!(tool.start_point, Some(DVec3::new(5.0, 0.0, 0.0)));
    }

    #[test]
    fn test_line_tool_closing_cycle_creates_face() {
        let mut tool = LineTool::new();
        let mut scene = Scene::new();

        let pts = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
            DVec3::new(0.0, 0.0, 0.0), // closes cycle
        ];

        for &p in &pts {
            let ctx = ToolContext::new(p, Ray::new(DVec3::ZERO, DVec3::Z));
            tool.on_click(&mut scene, &ctx);
        }

        assert_eq!(scene.mesh.edges.len(), 4);
        assert_eq!(scene.mesh.faces.len(), 1);
        assert!(tool.start_point.is_none()); // chain reset on close
    }

    #[test]
    fn test_line_tool_vcb_input() {
        let mut tool = LineTool::new();
        let mut scene = Scene::new();

        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);

        let ctx_hover = ToolContext::new(DVec3::new(2.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_move(&mut scene, &ctx_hover);

        // VCB length 10.0 along X
        assert!(tool.on_vcb_input(&mut scene, "10.0").is_ok());
        assert_eq!(scene.mesh.edges.len(), 1);
        assert_eq!(tool.start_point, Some(DVec3::new(11.0, 1.0, 0.0)));

        // VCB 3D delta
        assert!(tool.on_vcb_input(&mut scene, "0.0; 5.0; 2.0").is_ok());
        assert_eq!(scene.mesh.edges.len(), 2);
        assert_eq!(tool.start_point, Some(DVec3::new(11.0, 6.0, 2.0)));
    }
}
