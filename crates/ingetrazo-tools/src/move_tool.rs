// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::HashSet;
use glam::{DMat4, DVec3};
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{Tool, ToolContext, ToolType};

#[derive(Debug,Default,Clone)]
struct CopyAssets{
    mesh:ingetrazo_core::mesh::Mesh,
    plain:Vec<(DVec3,DVec3)>,
    isolated:Vec<DVec3>,
    groups:Vec<ingetrazo_core::groups::Group>,
}
/// Move and Copy tool supporting vector translations, Ctrl-copy, and linear array repeats.
#[derive(Debug, Default, Clone)]
pub struct MoveTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub is_copy_mode: bool,
    pub target_vertices: Vec<usize>,
    pub target_groups: Vec<usize>,
    pub target_planes:Vec<usize>,
    pub last_targets: Vec<usize>,
    pub last_delta: Option<DVec3>,
    pub last_moved_verts: Vec<usize>,
    exclusions: ingetrazo_core::snap_screen::SnapExclusions,
    copy_assets:CopyAssets,
    last_error:Option<String>,
}

impl MoveTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.start_point = None;
        self.hover_point = None;
        self.is_copy_mode = false;
        self.exclusions=Default::default();
        self.target_vertices.clear();
        self.target_groups.clear();self.target_planes.clear();
    }

    /// Resolve targets from selection or mouse click.
    pub fn gather_targets(&mut self, scene: &Scene, ctx: &ToolContext) {
        self.last_error=None;
        let mut vert_set = HashSet::new();
        let mut copy_edges=HashSet::new();let mut copy_faces=HashSet::new();let mut isolated=vec![];
        let mut group_set = HashSet::new();let mut plane_set=HashSet::new();

        if !scene.selection.is_empty() {
            for item in &scene.selection {
                match item {
                    SelectionItem::Vertex(vid) => {
                        if let Some(vertex)=scene.mesh.vertices.get(*vid){vert_set.insert(*vid);isolated.push(vertex.position);}
                    }
                    SelectionItem::Edge(eid) => {
                        if *eid < scene.mesh.edges.len() {
                            copy_edges.insert(*eid);
                            let e = &scene.mesh.edges[*eid];
                            vert_set.insert(e.v0);
                            vert_set.insert(e.v1);
                        }
                    }
                    SelectionItem::Face(fid) => {
                        if *fid < scene.mesh.faces.len() {
                            copy_faces.insert(*fid);
                            for &v in &scene.mesh.faces[*fid].vertices {
                                vert_set.insert(v);
                            }
                        }
                    }
                    SelectionItem::Group(gid) => {
                        group_set.insert(*gid);
                    }
                    SelectionItem::SectionPlane(id)=>{plane_set.insert(*id);},
                    SelectionItem::GeoPoint(_) | SelectionItem::GeoPath(_) => {}
                }
            }
        } else {
            // Original gather_targets picks a group, then an edge, then a face.
            if let Some(id)=crate::base::pick_section_plane(scene,&ctx.ray,0.2){plane_set.insert(id);}
            else if let Some((id,_))=crate::base::pick_scene_group(scene,&ctx.ray,0.2){group_set.insert(id);}
            else if let Some(eid)=crate::base::pick_scene_edge(scene,&ctx.ray,0.2){
                copy_edges.insert(eid);let e=&scene.mesh.edges[eid];vert_set.insert(e.v0);vert_set.insert(e.v1);
            }else if let Some((fid,_,_))=crate::base::pick_scene_face(scene,&ctx.ray){
                copy_faces.insert(fid);vert_set.extend(scene.mesh.faces[fid].vertices.iter().copied());
            }else if let Some(vid)=crate::base::pick_scene_vertex(scene,&ctx.ray,0.2){
                vert_set.insert(vid);isolated.push(scene.mesh.vertices[vid].position);
            }
        }

        let mut face_sides=HashSet::new();
        for &id in &copy_faces{let face=&scene.mesh.faces[id];for ring in std::iter::once(&face.vertices).chain(&face.holes){for i in 0..ring.len(){let (a,b)=(ring[i],ring[(i+1)%ring.len()]);face_sides.insert(if a<b{(a,b)}else{(b,a)});}}}
        let mut plain=vec![];
        copy_edges.retain(|&id|{let edge=&scene.mesh.edges[id];let pair=if edge.v0<edge.v1{(edge.v0,edge.v1)}else{(edge.v1,edge.v0)};
            if !edge.soft&&edge.curve_id.is_none()&&!face_sides.contains(&pair){plain.push((scene.mesh.vertices[edge.v0].position,scene.mesh.vertices[edge.v1].position));false}else{true}});
        match ingetrazo_core::containers::copy_mesh_subset(&scene.mesh,&copy_faces,&copy_edges){
            Ok(mut mesh)=>{
                // Source AddFace/AddEdge copy commands author fresh boundaries;
                // face attributes travel in full, explicit edges carry soft/curve.
                fn key(a:DVec3,b:DVec3)->([u64;3],[u64;3]){
                    let point=|v:DVec3|v.to_array().map(|x|if x==0.{0}else{x.to_bits()});let (a,b)=(point(a),point(b));if a<b{(a,b)}else{(b,a)}
                }
                let flags=copy_edges.iter().map(|&id|{let edge=&scene.mesh.edges[id];(key(scene.mesh.vertices[edge.v0].position,scene.mesh.vertices[edge.v1].position),(edge.soft,edge.curve_id))}).collect::<std::collections::HashMap<_,_>>();
                for edge in &mut mesh.edges{
                    let (soft,curve)=flags.get(&key(mesh.vertices[edge.v0].position,mesh.vertices[edge.v1].position)).copied().unwrap_or((false,None));
                    edge.attrs.clear();edge.layer=None;edge.hidden=false;edge.soft=soft;edge.curve_id=curve;
                }
                self.copy_assets=CopyAssets{mesh,plain,isolated,groups:group_set.iter().filter_map(|&id|scene.groups.get(id).cloned()).collect()};
            }
            Err(error)=>{self.last_error=Some(error);vert_set.clear();group_set.clear();plane_set.clear();self.copy_assets=Default::default();}
        }
        self.exclusions.loose_edges=scene.mesh.edges.iter().enumerate()
            .filter(|(_,edge)|vert_set.contains(&edge.v0)||vert_set.contains(&edge.v1))
            .map(|(id,_)|id).collect();
        self.exclusions.groups=group_set.iter().filter_map(|&id|scene.groups.get(id).map(|g|g.id)).collect();
        self.target_vertices = vert_set.into_iter().collect();
        self.target_groups = group_set.into_iter().collect();self.target_planes=plane_set.into_iter().collect();
    }

    /// Execute translation or copy.
    pub fn execute_move(&mut self, delta: DVec3, scene: &mut Scene) {
        if self.is_copy_mode&&self.target_vertices.is_empty()&&self.target_groups.is_empty(){return;}
        if delta.length_squared() < 1e-12 {
            return;
        }

        if !delta.is_finite(){self.last_error=Some("Move displacement must be finite".into());return;}
        if self.is_copy_mode {
            let result=(||->Result<_,String>{
                let (mut mesh,_,_)=ingetrazo_core::containers::paste_mesh(&scene.mesh,&self.copy_assets.mesh,delta)?;
                let segments=self.copy_assets.plain.iter().map(|&(a,b)|(a+delta,b+delta)).collect::<Vec<_>>();
                if !segments.is_empty(){mesh=ingetrazo_core::drafting::add_segments(&mesh,&segments,None,None)?;}
                let vertices=self.copy_assets.isolated.iter().map(|p|mesh.add_vertex(*p+delta)).collect::<Vec<_>>();
                let groups=self.copy_assets.groups.iter().map(|g|{let mut copy=g.duplicate_instance();copy.transform=DMat4::from_translation(delta)*copy.transform;copy}).collect::<Vec<_>>();
                Ok((mesh,vertices,groups))
            })();
            match result{Ok((mesh,vertices,groups))=>{scene.mesh=mesh;scene.groups.extend(groups);self.last_moved_verts=vertices;},Err(error)=>{self.last_error=Some(error);return;}}
        } else {
            // Translate original vertices
            for &vid in &self.target_vertices {
                scene.mesh.vertices[vid].position += delta;
            }
            for &gid in &self.target_groups {
                if gid < scene.groups.len() {
                    scene.groups[gid].transform =
                        DMat4::from_translation(delta) * scene.groups[gid].transform;
                }
            }
            self.last_moved_verts = self.target_vertices.clone();
        }

        // Original copy builders stamp geometry only; section planes stay put.
        if !self.is_copy_mode{for &id in &self.target_planes{if let Some(plane)=scene.section_planes.get_mut(id){plane.point+=delta;}}}
        self.last_targets = self.target_vertices.clone();
        self.last_delta = Some(delta);
        scene.bump_version();
    }
}

