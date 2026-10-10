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
#[derive(Debug,Clone)]
struct ArrayBase { mesh:ingetrazo_core::mesh::Mesh, groups:usize, version:u64 }
/// Move and Copy tool supporting vector translations, Ctrl-copy, and linear array repeats.
#[derive(Debug, Default, Clone)]
pub struct MoveTool {
    pub start_point: Option<DVec3>,
    pub hover_point: Option<DVec3>,
    pub is_copy_mode: bool,
    pub target_vertices: Vec<usize>,
    pub target_groups: Vec<usize>,
    pub target_planes:Vec<usize>,
    pub target_images:Vec<usize>,
    original_images:Vec<(usize,ingetrazo_core::image_plane::ImagePlane)>,
    preview_images:Vec<(usize,DVec3)>,
    image_command:Option<ingetrazo_core::image_plane::ImageFrameCommand>,
    pub last_targets: Vec<usize>,
    pub last_delta: Option<DVec3>,
    pub last_moved_verts: Vec<usize>,
    exclusions: ingetrazo_core::snap_screen::SnapExclusions,
    copy_assets:CopyAssets,
    last_error:Option<String>,
    control_down:bool,
    array_base:Option<ArrayBase>,
    replace_history:bool,
    array_history_revision:Option<u64>,
    original_positions:Vec<(usize,DVec3)>,
    original_groups:Vec<(usize,DMat4)>,
    original_planes:Vec<(usize,DVec3)>,
    preview_delta:DVec3,
    detached_vertices:bool,
    original_version:u64,
    preview_version:u64,
}

