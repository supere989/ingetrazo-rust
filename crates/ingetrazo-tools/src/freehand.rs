// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Freehand sketching tool.
//!
//! Click and drag to draw a continuous curve. The sampled stroke is simplified
//! via Ramer–Douglas–Peucker. If the end point lands close to the start,
//! the loop closes into a face.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

fn rdp_recursive(pts: &[DVec3], tol: f64, keep: &mut [bool], start: usize, end: usize) {
    if end <= start + 1 {
        return;
    }
    let p_start = pts[start];
    let p_end = pts[end];
    let line_vec = p_end - p_start;
    let line_len_sq = line_vec.length_squared();

    let mut max_dist = 0.0;
    let mut max_idx = start;

    for i in (start + 1)..end {
        let p = pts[i];
        let dist = if line_len_sq < 1e-12 {
            (p - p_start).length()
        } else {
            let t = ((p - p_start).dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
            let proj = p_start + line_vec * t;
            (p - proj).length()
        };
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > tol {
        keep[max_idx] = true;
        rdp_recursive(pts, tol, keep, start, max_idx);
        rdp_recursive(pts, tol, keep, max_idx, end);
    }
}

pub fn simplify_rdp(pts: &[DVec3], tolerance: f64) -> Vec<DVec3> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    rdp_recursive(pts, tolerance, &mut keep, 0, pts.len() - 1);
    pts.iter()
        .enumerate()
        .filter_map(|(i, &p)| if keep[i] { Some(p) } else { None })
        .collect()
}

pub struct FreehandTool {
    points: Vec<DVec3>,
    drawing: bool,
    screen: Vec<glam::DVec2>,
    error: Option<String>,
    work_plane: Option<(DVec3,DVec3)>,
    hover_normal: DVec3,
    tolerance: f64,
}

impl Default for FreehandTool {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            drawing: false,
            screen: vec![],
            error: None,
            work_plane: None,
            hover_normal: DVec3::Z,
            tolerance: 0.05,
        }
    }
}

impl Tool for FreehandTool {
    fn error_message(&self)->Option<&str>{self.error.as_deref()}
    fn drawing_plane(&self)->Option<(DVec3,DVec3)>{self.work_plane}
    fn note_cursor_plane(&mut self,_point:DVec3,normal:DVec3){if !self.drawing{self.hover_normal=normal;}}
    fn on_release(&mut self,scene:&mut Scene,_ctx:&ToolContext,_distance:f64){if self.drawing{self.commit(scene);}}
    fn tool_type(&self) -> ToolType {
        ToolType::Freehand
    }

    fn name(&self) -> &'static str {
        "Freehand"
    }

    fn description(&self) -> &'static str {
        "Drag to sketch a curve; a stroke that ends where it began closes into a face."
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if !self.drawing {
            self.work_plane=Some((ctx.world,crate::base::pick_scene_face(scene,&ctx.ray).map(|(i,_,_)|scene.mesh.face_normal(i)).unwrap_or(self.hover_normal)));
            self.drawing = true;
            self.points.clear();self.screen.clear();self.error=None;
            self.points.push(ctx.world);if let Some(p)=ctx.screen{self.screen.push(p);}
        } else {
            // Commit drawing on second click / release
            self.commit(scene);
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        if self.drawing {
            if let Some(&last)=self.points.last(){
                let sample=match (ctx.screen,self.screen.last()){(Some(p),Some(q))=>p.distance(*q)>=3.,_=>ctx.world.distance(last)>0.02};
                if sample{self.points.push(ctx.world);if let Some(p)=ctx.screen{self.screen.push(p);}}
            }

        }
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.points.clear();self.screen.clear();
        self.drawing = false;self.work_plane=None;
    }

    fn rubber_band_lines(&self)->Vec<(DVec3,DVec3)>{self.points.windows(2).map(|p|(p[0],p[1])).collect()}
    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        if self.points.len() >= 2 {
            let last = self.points[self.points.len() - 1];
            let prev = self.points[self.points.len() - 2];
            Some((prev, last))
        } else {
            None
        }
    }
}

impl FreehandTool {
    pub fn commit(&mut self,scene:&mut Scene){
        let result=self.prepare_commit(scene);self.points.clear();self.screen.clear();self.drawing=false;self.work_plane=None;self.error=result.err();
    }
    fn prepare_commit(&self,scene:&mut Scene)->Result<(),String>{
        if self.points.len()<3{return Ok(());}
        let screen_space=self.screen.len()==self.points.len();
        let mut pts=if screen_space {
            let projected=self.screen.iter().map(|p|DVec3::new(p.x,p.y,0.)).collect::<Vec<_>>();let mut keep=vec![false;projected.len()];keep[0]=true;keep[projected.len()-1]=true;rdp_recursive(&projected,1.6,&mut keep,0,projected.len()-1);self.points.iter().zip(keep).filter_map(|(&p,k)|k.then_some(p)).collect::<Vec<_>>()
        }else{simplify_rdp(&self.points,self.tolerance)};
        let closed=if screen_space {self.screen[0].distance(*self.screen.last().unwrap())<=10.}else{self.points[0].distance(*self.points.last().unwrap())<0.2};
        if closed&&pts.len()>=3 {let p=pts[0];*pts.last_mut().unwrap()=p;}
        if pts.len()<2{return Ok(());}
        let segments=pts.windows(2).map(|p|(p[0],p[1])).collect::<Vec<_>>();let curve=scene.mesh.edges.iter().filter_map(|e|e.curve_id).max().unwrap_or(0).checked_add(1).ok_or("Curve identity overflow")?;
        let candidate=ingetrazo_core::drafting::add_segments(&scene.mesh,&segments,None,Some(curve))?;scene.mesh=candidate;scene.bump_version();Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]fn release_commits_one_curve_after_immutable_screen_preview(){
        use crate::base::ToolContext;use ingetrazo_core::math::Ray;
        let mut tool=FreehandTool::default();let mut scene=Scene::new();let points=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y,DVec3::ZERO];let screen=[(0.,0.),(100.,0.),(100.,100.),(0.,100.),(0.,0.)];
        for (i,&p) in points.iter().enumerate(){let mut ctx=ToolContext::new(p,Ray::new(DVec3::Z,-DVec3::Z));ctx.screen=Some(glam::DVec2::new(screen[i].0,screen[i].1));if i==0{tool.on_click(&mut scene,&ctx);}else{tool.on_move(&mut scene,&ctx);}assert!(scene.mesh.edges.is_empty());}
        let ctx=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));tool.on_release(&mut scene,&ctx,100.);assert_eq!(scene.mesh.faces.len(),1);assert_eq!(scene.mesh.edges.len(),4);assert!(scene.mesh.edges.iter().all(|e|e.curve_id==scene.mesh.edges[0].curve_id&&e.curve_id.is_some()));let before=scene.clone();tool.on_release(&mut scene,&ctx,100.);assert_eq!(scene,before);
    }
    #[test]
    fn test_rdp_simplification() {
        let pts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.01, 0.0),
            DVec3::new(2.0, -0.01, 0.0),
            DVec3::new(3.0, 0.0, 0.0),
        ];
        let simp = simplify_rdp(&pts, 0.05);
        assert_eq!(simp.len(), 2);
        assert_eq!(simp[0], pts[0]);
        assert_eq!(simp[1], pts[3]);
    }
}