impl Tool for MoveTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Move
    }

    fn error_message(&self)->Option<&str>{self.last_error.as_deref()}

    fn name(&self) -> &'static str {
        "Move"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("M")
    }

    fn description(&self) -> &'static str {
        "Move the selection, or what you click, to a new place; Ctrl leaves a copy behind."
    }

    fn vcb_label(&self) -> &'static str {
        "Distance"
    }

    fn snap_anchor(&self) -> Option<DVec3> {
        self.start_point
    }

    fn prefers_vertical_drag(&self) -> bool { true }
    fn magnetic_axis_deg(&self)->Option<f64>{Some(15.)}
    fn screen_axis_px(&self)->Option<f64>{Some(9.)}

    fn snap_exclusions(&self)->Option<&ingetrazo_core::snap_screen::SnapExclusions>{
        (self.start_point.is_some()&&!self.is_copy_mode).then_some(&self.exclusions)
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
            self.is_copy_mode = true;
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if ctx.ctrl_held {
            self.is_copy_mode = true;
        }

        if self.start_point.is_none() {
            self.gather_targets(scene, ctx);
            if !self.target_vertices.is_empty() || !self.target_groups.is_empty() || !self.target_planes.is_empty() {
                self.start_point = Some(ctx.world);
            }
        } else {
            let start = self.start_point.unwrap();
            let delta = ctx.world - start;
            self.execute_move(delta, scene);
            self.reset();
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();

        // 1. Array repeat: e.g. "5x", "*5", "/5", "5/"
        if (trimmed.ends_with('x') || trimmed.ends_with('X') || trimmed.starts_with('*'))
            && self.last_delta.is_some()
        {
            let num_str = trimmed.trim_matches(|c| c == 'x' || c == 'X' || c == '*');
            let count: usize = num_str.trim().parse().map_err(|e| format!("Invalid count: {e}"))?;
            let delta = self.last_delta.unwrap();

            // Create (count - 1) additional copies along delta
            for k in 2..=count {
                for &vid in &self.last_targets {
                    let old_pos = scene.mesh.vertices[vid].position;
                    scene.mesh.add_vertex(old_pos + delta * (k as f64));
                }
            }
            scene.bump_version();
            return Ok(());
        }

        if (trimmed.starts_with('/') || trimmed.ends_with('/')) && self.last_delta.is_some() {
            let num_str = trimmed.trim_matches('/');
            let count: usize = num_str.trim().parse().map_err(|e| format!("Invalid count: {e}"))?;
            if count == 0 {
                return Err("Count must be greater than zero".into());
            }
            let delta = self.last_delta.unwrap();

            // Internal division copies
            for k in 1..count {
                let frac = k as f64 / count as f64;
                for &vid in &self.last_targets {
                    let old_pos = scene.mesh.vertices[vid].position;
                    scene.mesh.add_vertex(old_pos + delta * frac);
                }
            }
            scene.bump_version();
            return Ok(());
        }

        // 2. 3D delta or single distance during active move
        let Some(start) = self.start_point else {
            return Err("No active move drag in progress".into());
        };

        let parts: Vec<&str> = if trimmed.contains(';') {
            trimmed.split(';').collect()
        } else if trimmed.contains(',') {
            trimmed.split(',').collect()
        } else {
            trimmed.split_whitespace().collect()
        };

        let delta = if parts.len() == 3 {
            let dx: f64 = parts[0].trim().parse().map_err(|e| format!("Invalid dx: {e}"))?;
            let dy: f64 = parts[1].trim().parse().map_err(|e| format!("Invalid dy: {e}"))?;
            let dz: f64 = parts[2].trim().parse().map_err(|e| format!("Invalid dz: {e}"))?;
            DVec3::new(dx, dy, dz)
        } else {
            let dist: f64 = trimmed.parse().map_err(|e| format!("Invalid distance: {e}"))?;
            let dir = self
                .hover_point
                .map(|h| (h - start).normalize_or_zero())
                .unwrap_or(DVec3::X);
            dir * dist
        };

        self.execute_move(delta, scene);
        self.reset();
        if let Some(error)=self.last_error.take(){Err(error)}else{Ok(())}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn inference_excludes_incident_edges_and_nested_groups_but_copy_keeps_them(){
        let mut scene=Scene::new();
        let a=scene.mesh.add_vertex(DVec3::new(2.,3.,1.));let b=scene.mesh.add_vertex(DVec3::new(4.,3.,1.));
        let c=scene.mesh.add_vertex(DVec3::new(4.,4.,1.));let d=scene.mesh.add_vertex(DVec3::new(8.,4.,1.));
        let selected=scene.mesh.add_edge(a,b).unwrap();let incident=scene.mesh.add_edge(b,c).unwrap();let untouched=scene.mesh.add_edge(c,d).unwrap();
        scene.groups.push(ingetrazo_core::groups::Group::new("moving parent"));
        scene.groups[0].children.push(ingetrazo_core::groups::Group::new("nested child"));
        scene.select(SelectionItem::Edge(selected));scene.select(SelectionItem::Group(0));
        let ctx=ToolContext::new(DVec3::new(2.,3.,1.),Ray::new(DVec3::new(2.,3.,10.),-DVec3::Z));
        let mut tool=MoveTool::new();tool.on_click(&mut scene,&ctx);
        let excluded=tool.snap_exclusions().unwrap();
        assert_eq!(excluded.loose_edges,HashSet::from([selected,incident]));assert!(!excluded.loose_edges.contains(&untouched));
        assert_eq!(excluded.groups,HashSet::from([scene.groups[0].id]));
        tool.is_copy_mode=true;assert!(tool.snap_exclusions().is_none());
        tool.on_cancel(&mut scene);assert!(tool.snap_exclusions().is_none());
    }

    fn copy_at(scene:&mut Scene,delta:DVec3)->MoveTool{
        let mut tool=MoveTool::new();let mut first=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::new(0.,0.,10.),-DVec3::Z));first.ctrl_held=true;tool.on_click(scene,&first);
        tool.on_click(scene,&ToolContext::new(delta,first.ray));tool
    }
    #[test]fn copy_holed_face_preserves_face_paint_uv_and_authors_fresh_boundaries(){
        let mut scene=Scene::new();let outer=[DVec3::ZERO,DVec3::X*4.,DVec3::new(4.,4.,0.),DVec3::Y*4.].map(|p|scene.mesh.add_vertex(p));
        let hole=[DVec3::new(1.,1.,0.),DVec3::new(1.,3.,0.),DVec3::new(3.,3.,0.),DVec3::new(3.,1.,0.)].map(|p|scene.mesh.add_vertex(p));
        let face=scene.mesh.add_face(outer.to_vec(),vec![hole.to_vec()]).unwrap();
        scene.mesh.faces[face].attrs.insert("vendor".into(),serde_json::json!(71));scene.mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"uvw":[1.,0.,0.,2.,0.,1.,0.,3.],"path":"tile.png"}));
        for edge in &mut scene.mesh.edges{edge.soft=true;edge.attrs.insert("edge-vendor".into(),serde_json::json!(4));}
        scene.select(SelectionItem::Face(face));let before=scene.mesh.clone();let tool=copy_at(&mut scene,DVec3::X*10.);assert!(tool.error_message().is_none());
        assert_eq!(scene.mesh.faces.len(),2);assert_eq!(scene.mesh.edges.len(),16);assert_eq!(scene.mesh.vertices.len(),16);
        let copied=&scene.mesh.faces[1];assert_eq!(copied.holes.len(),1);assert_eq!(copied.attrs["vendor"],serde_json::json!(71));assert_eq!(copied.attrs["texture"]["uvw"][3],serde_json::json!(-8.));
        assert!(scene.mesh.edges[..8].iter().all(|e|e.soft));assert!(scene.mesh.edges[8..].iter().all(|e|!e.soft&&!e.attrs.contains_key("edge-vendor")));
        assert_eq!(scene.mesh.vertices[0].uid,before.vertices[0].uid);assert_ne!(scene.mesh.vertices[8].uid,before.vertices[0].uid);
    }
    #[test]fn copied_plain_edge_splits_existing_crossing_without_repainting_source(){
        let mut scene=Scene::new();let a=scene.mesh.add_vertex(-DVec3::X);let b=scene.mesh.add_vertex(DVec3::X);let edge=scene.mesh.add_edge(a,b).unwrap();
        let c=scene.mesh.add_vertex(DVec3::Y);let d=scene.mesh.add_vertex(DVec3::Y*3.);scene.mesh.add_edge(c,d);
        scene.mesh.edges[edge].attrs.insert("vendor".into(),serde_json::json!(71));scene.select(SelectionItem::Edge(edge));let tool=copy_at(&mut scene,DVec3::Y*2.);assert!(tool.error_message().is_none());
        assert_eq!(scene.mesh.edges.len(),5);let crossing=scene.mesh.vertices.iter().position(|v|v.position.distance(DVec3::Y*2.)<1e-9).unwrap();
        assert_eq!(scene.mesh.edges.iter().filter(|e|e.contains_vertex(crossing)).count(),4);assert_eq!(scene.mesh.edges.iter().filter(|e|e.attrs.get("vendor")==Some(&serde_json::json!(71))).count(),1);
    }
    #[test]fn group_copy_retains_private_semantics_nested_metadata_and_fresh_placements(){
        let mut scene=Scene::new();let mut group=ingetrazo_core::groups::Group::new("private");group.transform=DMat4::from_rotation_z(0.3);group.native_document=serde_json::json!({"material":{"color":[1,0,0]},"vendor":71});group.children.push(ingetrazo_core::groups::Group::new("child"));scene.groups.push(group);scene.select(SelectionItem::Group(0));
        let original=scene.groups[0].clone();let tool=copy_at(&mut scene,DVec3::X*5.);assert!(tool.error_message().is_none());assert_eq!(scene.groups.len(),2);let copy=&scene.groups[1];
        assert_eq!(scene.groups[0],original);assert_ne!(copy.id,original.id);assert_ne!(copy.children[0].id,original.children[0].id);assert!(copy.component_definition.is_none());assert_eq!(copy.native_document,original.native_document);
        assert!((copy.transform.transform_point3(DVec3::ONE)-original.transform.transform_point3(DVec3::ONE)-DVec3::X*5.).length()<1e-10);
    }
    #[test]fn nonfinite_copy_is_atomic_and_actionable(){
        let mut scene=Scene::new();let v=scene.mesh.add_vertex(DVec3::ZERO);scene.select(SelectionItem::Vertex(v));let before=scene.clone();let tool=copy_at(&mut scene,DVec3::splat(f64::NAN));assert_eq!(scene,before);assert_eq!(tool.error_message(),Some("Move displacement must be finite"));
    }

    #[test]
    fn test_move_tool_translation() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let ctx0 = ToolContext::new(DVec3::new(1.0, 1.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx0);
        assert_eq!(tool.target_vertices, vec![v0]);

        let ctx1 = ToolContext::new(DVec3::new(4.0, 5.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        assert_eq!(scene.mesh.vertices[v0].position, DVec3::new(4.0, 5.0, 0.0));
    }

    #[test]
    fn test_move_tool_copy_mode() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let mut ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        ctx0.ctrl_held = true;
        tool.on_click(&mut scene, &ctx0);
        assert!(tool.is_copy_mode);

        let ctx1 = ToolContext::new(DVec3::new(3.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Original stays at (0,0,0); new copy at (3,0,0)
        assert_eq!(scene.mesh.vertices.len(), 2);
        assert_eq!(scene.mesh.vertices[0].position, DVec3::ZERO);
        assert_eq!(scene.mesh.vertices[1].position, DVec3::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn test_move_tool_array_repeat() {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::ZERO);
        scene.select(SelectionItem::Vertex(v0));

        let mut tool = MoveTool::new();
        let mut ctx0 = ToolContext::new(DVec3::ZERO, Ray::new(DVec3::ZERO, DVec3::Z));
        ctx0.ctrl_held = true;
        tool.on_click(&mut scene, &ctx0);

        let ctx1 = ToolContext::new(DVec3::new(10.0, 0.0, 0.0), Ray::new(DVec3::ZERO, DVec3::Z));
        tool.on_click(&mut scene, &ctx1);

        // Type "3x" array repeat
        assert!(tool.on_vcb_input(&mut scene, "3x").is_ok());
        // Original + 1st copy + 2nd & 3rd copy from array repeat
        assert_eq!(scene.mesh.vertices.len(), 4);
    }
}
