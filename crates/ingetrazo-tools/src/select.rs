// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use std::collections::{HashSet, VecDeque};
use glam::DVec3;
use ingetrazo_core::math::BoundingBox;
use ingetrazo_core::scene::{Scene, SelectionItem};

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Selection tool for picking single entities, face+bounds, and connected components.
#[derive(Debug, Default, Clone)]
pub struct SelectTool {
    pick_viewport:Option<(ingetrazo_core::screen_pick::PixelViewport,DVec3)>,
    click_index:crate::select_pick::ClickIndex,
    annotation_hits:crate::base::AnnotationHits,
    annotation_leader:Option<u64>,
    annotation_edit:Option<crate::base::AnnotationEditTarget>,
    annotation_hover:Option<crate::base::AnnotationEditTarget>,
    annotation_hover_index:Option<usize>,
    hover_context:Vec<uuid::Uuid>,
    pub box_start: Option<DVec3>,
    pub box_end: Option<DVec3>,
    pub is_box_selecting: bool,
}

impl SelectTool {
    pub fn new() -> Self {
        Self::default()
    }

    fn annotation_index(&self,scene:&Scene,target:crate::base::AnnotationEditTarget)->Option<usize>{
        use crate::base::AnnotationEditTarget;
        match target{
            AnnotationEditTarget::Label(id)=>self.annotation_hover_index.filter(|i|self.annotation_hover==Some(target)&&scene.text_labels.get(*i).is_some_and(|t|t.history_id==id)).or_else(||scene.text_labels.iter().position(|t|t.history_id==id)),
            AnnotationEditTarget::Dimension(id)=>self.annotation_hover_index.filter(|i|self.annotation_hover==Some(target)&&scene.dimensions.get(*i).is_some_and(|d|d.id==id)).or_else(||scene.dimensions.iter().position(|d|d.id==id)),
        }
    }

