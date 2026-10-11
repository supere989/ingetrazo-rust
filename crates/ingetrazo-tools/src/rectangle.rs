// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;

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
    square_lock: bool,
    arrow_square_lock: bool,
    control_down: bool,
    plane_lock: Option<DVec3>,
    plane_reference: Option<ingetrazo_core::screen_pick::ReferenceKind>,
    axes: ingetrazo_core::drawing_axes::DrawingAxes,
}

impl Default for RectangleTool {
    fn default() -> Self {
        Self {
            start_point: None,
            hover_point: None,
            plane_normal: DVec3::Z, // Default XY ground plane
            from_center: false,
            error: None,
            square_lock: false,
            arrow_square_lock: false,
            control_down: false,
            plane_lock: None,
            plane_reference: None,
            axes: Default::default(),
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
        self.plane_normal=self.axes.z;self.square_lock=false;self.arrow_square_lock=false;self.error=None;self.plane_lock=None;self.plane_reference=None;
    }

    /// Calculate the 4 corners from start point and an opposite point.
    pub fn compute_corners(&self, start: DVec3, far: DVec3) -> Option<[DVec3; 4]> {
        let (u, v) = self.axes.plane_axes(self.plane_normal);
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

    fn inferred_corners(&self,start:DVec3,far:DVec3)->Option<[DVec3;4]>{
        let (u,v)=self.axes.plane_axes(self.plane_normal);let du=(far-start).dot(u);let dv=(far-start).dot(v);let near_square=(du.abs()-dv.abs()).abs()<=0.04*du.abs().max(dv.abs());
        let far=if du.abs()>1e-6&&dv.abs()>1e-6&&(self.square_lock||self.arrow_square_lock||near_square){let side=du.abs().max(dv.abs());start+u*side.copysign(du)+v*side.copysign(dv)}else{far};self.compute_corners(start,far)
    }

    /// Preview the 4 corners during hover.
    pub fn preview_corners(&self) -> Option<[DVec3; 4]> {
        match (self.start_point, self.hover_point) {
            (Some(s), Some(h)) => self.inferred_corners(s, h),
            _ => None,
        }
    }

    fn commit_corners(&mut self,corners:[DVec3;4],scene:&mut Scene)->Result<(),String>{
        let segments=(0..4).map(|i|(corners[i],corners[(i+1)%4])).collect::<Vec<_>>();
        scene.mesh=ingetrazo_core::drafting::add_segments_tagged_with_id(&scene.mesh,&segments,Some(&corners),None,scene.active_ifc.as_ref(),scene.bim_draw_id.as_ref())?;
        scene.bump_version();self.reset();self.error=None;Ok(())
    }

}

impl Tool for RectangleTool {
    // The original viewport leaves edits without replacing Rectangle. Its
    // start/hover points, plane and captured axes are world-space values.
    fn preserves_global_preview_on_context_exit(&self)->bool { true }

    fn error_message(&self)->Option<&str>{self.error.as_deref()}
    fn note_control_state(&mut self,held:bool){
        if held&&!self.control_down{self.from_center=!self.from_center;}
        self.control_down=held;
    }

    fn toggle_inference_lock(&mut self)->bool {
        if self.start_point.is_none(){return false;}
        if self.arrow_square_lock{self.arrow_square_lock=false;return true;}
        let (Some(start),Some(hover))=(self.start_point,self.hover_point) else{return false;};
        let (u,v)=self.axes.plane_axes(self.plane_normal);let d=hover-start;
        let a=d.dot(u).abs();let b=d.dot(v).abs();
        if a>1e-6 && b>1e-6 && (self.square_lock||(a-b).abs()<=0.04*a.max(b)){
            self.arrow_square_lock=true;return true;
        }
        false
    }

    fn note_cursor_plane(&mut self,_point:DVec3,normal:DVec3){if self.start_point.is_none()&&self.plane_lock.is_none(){self.plane_normal=normal;}}
    fn snap_anchor(&self)->Option<DVec3>{self.start_point}
    fn lock_drawing_reference(&mut self,reference:Option<ingetrazo_core::screen_pick::HoverReference>)->bool{
        if self.start_point.is_some(){return false;}
        if self.plane_reference.is_some(){self.plane_reference=None;self.plane_lock=None;self.plane_normal=self.axes.z;}
        else if let Some(reference)=reference{
            if reference.normal.is_finite()&&reference.normal.length()>1e-9{
                self.plane_reference=Some(reference.kind);self.plane_lock=Some(reference.normal.normalize());self.plane_normal=reference.normal.normalize();
            }
        }
        true
    }
    fn lock_drawing_plane(&mut self,normal:DVec3)->bool{
        if self.start_point.is_some(){return false;}let had_reference=self.plane_reference.take().is_some();self.plane_lock=if !had_reference&&self.plane_lock.is_some_and(|n|n.dot(normal).abs()>0.999){None}else{Some(normal.normalize_or_zero())};self.plane_normal=self.plane_lock.unwrap_or(self.axes.z);true
    }
    fn drawing_plane(&self)->Option<(DVec3,DVec3)>{self.start_point.or_else(||self.plane_lock.map(|_|self.hover_point.unwrap_or(self.axes.origin))).map(|p|(p,self.plane_normal))}
    fn rubber_band_lines(&self)->Vec<(DVec3,DVec3)>{let Some(c)=self.preview_corners()else{return vec![];};let mut edges=(0..4).map(|i|(c[i],c[(i+1)%4])).collect::<Vec<_>>();if (c[0].distance(c[1])-c[1].distance(c[2])).abs()<1e-6{edges.push((c[0],c[2]));}edges}
    fn value_label(&self)->Option<(String,DVec3)>{let c=self.preview_corners()?;Some((format!("{:.3} × {:.3}",c[0].distance(c[1]),c[1].distance(c[2])),c[2]))}

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

    fn on_activate(&mut self, scene: &mut Scene) {
        self.axes=scene.drawing_axes();
        self.reset();
        self.from_center=false;
    }

    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.axes=scene.drawing_axes();
        self.hover_point = Some(ctx.world);
        if self.plane_lock.is_none(){if let Some(start)=self.start_point{let d=ctx.world-start;if d.dot(self.plane_normal).abs()>=1e-3{let flat=[self.axes.x,self.axes.y,self.axes.z].into_iter().filter(|n|d.dot(*n).abs()<1e-3).collect::<Vec<_>>();if flat.len()==1{self.plane_normal=flat[0];}}}}
        self.note_control_state(ctx.ctrl_held);
        if !ctx.shift_held{self.square_lock=false;}else if let Some(start)=self.start_point{let (u,v)=self.axes.plane_axes(self.plane_normal);let d=ctx.world-start;let a=d.dot(u).abs();let b=d.dot(v).abs();if a>1e-6&&b>1e-6&&(a-b).abs()<=0.04*a.max(b){self.square_lock=true;}}

    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let axes=scene.drawing_axes();
        if self.start_point.is_none() && self.plane_lock.is_none() && self.axes!=axes {self.plane_normal=axes.z;}
        self.axes=axes;
        self.note_control_state(ctx.ctrl_held);
        if self.start_point.is_some(){self.on_move(scene,ctx);}
        if self.start_point.is_none() {
            if self.plane_lock.is_none(){if let Some((_,normal,_))=ingetrazo_core::screen_pick::hover_face(scene,&ctx.ray){self.plane_normal=normal;}}
            self.start_point = Some(ctx.world);
        } else {
            let start = self.start_point.unwrap();
            if let Some(corners) = self.inferred_corners(start, ctx.world) {
                if let Err(error)=self.commit_corners(corners,scene){self.error=Some(error);}
            }
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let Some(start) = self.start_point else {
            return Err("No active start point for rectangle dimensions".into());
        };

        let mut values=ingetrazo_core::units::vcb::parse_values(text,ingetrazo_core::units::model_scale(scene),true)?;
        if values.len()==1&&(self.square_lock||self.arrow_square_lock){values.push(values[0]);}
        let [w,h]=values.as_slice() else{return Err("Rectangle expects width and height".into());};
        let (w,h)=(*w,*h);

        if !w.is_finite()||!h.is_finite()||w <= 0. || h <= 0. {
            return Err("Dimensions must be finite and positive".into());
        }

        let (u, v) = self.axes.plane_axes(self.plane_normal);
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

    #[test]
    fn reference_plane_lock_prefers_caller_edge_and_survives_first_click(){
        use ingetrazo_core::screen_pick::{HoverReference,ReferenceKind};
        let mut scene=Scene::new();let before=scene.clone();let mut tool=RectangleTool::new();
        assert!(tool.lock_drawing_reference(None));assert!(tool.drawing_plane().is_none());
        let n=DVec3::new(1.,2.,3.).normalize();assert!(tool.lock_drawing_reference(Some(HoverReference{normal:n,kind:ReferenceKind::Edge})));
        let ctx=ToolContext::new(DVec3::new(2.,3.,4.),Ray::new(DVec3::Z,-DVec3::Z));tool.on_click(&mut scene,&ctx);
        assert_eq!(tool.drawing_plane().unwrap(),(ctx.world,n));assert!(!tool.lock_drawing_reference(None));
        assert_eq!(serde_json::to_value(&scene).unwrap(),serde_json::to_value(&before).unwrap());
        tool.on_cancel(&mut scene);assert!(tool.drawing_plane().is_none());
        assert!(tool.lock_drawing_reference(Some(HoverReference{normal:DVec3::Y,kind:ReferenceKind::Face})));
        assert!(tool.lock_drawing_reference(None));assert!(tool.drawing_plane().is_none());
        tool.lock_drawing_reference(Some(HoverReference{normal:n,kind:ReferenceKind::Edge}));
        assert!(tool.lock_drawing_plane(DVec3::X));assert_eq!(tool.plane_reference,None);assert_eq!(tool.plane_normal,DVec3::X);
    }

    #[test]
    fn control_toggles_center_persistently_without_pointer_motion() {
        let mut tool=RectangleTool::new();let mut scene=Scene::new();let before=scene.clone();
        tool.note_control_state(true);assert!(tool.from_center);
        tool.note_control_state(true);assert!(tool.from_center);
        tool.note_control_state(false);assert!(tool.from_center);
        assert_eq!(serde_json::to_value(&scene).unwrap(),serde_json::to_value(&before).unwrap());
        tool.start_point=Some(DVec3::ZERO);tool.hover_point=Some(DVec3::X+DVec3::Y);
        tool.on_vcb_input(&mut scene,"4,2").unwrap();assert!(tool.from_center);
        assert!((scene.mesh.face_area(0)-8.).abs()<1e-9);
        tool.on_cancel(&mut scene);assert!(tool.from_center);
        tool.note_control_state(true);assert!(!tool.from_center);
        tool.note_control_state(false);tool.note_control_state(true);assert!(tool.from_center);
        tool.on_activate(&mut scene);assert!(!tool.from_center);
    }

    #[test]
    fn down_latches_only_inferred_square_without_model_mutation() {
        let mut scene=Scene::new();let before=scene.clone();let mut tool=RectangleTool::new();
        assert!(!tool.toggle_inference_lock());tool.start_point=Some(DVec3::ZERO);
        let mut ctx=ToolContext::new(DVec3::new(4.,1.,0.),Ray::new(DVec3::Z,-DVec3::Z));
        tool.on_move(&mut scene,&ctx);assert!(!tool.toggle_inference_lock());
        ctx.world=DVec3::new(2.,1.95,0.);tool.on_move(&mut scene,&ctx);
        assert!(tool.toggle_inference_lock());ctx.world=DVec3::new(4.,1.,0.);
        tool.on_move(&mut scene,&ctx);assert_eq!(tool.preview_corners().unwrap()[2],DVec3::new(4.,4.,0.));
        assert_eq!(serde_json::to_value(&scene).unwrap(),serde_json::to_value(&before).unwrap());
        assert!(tool.toggle_inference_lock());assert_eq!(tool.preview_corners().unwrap()[2],ctx.world);
        ctx.world=DVec3::new(2.,1.95,0.);tool.on_move(&mut scene,&ctx);assert!(tool.toggle_inference_lock());
        tool.on_vcb_input(&mut scene,"3").unwrap();assert!((scene.mesh.face_area(0)-9.).abs()<1e-9);
        assert!(!tool.toggle_inference_lock());
    }

    #[test]fn nonprincipal_axes_project_world_x_and_square_inference_keeps_typed_sizes(){
        let normal=DVec3::new(0.3,0.4,0.866025403784).normalize();let tool=RectangleTool::with_normal(normal);let (u,v)=ingetrazo_core::drawing_axes::DrawingAxes::default().plane_axes(normal);let expected=(DVec3::X-normal*normal.x).normalize();assert!(u.distance(expected)<1e-12);let corners=tool.compute_corners(DVec3::ZERO,u*2.+v*3.).unwrap();assert!(corners[1].distance(u*2.)<1e-12);assert!(corners[3].distance(v*3.)<1e-12);
        let mut tool=RectangleTool::new();tool.start_point=Some(DVec3::ZERO);let mut ctx=ToolContext::new(DVec3::new(2.,1.95,0.),Ray::new(DVec3::Z,-DVec3::Z));ctx.shift_held=true;let mut scene=Scene::new();tool.on_move(&mut scene,&ctx);let square=tool.preview_corners().unwrap();assert_eq!(square[2],DVec3::new(2.,2.,0.));ctx.world=DVec3::new(4.,1.,0.);tool.on_move(&mut scene,&ctx);assert_eq!(tool.preview_corners().unwrap()[2],DVec3::new(4.,4.,0.));tool.on_vcb_input(&mut scene,"3").unwrap();assert!((scene.mesh.face_area(0)-9.).abs()<1e-9);
        let mut tool=RectangleTool::new();tool.start_point=Some(DVec3::ZERO);tool.hover_point=Some(DVec3::X+DVec3::Y);tool.on_vcb_input(&mut scene,"2,1.95").unwrap();assert!(scene.mesh.faces.iter().enumerate().any(|(i,_)|(scene.mesh.face_area(i)-3.9).abs()<1e-9));
    }
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