/// Equality representation of original mesh._key: Qt float32 input, Python
/// ties-even integer rounding, and no narrowing integer cast. None is the
/// original nonfinite key refusal, not a coordinate eligible for welding.
#[doc(hidden)]
pub fn source_move_position_key(p:DVec3)->Option<[f64;3]>{
    let p=p.as_vec3().as_dvec3();
    p.is_finite().then(||p.to_array().map(|x|{let n=(x*10_000.).round_ties_even();if n==0.{0.}else{n}}))
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
        self.target_groups.clear();self.target_planes.clear();self.target_images.clear();self.original_images.clear();self.preview_images.clear();
        self.detached_vertices=false;self.original_positions.clear();self.original_groups.clear();self.original_planes.clear();self.preview_delta=DVec3::ZERO;
    }

    /// Resolve targets from selection or mouse click.
    pub fn gather_targets(&mut self, scene: &Scene, ctx: &ToolContext) {
        self.last_error=None;self.image_command=None;
        self.target_images=crate::base::gather_image_targets(scene,ctx);
        self.original_images=self.target_images.iter().map(|&id|(id,scene.image_planes[id].clone())).collect();
        self.preview_images=self.original_images.iter().map(|(id,image)|(*id,image.origin)).collect();
        self.array_base=None;self.replace_history=false;
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
                    SelectionItem::ImagePlane(_) | SelectionItem::GeoPoint(_) | SelectionItem::GeoPath(_) => {}
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
        self.original_version=scene.version;self.preview_version=scene.version;
        self.original_positions=self.target_vertices.iter().map(|&id|(id,scene.mesh.vertices[id].position)).collect();
        self.original_groups=self.target_groups.iter().filter_map(|&id|scene.groups.get(id).map(|g|(id,g.transform))).collect();
        self.original_planes=self.target_planes.iter().filter_map(|&id|scene.section_planes.get(id).map(|p|(id,p.point))).collect();
    }

    fn restore_positions(&self,scene:&mut Scene){
        for (id,image) in &self.original_images{if let Some(live)=scene.image_planes.get_mut(*id){live.origin=image.origin;}}
        if !self.detached_vertices{for &(id,point) in &self.original_positions{if let Some(v)=scene.mesh.vertices.get_mut(id){v.position=point;}}}
        for &(id,transform) in &self.original_groups{if let Some(g)=scene.groups.get_mut(id){g.transform=transform;}}
        for &(id,point) in &self.original_planes{if let Some(p)=scene.section_planes.get_mut(id){p.point=point;}}
    }
    fn apply_preview(&mut self,scene:&mut Scene,delta:DVec3){
        if !delta.is_finite()||delta==self.preview_delta{return;}
        if !self.detached_vertices{for &(id,point) in &self.original_positions{if let Some(v)=scene.mesh.vertices.get_mut(id){v.position=point+delta;}}}
        for &(id,transform) in &self.original_groups{if let Some(g)=scene.groups.get_mut(id){g.transform=DMat4::from_translation(delta)*transform;}}
        for &(id,point) in &self.original_planes{if let Some(p)=scene.section_planes.get_mut(id){p.point=point+delta;}}
        // Qt applies each preview increment in float32, including accumulated rounding.
        for (id,_) in &self.original_images{if let Some(live)=scene.image_planes.get_mut(*id){live.origin=(live.origin.as_vec3()+(delta.as_vec3()-self.preview_delta.as_vec3())).as_dvec3();}}
        for(id,origin)in &mut self.preview_images{if let Some(live)=scene.image_planes.get(*id){*origin=live.origin;}}
        self.preview_delta=delta;scene.bump_version();self.preview_version=scene.version;
    }
    fn revert_preview(&mut self,scene:&mut Scene){
        if self.preview_delta!=DVec3::ZERO{
            let image_origins=self.original_images.iter().filter_map(|(id,_)|scene.image_planes.get(*id).map(|p|(*id,(p.origin.as_vec3()-self.preview_delta.as_vec3()).as_dvec3()))).collect::<Vec<_>>();
            self.restore_positions(scene);for(id,origin)in image_origins{scene.image_planes[id].origin=origin;}
            for(id,origin)in &mut self.preview_images{if let Some(live)=scene.image_planes.get(*id){*origin=live.origin;}}self.preview_delta=DVec3::ZERO;scene.bump_version();self.preview_version=scene.version;}
    }
    /// Execute translation or copy.
    pub fn execute_move(&mut self, delta: DVec3, scene: &mut Scene) {
        self.last_error=None;
        if !delta.is_finite(){self.last_error=Some("Move displacement must be finite".into());return;}
        self.revert_preview(scene);
        if self.detached_vertices {
            // Source MoveVerticesCommand resolves grabbed positions against the
            // current loose mesh. The published group is not a replacement for
            // the detached working vertex objects held by the active tool.
            let Some(positions)=self.original_positions.iter().map(|(_,p)|source_move_position_key(*p).map(|k|k.map(f64::to_bits))).collect::<Option<HashSet<_>>>() else {self.last_error=Some("Move source coordinates are not finite".into());return;};
            if scene.mesh.vertices.iter().any(|v|!v.position.as_vec3().is_finite()){self.last_error=Some("Move mesh coordinates are not finite".into());return;}
            self.target_vertices=scene.mesh.vertices.iter().enumerate().filter_map(|(id,v)|positions.contains(&source_move_position_key(v.position).unwrap().map(f64::to_bits)).then_some(id)).collect();
        }
        if self.is_copy_mode&&self.target_vertices.is_empty()&&self.target_groups.is_empty(){return;}
        if delta.length_squared() < 1e-12 && (self.target_images.is_empty() || delta.as_vec3().length() <= 1e-9) {
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
            match result{Ok((mesh,vertices,groups))=>{let base=std::mem::replace(&mut scene.mesh,mesh);self.array_base=Some(ArrayBase{mesh:base,groups:scene.groups.len(),version:scene.version.wrapping_add(1)});scene.groups.extend(groups);self.last_moved_verts=vertices;},Err(error)=>{self.last_error=Some(error);return;}}
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

        // Original copy builders stamp geometry only; reference images stay put.
        if !self.is_copy_mode&&!self.original_images.is_empty(){
            let command=ingetrazo_core::image_plane::ImageFrameCommand{images:self.original_images.iter().map(|(_,image)|image.clone()).collect(),operation:ingetrazo_core::image_plane::ImageFrameOperation::Move(delta)};
            // Publish grab baseline rather than live preview into held identity.
            scene.image_objects.publish(&scene.image_planes);
            command.apply(&mut scene.image_planes,&scene.image_objects,true);self.image_command=Some(command);
        }
        // Original copy builders stamp geometry only; section planes stay put.
        if !self.is_copy_mode{for &id in &self.target_planes{if let Some(plane)=scene.section_planes.get_mut(id){plane.point+=delta;}}}
        self.last_targets = self.target_vertices.clone();
        self.last_delta = Some(delta);
        scene.mesh.rebuild_spatial_index();
        scene.bump_version();
    }
}