    fn expanded(scene:&Scene,target:SelectionItem)->Vec<SelectionItem> {
        match target {
                SelectionItem::Face(id)=>{
                    let mut faces=vec![id];let mut seen=HashSet::from([id]);let mut index=0;
                    while index<faces.len(){
                        let face=faces[index];index+=1;
                        for &v in &scene.mesh.faces[face].vertices {for &e in &scene.mesh.vertices[v].edges {let edge=&scene.mesh.edges[e];if edge.soft&&edge.faces.contains(&face){for &next in &edge.faces {if seen.insert(next){faces.push(next);}}}}}
                    }
                    faces.into_iter().map(SelectionItem::Face).collect::<Vec<_>>()
                },
                SelectionItem::Edge(id)=>if let Some(curve)=scene.mesh.edges[id].curve_id {scene.mesh.edges.iter().enumerate().filter(|(_,e)|e.curve_id==Some(curve)).map(|(id,_)|SelectionItem::Edge(id)).collect()}else{vec![target]},
                _=>vec![target],
            }
    }
    fn apply_selection(scene:&mut Scene,ctx:&ToolContext,targets:Vec<SelectionItem>){
        if !ctx.shift_held&&!ctx.ctrl_held{scene.clear_selection();}
        for target in targets {
            if ctx.shift_held&&ctx.ctrl_held{scene.deselect(&target);}
            else if ctx.shift_held&&scene.is_selected(&target){scene.deselect(&target);}
            else{scene.select(target);}
        }
    }
    pub fn select_pixels(
        selection: &crate::base::BoxSelection,
        scene: &mut Scene,
    ) -> Result<(), String> {
        use glam::{DMat4, DVec2};
        let r = selection.bounds();
        if !selection.start.is_finite()
            || !selection.end.is_finite()
            || !selection.viewport.matrix.is_finite()
            || !selection.viewport.width.is_finite()
            || !selection.viewport.height.is_finite()
            || selection.viewport.width <= 0.
            || selection.viewport.height <= 0.
        {
            return Err("Invalid selection rectangle".into());
        }
        let inside = |p: DVec2| p.x >= r[0] && p.x <= r[2] && p.y >= r[1] && p.y <= r[3];
        let segment = |a: DVec2, b: DVec2| {
            if inside(a) || inside(b) {
                return true;
            }
            let ccw = |a: DVec2, b: DVec2, c: DVec2| {
                (c.y - a.y) * (b.x - a.x) - (b.y - a.y) * (c.x - a.x)
            };
            let corners = [
                DVec2::new(r[0], r[1]),
                DVec2::new(r[2], r[1]),
                DVec2::new(r[2], r[3]),
                DVec2::new(r[0], r[3]),
            ];
            (0..4).any(|i| {
                let p = corners[i];
                let q = corners[(i + 1) % 4];
                (ccw(p, q, a) > 0.) != (ccw(p, q, b) > 0.)
                    && (ccw(a, b, p) > 0.) != (ccw(a, b, q) > 0.)
            })
        };
        let project = |p| selection.viewport.project_for_snapping(p);
        let hit_segment = |a, b| match (project(a), project(b)) {
            (Some(a), Some(b)) => {
                if selection.crossing() {
                    segment(a, b)
                } else {
                    inside(a) && inside(b)
                }
            }
            _ => false,
        };
        for f in &scene.mesh.faces {
            for &vertex in f.vertices.iter().chain(f.holes.iter().flatten()) {
                if vertex >= scene.mesh.vertices.len() {
                    return Err("Invalid face vertex".into());
                }
            }
        }
        let mut picked = Vec::new();
        // Operative Viewport uses its loose-edge projection index. Faces are
        // admitted from radial edges, including hole boundaries, rather than
        // polygon-area overlap (a box wholly inside a face does not catch it).
        let mut face_edges = vec![0usize; scene.mesh.faces.len()];
        let mut face_hits = vec![0usize; scene.mesh.faces.len()];
        for (id, e) in scene.mesh.edges.iter().enumerate() {
            let (a, b) = (
                scene
                    .mesh
                    .vertices
                    .get(e.v0)
                    .ok_or("Invalid edge endpoint")?,
                scene
                    .mesh
                    .vertices
                    .get(e.v1)
                    .ok_or("Invalid edge endpoint")?,
            );
            let hit = hit_segment(a.position, b.position);
            for &face in &e.faces {
                let count = face_edges.get_mut(face).ok_or("Invalid edge radial face")?;
                *count += 1;
                if hit {
                    face_hits[face] += 1;
                }
            }
            if hit && !e.hidden && scene.is_layer_selectable(e.layer.as_deref()) {
                picked.push(SelectionItem::Edge(id));
            }
        }
        for (id, _) in scene.mesh.faces.iter().enumerate() {
            if !scene.mesh_face_selectable(&scene.mesh, id) {
                continue;
            }
            if face_edges[id] > 0
                && if selection.crossing() {
                    face_hits[id] > 0
                } else {
                    face_hits[id] == face_edges[id]
                }
            {
                picked.push(SelectionItem::Face(id));
            }
        }
        fn placements<'a>(
            g: &'a ingetrazo_core::groups::Group,
            parent: DMat4,
            out: &mut Vec<(&'a ingetrazo_core::mesh::Mesh, DMat4)>,
        ) {
            let m = ingetrazo_core::change_axes::multiply(parent, g.transform);
            if !g.mesh.vertices.is_empty() {
                out.push((&g.mesh, m));
            }
            for c in &g.children {
                placements(c, m, out);
            }
        }
        for (id, g) in scene.groups.iter().enumerate() {
            if !scene.group_selectable(g) {
                continue;
            }
            let mut places = Vec::new();
            placements(g, DMat4::IDENTITY, &mut places);
            if places.is_empty() {
                continue;
            }
            // The operative group chunk contains triangle corners and ordinary
            // hard-edge pairs, not orphan vertices. Only an empty chunk falls
            // back to the source's complete per-vertex placement walk.
            let mut packet = Vec::new();
            let mut pairs = Vec::new();
            for &(mesh, m) in &places {
                for (face, f) in mesh.faces.iter().enumerate() {
                    for &v in f.vertices.iter().chain(f.holes.iter().flatten()) {
                        if v >= mesh.vertices.len() {
                            return Err("Invalid group face vertex".into());
                        }
                    }
                    if f.attrs
                        .get("hidden")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    for (a, b, c) in mesh.triangulate_face(face) {
                        for point in [a, b, c] {
                            packet.push(ingetrazo_core::change_axes::map(m, point));
                        }
                    }
                }
                for e in &mesh.edges {
                    let (a, b) = (
                        mesh.vertices.get(e.v0).ok_or("Invalid group edge")?,
                        mesh.vertices.get(e.v1).ok_or("Invalid group edge")?,
                    );
                    if e.hidden || e.soft {
                        continue;
                    }
                    let (a, b) = (
                        ingetrazo_core::change_axes::map(m, a.position),
                        ingetrazo_core::change_axes::map(m, b.position),
                    );
                    packet.extend([a, b]);
                    pairs.push((a, b));
                }
            }
            let mut hit = !selection.crossing();
            if !packet.is_empty() {
                hit = if selection.crossing() {
                    packet.iter().any(|&p| project(p).is_some_and(&inside))
                        || pairs.iter().any(|&(a, b)| hit_segment(a, b))
                } else {
                    packet.iter().all(|&p| project(p).is_some_and(&inside))
                };
            } else {
                'places: for (mesh, m) in places {
                    for v in &mesh.vertices {
                        let p = project(ingetrazo_core::change_axes::map(m, v.position));
                        if selection.crossing() {
                            if p.is_some_and(&inside) {
                                hit = true;
                                break 'places;
                            }
                        } else if !p.is_some_and(&inside) {
                            hit = false;
                            break 'places;
                        }
                    }
                    if selection.crossing() {
                        for e in &mesh.edges {
                            let (a, b) = (
                                mesh.vertices.get(e.v0).ok_or("Invalid group edge")?,
                                mesh.vertices.get(e.v1).ok_or("Invalid group edge")?,
                            );
                            if hit_segment(
                                ingetrazo_core::change_axes::map(m, a.position),
                                ingetrazo_core::change_axes::map(m, b.position),
                            ) {
                                hit = true;
                                break 'places;
                            }
                        }
                    }
                }
            }
            if hit {
                picked.push(SelectionItem::Group(id));
            }
        }
        let guide_segment = |a: DVec3, b: DVec3| {
            let m = selection.viewport.matrix.as_mat4();
            let wa = (m * a.as_vec3().extend(1.)).w;
            let wb = (m * b.as_vec3().extend(1.)).w;
            if wa <= 1e-3 && wb <= 1e-3 {
                return false;
            }
            let (a, b) = if wa <= 1e-3 || wb <= 1e-3 {
                let t = (1e-3 - wa) / (wb - wa);
                let cut = (a.as_vec3() + (b.as_vec3() - a.as_vec3()) * t).as_dvec3();
                if wa <= 1e-3 {
                    (cut, b)
                } else {
                    (a, cut)
                }
            } else {
                (a, b)
            };
            hit_segment(a, b)
        };
        for (id, g) in scene.guides.iter().enumerate() {
            if !scene.is_layer_selectable(None) {
                continue;
            }
            use ingetrazo_core::guides::GuideType;
            let hit = match g.kind {
                GuideType::Point(p) => project(p).is_some_and(&inside),
                GuideType::InfiniteLine { point, dir } => {
                    selection.crossing()
                        && guide_segment(
                            (point.as_vec3() - dir.as_vec3() * 1e4).as_dvec3(),
                            (point.as_vec3() + dir.as_vec3() * 1e4).as_dvec3(),
                        )
                }
                GuideType::Segment { a, b } => selection.crossing() && hit_segment(a, b),
            };
            if hit {
                picked.push(SelectionItem::Guide(id));
            }
        }
        for (id, d) in scene.dimensions.iter().enumerate() {
            if scene.is_layer_selectable(d.layer.as_deref()) {
                let (a, b) = d.line_points();
                if hit_segment(a, b) {
                    picked.push(SelectionItem::Dimension(id));
                }
            }
        }
        for (id, l) in scene.text_labels.iter().enumerate() {
            if !scene.is_layer_selectable(l.layer.as_deref()) {
                continue;
            }
            let (a, p) = (project(l.anchor), project(l.position()));
            let hit = p.is_some_and(|p| {
                if selection.crossing() {
                    inside(p) || a.is_some_and(|a| segment(a, p))
                } else {
                    inside(p) && a.is_none_or(&inside)
                }
            });
            if hit {
                picked.push(SelectionItem::TextLabel(id));
            }
        }
        if !selection.shift && !selection.ctrl {
            scene.clear_selection();
        }
        for target in picked {
            if selection.shift && selection.ctrl {
                scene.deselect(&target);
            } else if selection.shift && scene.is_selected(&target) {
                scene.deselect(&target);
            } else {
                scene.select(target);
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.annotation_hits=Default::default();
        self.annotation_leader=None;
        self.annotation_edit=None;
        self.annotation_hover=None;
        self.box_start = None;
        self.box_end = None;
        self.is_box_selecting = false;
    }

    /// Select all connected vertices, edges, and faces in the mesh component.
    pub fn select_all_connected(scene: &mut Scene, start_item: SelectionItem) {
        let mut visited_verts = HashSet::new();
        let mut queue = VecDeque::new();

        // Seed queue from start item
        match start_item {
            SelectionItem::Guide(_) | SelectionItem::Dimension(_) | SelectionItem::TextLabel(_) => {scene.select(start_item);return;}
            SelectionItem::ContextGroup{context,group}=>{scene.select(SelectionItem::ContextGroup{context,group});return;}
            SelectionItem::ContextFace{context,face,vertices}=>{scene.select(SelectionItem::ContextFace{context,face,vertices});return;}
            SelectionItem::ImagePlane(id)=>{scene.select(SelectionItem::ImagePlane(id));return;}
            SelectionItem::SectionPlane(id)=>{scene.select(SelectionItem::SectionPlane(id));return;}
            SelectionItem::Vertex(vid) => {
                queue.push_back(vid);
                visited_verts.insert(vid);
            }
            SelectionItem::Edge(eid) => {
                if eid < scene.mesh.edges.len() {
                    let e = &scene.mesh.edges[eid];
                    queue.push_back(e.v0);
                    queue.push_back(e.v1);
                    visited_verts.insert(e.v0);
                    visited_verts.insert(e.v1);
                }
            }
            SelectionItem::Face(fid) => {
                if fid < scene.mesh.faces.len() {
                    for &v in &scene.mesh.faces[fid].vertices {
                        queue.push_back(v);
                        visited_verts.insert(v);
                    }
                }
            }
            SelectionItem::Group(gid) => {
                scene.select(SelectionItem::Group(gid));
                return;
            }
            SelectionItem::GeoPoint(pid) => {
                scene.select(SelectionItem::GeoPoint(pid));
                return;
            }
            SelectionItem::GeoPath(pid) => {
                scene.select(SelectionItem::GeoPath(pid));
                return;
            }
        }

        // BFS through incident edges
        while let Some(vid) = queue.pop_front() {
            if vid >= scene.mesh.vertices.len() {
                continue;
            }
            for &eid in &scene.mesh.vertices[vid].edges {
                if eid < scene.mesh.edges.len() {
                    let other = scene.mesh.edges[eid].other_vertex(vid);
                    if let Some(next_v) = other {
                        if !visited_verts.contains(&next_v) {
                            visited_verts.insert(next_v);
                            queue.push_back(next_v);
                        }
                    }
                }
            }
        }

        // Collect matching edge IDs and face IDs
        let matching_edges: Vec<usize> = scene
            .mesh
            .edges
            .iter()
            .enumerate()
            .filter_map(|(eid, e)| {
                if visited_verts.contains(&e.v0) && visited_verts.contains(&e.v1) {
                    Some(eid)
                } else {
                    None
                }
            })
            .collect();

        let matching_faces: Vec<usize> = scene
            .mesh
            .faces
            .iter()
            .enumerate()
            .filter_map(|(fid, f)| {
                if !f.vertices.is_empty() && f.vertices.iter().all(|v| visited_verts.contains(v)) {
                    Some(fid)
                } else {
                    None
                }
            })
            .collect();

        // Mutate scene selection
        for &v in &visited_verts {
            scene.select(SelectionItem::Vertex(v));
        }
        for eid in matching_edges {
            scene.select(SelectionItem::Edge(eid));
        }
        for fid in matching_faces {
            scene.select(SelectionItem::Face(fid));
        }
    }

    /// Box selection of entities inside 3D bounding box.
    pub fn select_in_box(&self, bbox: &BoundingBox, scene: &mut Scene, add_to_selection: bool) {
        if !add_to_selection {
            scene.clear_selection();
        }

        let selected_verts: Vec<usize> = scene
            .mesh
            .vertices
            .iter()
            .enumerate()
            .filter_map(|(vid, v)| {
                let p = v.position;
                if p.x >= bbox.min.x && p.x <= bbox.max.x
                    && p.y >= bbox.min.y && p.y <= bbox.max.y
                    && p.z >= bbox.min.z && p.z <= bbox.max.z
                {
                    Some(vid)
                } else {
                    None
                }
            })
            .collect();

        let selected_edges: Vec<usize> = scene
            .mesh
            .edges
            .iter()
            .enumerate()
            .filter_map(|(eid, e)| {
                let p0 = scene.mesh.vertices[e.v0].position;
                let p1 = scene.mesh.vertices[e.v1].position;
                let in_b = |p: DVec3| {
                    p.x >= bbox.min.x && p.x <= bbox.max.x
                        && p.y >= bbox.min.y && p.y <= bbox.max.y
                        && p.z >= bbox.min.z && p.z <= bbox.max.z
                };
                if in_b(p0) || in_b(p1) {
                    Some(eid)
                } else {
                    None
                }
            })
            .collect();

        let selected_faces: Vec<usize> = scene
            .mesh
            .faces
            .iter()
            .enumerate()
            .filter_map(|(fid, _)| {
                let c = scene.mesh.face_centroid(fid);
                if c.x >= bbox.min.x && c.x <= bbox.max.x
                    && c.y >= bbox.min.y && c.y <= bbox.max.y
                    && c.z >= bbox.min.z && c.z <= bbox.max.z
                {
                    Some(fid)
                } else {
                    None
                }
            })
            .collect();

        for vid in selected_verts {
            scene.select(SelectionItem::Vertex(vid));
        }
        for eid in selected_edges {
            scene.select(SelectionItem::Edge(eid));
        }
        for fid in selected_faces {
            scene.select(SelectionItem::Face(fid));
        }
    }
}

impl Tool for SelectTool {
    fn wants_annotation_hits(&self)->bool {true}
    fn note_annotation_hits(&mut self,hits:crate::base::AnnotationHits){self.annotation_hits=hits;}
    fn note_annotation_leader_hit(&mut self,hit:Option<u64>){self.annotation_leader=hit;}
    fn note_annotation_pick_error(&mut self,_error:String){self.annotation_hits=crate::base::AnnotationHits::default();self.annotation_leader=None;self.annotation_hover=None;}
    fn take_annotation_edit_request(&mut self)->Option<crate::base::AnnotationEditTarget>{self.annotation_edit.take()}
    fn annotation_edit_target(&mut self,scene:&Scene,ctx:&ToolContext)->Option<crate::base::AnnotationEditTarget>{
        use crate::base::AnnotationEditTarget;
        if let Some(id)=self.annotation_hits.label.filter(|id|self.annotation_index(scene,AnnotationEditTarget::Label(*id)).is_some_and(|i|scene.is_layer_selectable(scene.text_labels[i].layer.as_deref()))){return Some(AnnotationEditTarget::Label(id));}
        if self.annotation_hits.dimension.is_none()&&self.annotation_leader.is_none(){return None;}
        let edge=if let(Some((viewport,eye)),Some(cursor))=(self.pick_viewport,ctx.screen){self.click_index.edge(scene,viewport,cursor,eye)}else{crate::base::pick_scene_edge(scene,&ctx.ray,0.2)};
        if edge.is_some()||crate::base::pick_scene_group(scene,&ctx.ray,0.2).is_some(){return None;}
        if let Some(id)=self.annotation_hits.dimension.filter(|id|self.annotation_index(scene,AnnotationEditTarget::Dimension(*id)).is_some_and(|i|scene.is_layer_selectable(scene.dimensions[i].layer.as_deref()))){return Some(AnnotationEditTarget::Dimension(id));}
        self.annotation_leader.filter(|id|self.annotation_index(scene,AnnotationEditTarget::Label(*id)).is_some_and(|i|scene.is_layer_selectable(scene.text_labels[i].layer.as_deref()))).map(AnnotationEditTarget::Label)
    }


