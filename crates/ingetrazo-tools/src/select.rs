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
    pub box_start: Option<DVec3>,
    pub box_end: Option<DVec3>,
    pub is_box_selecting: bool,
}

impl SelectTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
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
        self.reset();
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2);
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).filter(|(_,t)|face_hit.is_none_or(|(_,distance,_)|*t<distance)).map(|(id,_)|id);

        let item = if let Some(id)=hit_plane{Some(SelectionItem::SectionPlane(id))}else if let Some(id)=hit_group{Some(SelectionItem::Group(id))}else if let Some(eid) = hit_edge {
            Some(SelectionItem::Edge(eid))
        } else if let Some(fid) = hit_face {
            Some(SelectionItem::Face(fid))
        } else {
            scene.image_plane_hit(&ctx.ray,ingetrazo_core::image_plane::ImagePickMode::Selectable).map(|(i,_)|SelectionItem::ImagePlane(i))
        };

        if let Some(target) = item {
            if ctx.shift_held && ctx.ctrl_held {
                scene.deselect(&target);
            } else if ctx.shift_held {
                // Toggle
                if scene.is_selected(&target) {
                    scene.deselect(&target);
                } else {
                    scene.select(target);
                }
            } else if ctx.ctrl_held {
                scene.select(target);
            } else {
                scene.clear_selection();
                scene.select(target);
            }
        } else if !ctx.shift_held && !ctx.ctrl_held {
            scene.clear_selection();
        }
    }

    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2);
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).filter(|(_,t)|face_hit.is_none_or(|(_,distance,_)|*t<distance)).map(|(id,_)|id);

        if let Some(id)=hit_plane{scene.clear_selection();scene.select(SelectionItem::SectionPlane(id));return;}
        if let Some(id)=hit_group{scene.clear_selection();scene.select(SelectionItem::Group(id));return;}
        if let Some(fid) = hit_face {
            let mut bounding_edges = Vec::new();
            let f = &scene.mesh.faces[fid];
            let n = f.vertices.len();
            for i in 0..n {
                let v0 = f.vertices[i];
                let v1 = f.vertices[(i + 1) % n];
                if let Some(eid) = scene.mesh.find_edge(v0, v1) {
                    bounding_edges.push(eid);
                }
            }

            scene.select(SelectionItem::Face(fid));
            for eid in bounding_edges {
                scene.select(SelectionItem::Edge(eid));
            }
        } else if let Some(eid) = hit_edge {
            let incident_faces = scene.mesh.edges[eid].faces.clone();
            scene.select(SelectionItem::Edge(eid));
            for f in incident_faces {
                scene.select(SelectionItem::Face(f));
            }
        } else {
            self.on_click(scene, ctx);
        }
    }

    fn on_triple_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        let hit_plane=crate::base::pick_section_plane(scene,&ctx.ray,0.2);
        let hit_edge = crate::base::pick_scene_edge(scene, &ctx.ray, 0.2);
        let face_hit=crate::base::pick_scene_face(scene,&ctx.ray);let hit_face=face_hit.map(|(id,_,_)|id);
        let hit_group=crate::base::pick_scene_group(scene,&ctx.ray,0.2).filter(|(_,t)|face_hit.is_none_or(|(_,distance,_)|*t<distance)).map(|(id,_)|id);

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