impl Tool for MoveTool {
    fn visit_image_metadata_roots(&self,visit:&mut dyn FnMut(&ingetrazo_core::image_plane::ImagePlane)){for(_,image)in &self.original_images{visit(image);}}
    fn preserves_global_preview_on_context_exit(&self)->bool{self.start_point.is_some()&&!self.is_copy_mode&&!self.target_images.is_empty()&&self.target_groups.is_empty()&&self.target_planes.is_empty()}
    fn detach_context_preview(&mut self){self.detached_vertices=true;}
    fn take_image_frame_command(&mut self)->Option<ingetrazo_core::image_plane::ImageFrameCommand>{self.image_command.take()}
    fn placement_preview(&self)->Option<crate::base::PlacementPreview<'_>> {
        if self.start_point.is_none() || self.is_copy_mode || self.original_groups.is_empty()
            || !self.original_positions.is_empty() || !self.original_planes.is_empty() || !self.original_images.is_empty() {return None;}
        Some(crate::base::PlacementPreview {grab_version:self.original_version,
            groups:&self.original_groups,world_delta:self.preview_delta})
    }

    fn tool_type(&self) -> ToolType {
        ToolType::Move
    }

    fn error_message(&self)->Option<&str>{self.last_error.as_deref()}
    fn take_history_replacement(&mut self)->bool{std::mem::take(&mut self.replace_history)}
    fn note_history_revision(&mut self,revision:u64,committed:bool){
        if committed{self.array_history_revision=self.array_base.as_ref().map(|_|revision);}
        else if self.array_history_revision.is_some_and(|old|old!=revision){self.array_base=None;self.array_history_revision=None;}
    }
    fn prepare_undo(&mut self,_available:bool)->bool{self.array_base=None;false}

    fn restore_transient_model(&self,scene:&mut Scene){if self.start_point.is_some(){
        let origins=self.original_images.iter().filter_map(|(id,_)|scene.image_planes.get(*id).map(|p|(*id,(p.origin.as_vec3()-self.preview_delta.as_vec3()).as_dvec3()))).collect::<Vec<_>>();
        self.restore_positions(scene);for(id,origin)in origins{scene.image_planes[id].origin=origin;}scene.version=self.original_version;
    }}
    fn end_transient_before_history(&mut self,scene:&mut Scene){if self.start_point.is_some(){self.on_cancel(scene);}}
    fn restore_current_preview_model(&self,scene:&mut Scene){
        if self.start_point.is_none(){return;}
        scene.version=self.preview_version;
        // The exact accumulated float32 pose cannot be reconstructed from its inverse.
        for(id,origin)in &self.preview_images{if let Some(live)=scene.image_planes.get_mut(*id){live.origin=*origin;}}
        if !self.detached_vertices{for &(id,point) in &self.original_positions{if let Some(v)=scene.mesh.vertices.get_mut(id){v.position=point+self.preview_delta;}}}
        for &(id,transform) in &self.original_groups{if let Some(g)=scene.groups.get_mut(id){g.transform=DMat4::from_translation(self.preview_delta)*transform;}}
        for &(id,point) in &self.original_planes{if let Some(p)=scene.section_planes.get_mut(id){p.point=point+self.preview_delta;}}
    }
    fn on_control_state(&mut self,scene:&mut Scene,held:bool){
        self.note_control_state(held);
        if let (Some(start),Some(hover))=(self.start_point,self.hover_point){
            if self.is_copy_mode{self.revert_preview(scene);}else{self.apply_preview(scene,hover-start);}
        }
    }
    fn note_control_state(&mut self,held:bool){
        if held&&!self.control_down{self.is_copy_mode=!self.is_copy_mode;}
        self.control_down=held;
    }

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

    fn on_deactivate(&mut self, scene: &mut Scene) {
        self.revert_preview(scene);
        self.array_base=None;
        self.reset();
    }

    fn on_cancel(&mut self, scene: &mut Scene) {
        self.revert_preview(scene);
        self.array_base=None;
        self.reset();
    }

    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
        self.note_control_state(ctx.ctrl_held);
        if let Some(start)=self.start_point{
            if self.is_copy_mode{self.revert_preview(scene);}else{self.apply_preview(scene,ctx.world-start);}
        }
    }
    fn rubber_band_lines(&self)->Vec<(DVec3,DVec3)>{
        let (Some(start),Some(hover))=(self.start_point,self.hover_point)else{return vec![]};
        let mut lines=vec![(start,hover)];if !self.is_copy_mode{return lines;}
        let delta=hover-start;lines.extend(self.copy_assets.plain.iter().map(|&(a,b)|(a+delta,b+delta)));
        for e in &self.copy_assets.mesh.edges{lines.push((self.copy_assets.mesh.vertices[e.v0].position+delta,self.copy_assets.mesh.vertices[e.v1].position+delta));}
        fn append(group:&ingetrazo_core::groups::Group,parent:DMat4,delta:DVec3,lines:&mut Vec<(DVec3,DVec3)>){
            let world=parent*group.transform;
            for e in &group.mesh.edges{if !e.hidden{lines.push((world.transform_point3(group.mesh.vertices[e.v0].position)+delta,world.transform_point3(group.mesh.vertices[e.v1].position)+delta));}}
            for child in &group.children{append(child,world,delta,lines);}
        }
        for group in &self.copy_assets.groups{append(group,DMat4::IDENTITY,delta,&mut lines);}
        lines
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.note_control_state(ctx.ctrl_held);

        if self.start_point.is_none() {
            self.gather_targets(scene, ctx);
            if !self.target_vertices.is_empty() || !self.target_groups.is_empty() || !self.target_planes.is_empty() || !self.target_images.is_empty() {
                self.start_point = Some(ctx.world);
            }
        } else {
            let start = self.start_point.unwrap();
            let delta = ctx.world - start;
            self.execute_move(delta, scene);
            if self.last_error.is_none(){self.reset();}
        }
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let trimmed = text.trim();

        self.replace_history=false;
        // Retyping rebuilds the complete copied topology from the pre-copy
        // mesh, as original History.undo + copy-builder replaces one command.
        let lower=trimmed.to_ascii_lowercase();
        let array=if let Some(n)=lower.strip_prefix('/') {Some((n,'/',None))}
            else if let Some(n)=lower.strip_suffix('/') {Some((n,'/',None))}
            else if let Some(n)=lower.strip_prefix(['x','*']) {Some((n,'x',None))}
            else if let Some((n,spacing))=lower.split_once(['x','*']) {Some((n,'x',(!spacing.trim().is_empty()).then_some(spacing)))}else{None};
        if let Some((number,mode,spacing))=array {
            let count:usize=number.trim().parse().map_err(|_|"Invalid array count")?;
            if count==0{return Err("Count must be greater than zero".into());}
            let step=spacing.map(|s|ingetrazo_core::units::parse_length(&s.trim().replace(',',"."),ingetrazo_core::units::LengthUnit::Meters)).transpose()?;
            if step.is_some_and(|s|!s.is_finite()||s<=0.){return Err("Array spacing must be positive and finite".into());}
            let active=self.start_point.is_some();
            let (base_mesh,group_count,delta)=if active {
                if !self.is_copy_mode||step.is_none(){return Err("A spaced array requires an active copy drag".into());}
                let direction=self.hover_point.ok_or("Move the pointer to set the array direction")?-self.start_point.unwrap();
                if direction.length()<1e-9{return Err("Move the pointer to set the array direction".into());}
                (&scene.mesh,scene.groups.len(),direction.normalize()*step.unwrap())
            }else{
                let base=self.array_base.as_ref().ok_or("No completed copy available for an array")?;
                if scene.version!=base.version {self.array_base=None;return Err("Array window closed by another model edit".into());}
                let previous=self.last_delta.ok_or("No copy displacement")?;
                let delta=if let Some(step)=step{previous.normalize_or_zero()*step}else{previous};
                (&base.mesh,base.groups,delta)
            };
            let mut mesh=base_mesh.clone();let mut groups=vec![];
            if active{for &(id,point)in &self.original_positions{if let Some(v)=mesh.vertices.get_mut(id){v.position=point;}}mesh.rebuild_spatial_index();}
            for k in 1..=count {
                let offset=delta*if mode=='/' {k as f64/count as f64}else{k as f64};
                if !offset.is_finite(){return Err("Array displacement must be finite".into());}
                mesh=ingetrazo_core::containers::paste_mesh(&mesh,&self.copy_assets.mesh,offset)?.0;
                for point in &self.copy_assets.isolated {mesh.add_vertex(*point+offset);}
                groups.extend(self.copy_assets.groups.iter().map(|g|{let mut copy=g.duplicate_instance();copy.transform=DMat4::from_translation(offset)*copy.transform;copy}));
            }
            // Original builder stamps every face before planning all hard lines.
            let segments=(1..=count).flat_map(|k|{
                let offset=delta*if mode=='/' {k as f64/count as f64}else{k as f64};
                self.copy_assets.plain.iter().map(move |&(a,b)|(a+offset,b+offset))
            }).collect::<Vec<_>>();
            if !segments.is_empty(){mesh=ingetrazo_core::drafting::add_segments(&mesh,&segments,None,None)?;}
            if active{self.revert_preview(scene);}
            let old_mesh=std::mem::replace(&mut scene.mesh,mesh);scene.groups.truncate(group_count);scene.groups.extend(groups);scene.bump_version();
            if active {self.array_base=Some(ArrayBase{mesh:old_mesh,groups:group_count,version:scene.version});self.reset();}
            else{self.array_base.as_mut().unwrap().version=scene.version;}
            self.last_delta=Some(delta);self.replace_history=!active;
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
        if let Some(error)=self.last_error.take(){return Err(error);}
        self.reset();Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    #[test]
    fn control_press_toggles_once_release_keeps_mode_and_commit_consumes_it(){
        let mut tool=MoveTool::new();tool.note_control_state(true);assert!(tool.is_copy_mode);
        tool.note_control_state(true);assert!(tool.is_copy_mode);
        tool.note_control_state(false);assert!(tool.is_copy_mode);
        tool.note_control_state(true);assert!(!tool.is_copy_mode);
        tool.note_control_state(false);tool.note_control_state(true);assert!(tool.is_copy_mode);
        tool.reset();assert!(!tool.is_copy_mode);
        tool.note_control_state(true);assert!(!tool.is_copy_mode,"held modifier does not rearm consumed operation");
        tool.note_control_state(false);tool.note_control_state(true);assert!(tool.is_copy_mode);
    }
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
    #[test]fn placement_descriptor_is_borrowed_scoped_and_retires(){
        let mut scene=Scene::new();
        let mut group=ingetrazo_core::groups::Group::new("placement");
        group.transform=DMat4::from_translation(DVec3::Y*2.)*DMat4::from_rotation_z(0.3);
        scene.groups.push(group);scene.select(SelectionItem::Group(0));
        let original=scene.groups[0].transform;let version=scene.version;
        let mut tool=MoveTool::new();let first=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));
        assert!(tool.placement_preview().is_none());tool.on_click(&mut scene,&first);
        let preview=tool.placement_preview().unwrap();assert_eq!(preview.grab_version,version);
        assert_eq!(preview.groups,&[(0,original)]);let address=preview.groups.as_ptr();
        tool.on_move(&mut scene,&ToolContext::new(DVec3::X*3.,first.ray));
        let preview=tool.placement_preview().unwrap();assert_eq!(preview.groups.as_ptr(),address);
        assert_eq!(preview.world_delta,DVec3::X*3.);assert_eq!(scene.groups[0].transform,DMat4::from_translation(preview.world_delta)*original);
        assert!(scene.version>version,"real mutation must invalidate shared caches");
        tool.on_control_state(&mut scene,true);assert!(tool.placement_preview().is_none());assert_eq!(scene.groups[0].transform,original);
        tool.on_control_state(&mut scene,false);tool.on_control_state(&mut scene,true);assert!(tool.placement_preview().is_some());
        tool.on_cancel(&mut scene);assert!(tool.placement_preview().is_none());assert_eq!(scene.groups[0].transform,original);
        tool.on_click(&mut scene,&first);tool.on_click(&mut scene,&ToolContext::new(DVec3::X,first.ray));assert!(tool.placement_preview().is_none());
        let a=scene.mesh.add_vertex(DVec3::ZERO);let b=scene.mesh.add_vertex(DVec3::Y);let edge=scene.mesh.add_edge(a,b).unwrap();scene.select(SelectionItem::Edge(edge));
        tool.on_click(&mut scene,&first);assert!(tool.placement_preview().is_none(),"mixed geometry cannot use placement-only cache");
    }
    #[test]fn live_preview_uses_absolute_delta_control_reverts_and_cancel_restores_exactly(){
        let mut scene=Scene::new();let a=scene.mesh.add_vertex(DVec3::ZERO);let b=scene.mesh.add_vertex(DVec3::X);let e=scene.mesh.add_edge(a,b).unwrap();scene.select(SelectionItem::Edge(e));let before=scene.mesh.clone();
        let mut tool=MoveTool::new();let first=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));tool.on_click(&mut scene,&first);
        for point in [DVec3::Y,DVec3::Y*2.] {tool.on_move(&mut scene,&ToolContext::new(point,first.ray));assert_eq!(scene.mesh.vertices[a].position,point);assert_eq!(scene.mesh.vertices[b].position,DVec3::X+point);}
        tool.on_control_state(&mut scene,true);assert_eq!(scene.mesh,before);assert!(tool.is_copy_mode);assert_eq!(tool.rubber_band_lines().len(),2);
        tool.on_control_state(&mut scene,false);tool.on_control_state(&mut scene,true);assert!(!tool.is_copy_mode);assert_eq!(scene.mesh.vertices[a].position,DVec3::Y*2.);
        let mut snapshot=scene.clone();tool.restore_transient_model(&mut snapshot);assert_eq!(snapshot.mesh,before);
        tool.on_cancel(&mut scene);assert_eq!(scene.mesh,before);assert!(tool.start_point.is_none());
        tool.on_click(&mut scene,&first);tool.on_move(&mut scene,&ToolContext::new(DVec3::Y,first.ray));tool.on_click(&mut scene,&ToolContext::new(DVec3::Y,first.ray));assert_eq!(scene.mesh.vertices[a].position,DVec3::Y,"commit must not double preview");
        assert_eq!(scene.mesh.add_vertex(DVec3::Y),a,"committed spatial index follows translation");
    }
    #[test]fn spaced_array_during_copy_and_retyping_units_stays_atomic(){
        let mut scene=Scene::new();scene.groups.push(ingetrazo_core::groups::Group::new("private"));scene.select(SelectionItem::Group(0));
        let mut tool=MoveTool::new();let mut first=ToolContext::new(DVec3::ZERO,Ray::new(DVec3::Z,-DVec3::Z));first.ctrl_held=true;tool.on_click(&mut scene,&first);
        tool.on_move(&mut scene,&ToolContext::new(DVec3::X*3.,first.ray));let before=scene.clone();assert!(tool.on_vcb_input(&mut scene,"3x0m").is_err());assert_eq!(scene,before);
        tool.on_vcb_input(&mut scene,"3x200cm").unwrap();assert!(tool.start_point.is_none());assert!(!tool.take_history_replacement());
        assert_eq!(scene.groups.iter().map(|g|g.transform.w_axis.x).collect::<Vec<_>>(),vec![0.,2.,4.,6.]);
        tool.on_vcb_input(&mut scene,"2*400cm").unwrap();assert!(tool.take_history_replacement());assert_eq!(scene.groups.iter().map(|g|g.transform.w_axis.x).collect::<Vec<_>>(),vec![0.,4.,8.]);
        tool.on_vcb_input(&mut scene,"x1").unwrap();assert_eq!(scene.groups.len(),2);
        let before=scene.clone();assert!(tool.on_vcb_input(&mut scene,"2xNaNm").is_err());assert_eq!(scene,before);
    }
    #[test]fn array_retyping_rebuilds_full_faces_and_groups_and_rejects_external_edits(){
        let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|scene.mesh.add_vertex(p));let f=scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();scene.mesh.faces[f].attrs.insert("vendor".into(),serde_json::json!(71));
        scene.groups.push(ingetrazo_core::groups::Group::new("private"));scene.select(SelectionItem::Face(f));scene.select(SelectionItem::Group(0));
        let mut tool=copy_at(&mut scene,DVec3::X*6.);tool.on_vcb_input(&mut scene,"3x").unwrap();
        assert_eq!(scene.mesh.faces.len(),4);assert_eq!(scene.mesh.edges.len(),16);assert_eq!(scene.groups.len(),4);
        assert!(scene.mesh.faces.iter().all(|f|f.attrs["vendor"]==serde_json::json!(71)));
        tool.on_vcb_input(&mut scene,"/3").unwrap();assert_eq!(scene.mesh.faces.len(),4);assert_eq!(scene.groups.len(),4);
        assert_eq!(scene.groups.iter().map(|g|g.transform.w_axis.x).collect::<Vec<_>>(),vec![0.,2.,4.,6.]);
        tool.on_vcb_input(&mut scene,"2x").unwrap();assert_eq!(scene.mesh.faces.len(),3);assert_eq!(scene.groups.len(),3);
        assert_eq!(scene.groups.iter().map(|g|g.transform.w_axis.x).collect::<Vec<_>>(),vec![0.,6.,12.]);
        let before=scene.clone();assert!(tool.on_vcb_input(&mut scene,"0x").is_err());assert_eq!(scene,before);
        scene.bump_version();let before=scene.clone();assert!(tool.on_vcb_input(&mut scene,"4x").is_err());assert_eq!(scene,before);
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