    fn on_move(&mut self,scene:&mut Scene,ctx:&ToolContext){
        let target=self.annotation_edit_target(scene,ctx);
        let index=target.and_then(|t|self.annotation_index(scene,t));
        self.annotation_hover=target;
        self.annotation_hover_index=index;
        if self.hover_context!=scene.history_context{self.hover_context.clone_from(&scene.history_context);}
    }
    fn clear_annotation_hover(&mut self){self.annotation_hover=None;}
    fn annotation_hover_target(&self,scene:&Scene)->Option<crate::base::AnnotationEditTarget>{
        if self.hover_context!=scene.history_context{return None;}
        self.annotation_hover.filter(|target|match target{
            crate::base::AnnotationEditTarget::Label(id)=>self.annotation_hover_index.and_then(|i|scene.text_labels.get(i)).filter(|t|t.history_id==*id).or_else(||scene.text_labels.iter().find(|t|t.history_id==*id)).is_some_and(|t|scene.is_layer_selectable(t.layer.as_deref())),
            crate::base::AnnotationEditTarget::Dimension(id)=>self.annotation_hover_index.and_then(|i|scene.dimensions.get(i)).filter(|d|d.id==*id).or_else(||scene.dimensions.iter().find(|d|d.id==*id)).is_some_and(|d|!d.native_document["hidden"].as_bool().unwrap_or(false)&&scene.is_layer_selectable(d.layer.as_deref())),
        })
    }

    fn box_select(&mut self, selection:&crate::base::BoxSelection,scene:&mut Scene)->Result<(),String>{Self::select_pixels(selection,scene)}
    fn tool_type(&self) -> ToolType {
        ToolType::Select
    }

    fn name(&self) -> &'static str {
        "Select"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("Space")
    }

    fn description(&self) -> &'static str {
        "Select edges, faces, and groups in the viewport."
    }

    fn on_activate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_deactivate(&mut self, _scene: &mut Scene) {
        self.reset();
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        // Source Select inherits the no-op cancel; retiring a box must not
        // erase an otherwise live hover (tool deactivation/leave do that).
        let hover=self.annotation_hover;
        let index=self.annotation_hover_index;
        self.reset();
        self.annotation_hover=hover;
        self.annotation_hover_index=index;
    }

    fn note_pick_viewport(&mut self,viewport:ingetrazo_core::screen_pick::PixelViewport,eye:DVec3,model_revision:u64){self.pick_viewport=Some((viewport,eye));self.click_index.note_model_revision(model_revision);}

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if let Some(id)=self.annotation_hits.label.and_then(|token|scene.text_labels.iter().position(|t|t.history_id==token&&scene.is_layer_selectable(t.layer.as_deref()))){Self::apply_selection(scene,ctx,vec![SelectionItem::TextLabel(id)]);return;}
        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = if let (Some((viewport,eye)),Some(cursor))=(self.pick_viewport,ctx.screen){self.click_index.edge(scene,viewport,cursor,eye)}else{crate::base::pick_scene_edge(scene, &ctx.ray, 0.2)};
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).map(|(id,_)|id);

        let dimension=self.annotation_hits.dimension.and_then(|token|scene.dimensions.iter().position(|d|d.id==token&&scene.is_layer_selectable(d.layer.as_deref())));
        let leader=self.annotation_leader.and_then(|token|scene.text_labels.iter().position(|t|t.history_id==token&&scene.is_layer_selectable(t.layer.as_deref())));
        let item = if let Some(eid) = hit_edge {
            Some(SelectionItem::Edge(eid))
        } else if let Some(id)=hit_group {Some(SelectionItem::Group(id))} else if let Some(id)=dimension {Some(SelectionItem::Dimension(id))} else if let Some(id)=leader {Some(SelectionItem::TextLabel(id))} else if let Some(id)=hit_plane {Some(SelectionItem::SectionPlane(id))} else if let Some(fid) = hit_face {
            Some(SelectionItem::Face(fid))
        } else {
            scene.image_plane_hit(&ctx.ray,ingetrazo_core::image_plane::ImagePickMode::Selectable).map(|(i,_)|SelectionItem::ImagePlane(i))
        };

        if let Some(target) = item {
            let targets=Self::expanded(scene,target);
            Self::apply_selection(scene,ctx,targets);
        } else if !ctx.shift_held && !ctx.ctrl_held {
            scene.clear_selection();
        }
    }

    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if let Some(target)=self.annotation_edit_target(scene,ctx){self.annotation_edit=Some(target);return;}

        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = if let (Some((viewport,eye)),Some(cursor))=(self.pick_viewport,ctx.screen){self.click_index.edge(scene,viewport,cursor,eye)}else{crate::base::pick_scene_edge(scene, &ctx.ray, 0.2)};
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).map(|(id,_)|id);

        if let Some(id)=hit_plane{scene.clear_selection();scene.select(SelectionItem::SectionPlane(id));return;}
        if let Some(id)=hit_group.filter(|_|hit_edge.is_none()){scene.clear_selection();scene.select(SelectionItem::Group(id));return;}
        let target=hit_edge.map(SelectionItem::Edge).or_else(||hit_face.map(SelectionItem::Face));
        if let Some(target)=target {
            let mut targets=Self::expanded(scene,target.clone());
            for item in targets.clone() {
                match item {
                    SelectionItem::Face(id)=>{let f=&scene.mesh.faces[id];for ring in std::iter::once(&f.vertices).chain(f.holes.iter()){for i in 0..ring.len(){if let Some(edge)=scene.mesh.find_edge(ring[i],ring[(i+1)%ring.len()]){targets.push(SelectionItem::Edge(edge));}}}},
                    SelectionItem::Edge(id)=>targets.extend(scene.mesh.edges[id].faces.iter().copied().map(SelectionItem::Face)),
                    _=>(),
                }
            }
            let mut seen=HashSet::new();targets.retain(|t|seen.insert(t.clone()));
            Self::apply_selection(scene,ctx,targets);
        }else{self.on_click(scene,ctx);}
    }

    fn on_triple_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = if let (Some((viewport,eye)),Some(cursor))=(self.pick_viewport,ctx.screen){self.click_index.edge(scene,viewport,cursor,eye)}else{crate::base::pick_scene_edge(scene, &ctx.ray, 0.2)};
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).map(|(id,_)|id);

        if let Some(id)=hit_plane{scene.clear_selection();scene.select(SelectionItem::SectionPlane(id));return;}
        if let Some(id)=hit_group{scene.clear_selection();scene.select(SelectionItem::Group(id));return;}
        if let Some(fid) = hit_face {
            Self::select_all_connected(scene, SelectionItem::Face(fid));
        } else if let Some(eid) = hit_edge {
            Self::select_all_connected(scene, SelectionItem::Edge(eid));
        } else {
            self.on_click(scene, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    fn make_test_cube() -> Scene {
        let mut scene = Scene::new();
        let v000 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v100 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v110 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v010 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        let v001 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 1.0));
        let v101 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 1.0));
        let v111 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 1.0));
        let v011 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 1.0));

        scene.mesh.add_face(vec![v000, v010, v110, v100], vec![]);
        scene.mesh.add_face(vec![v001, v101, v111, v011], vec![]);
        scene.mesh.add_face(vec![v000, v100, v101, v001], vec![]);
        scene.mesh.add_face(vec![v100, v110, v111, v101], vec![]);
        scene.mesh.add_face(vec![v110, v010, v011, v111], vec![]);
        scene.mesh.add_face(vec![v010, v000, v001, v011], vec![]);
        scene
    }

    #[test]
    fn image_is_last_selection_candidate_and_combined_modifier_removes_only() {
        let mut scene=Scene::new();scene.image_planes.push(ingetrazo_core::image_plane::ImagePlane::new("scan",DVec3::Z,DVec3::X*2.,DVec3::Y*2.));
        let ray=Ray::new(DVec3::new(0.5,0.5,10.),-DVec3::Z);let mut tool=SelectTool::new();
        tool.on_click(&mut scene,&ToolContext::with_modifiers(DVec3::ZERO,ray,true,true,false));assert!(scene.selection.is_empty());
        tool.on_click(&mut scene,&ToolContext::new(DVec3::ZERO,ray));assert!(scene.is_selected(&SelectionItem::ImagePlane(0)));
        scene.mesh.add_vertex(DVec3::ZERO);scene.mesh.add_vertex(DVec3::X*2.);scene.mesh.add_vertex(DVec3::Y*2.);scene.mesh.add_face(vec![0,1,2],vec![]);
        tool.on_click(&mut scene,&ToolContext::new(DVec3::ZERO,ray));assert!(scene.is_selected(&SelectionItem::Face(0)));assert!(!scene.is_selected(&SelectionItem::ImagePlane(0)));
    }
    #[test]
    fn test_select_tool_single_click() {
        let mut scene = make_test_cube();
        let mut tool = SelectTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);

        tool.on_click(&mut scene, &ctx);
        assert_eq!(scene.selection.len(), 1);
        assert!(scene.is_selected(&SelectionItem::Face(1)));
    }

    #[test]
    fn test_select_tool_double_click() {
        let mut scene = make_test_cube();
        let mut tool = SelectTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);

        tool.on_double_click(&mut scene, &ctx);
        assert_eq!(scene.selection.len(), 5);
        assert!(scene.is_selected(&SelectionItem::Face(1)));
    }

    #[test]
    fn test_select_tool_triple_click_all_connected() {
        let mut scene = make_test_cube();
        let mut tool = SelectTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);

        tool.on_triple_click(&mut scene, &ctx);
        assert_eq!(scene.selection.len(), 26);
    }
}