#[cfg(test)] mod image_move_tests{
 use super::*;use ingetrazo_core::{image_plane::ImagePlane,math::Ray};
 #[test] fn preview_cancel_copy_and_relative_command_are_separate(){
  let mut scene=Scene::new();scene.image_planes.push(ImagePlane::new("scan",DVec3::ZERO,DVec3::X,DVec3::Y));scene.select(SelectionItem::ImagePlane(0));let mut tool=MoveTool::new();
  let ctx=|p|ToolContext::new(p,Ray::new(DVec3::new(0.5,0.5,1.),-DVec3::Z));tool.on_click(&mut scene,&ctx(DVec3::ZERO));tool.on_move(&mut scene,&ctx(DVec3::X*2.));assert_eq!(scene.image_planes[0].origin,DVec3::X*2.);assert!(tool.placement_preview().is_none());
  tool.on_cancel(&mut scene);assert_eq!(scene.image_planes[0].origin,DVec3::ZERO);assert!(tool.take_image_frame_command().is_none());
  tool.on_click(&mut scene,&ctx(DVec3::ZERO));tool.on_move(&mut scene,&ctx(DVec3::X));tool.on_control_state(&mut scene,true);assert_eq!(scene.image_planes[0].origin,DVec3::ZERO);let mut end=ctx(DVec3::X*3.);end.ctrl_held=true;tool.on_click(&mut scene,&end);assert_eq!(scene.image_planes.len(),1);assert!(tool.take_image_frame_command().is_none());
  tool.on_click(&mut scene,&ctx(DVec3::ZERO));tool.on_click(&mut scene,&ctx(DVec3::Y*2.));let command=tool.take_image_frame_command().unwrap();scene.image_planes[0].origin+=DVec3::X*10.;scene.image_objects.publish(&scene.image_planes);command.apply(&mut scene.image_planes,&scene.image_objects,false);assert_eq!(scene.image_planes[0].origin,DVec3::X*10.);
 }
}

#[cfg(test)]mod image_refusal_tests{
 use super::*;use ingetrazo_core::{image_plane::ImagePlane,math::Ray};
 #[test]fn refusal_restores_exact_accumulated_preview(){
  let mut scene=Scene::new();scene.image_planes.push(ImagePlane::new("scan",DVec3::new(0.125,-1.25,2.5),DVec3::X,DVec3::Y));scene.select(SelectionItem::ImagePlane(0));let mut tool=MoveTool::new();
  let ctx=|p|ToolContext::new(p,Ray::new(DVec3::new(0.5,0.5,10.),-DVec3::Z));tool.on_click(&mut scene,&ctx(DVec3::ZERO));tool.on_move(&mut scene,&ctx(DVec3::new(0.7,1.2,-0.3)));tool.on_move(&mut scene,&ctx(DVec3::new(0.9,0.4,0.8)));let exact=scene.image_planes[0].origin;
  let mut before=scene.clone();tool.restore_transient_model(&mut before);assert!(tool.on_vcb_input(&mut scene,"invalid").is_err());scene=before;tool.restore_current_preview_model(&mut scene);assert_eq!(scene.image_planes[0].origin,exact);assert_eq!(exact.z,3.3000001907348633);
 }
}
