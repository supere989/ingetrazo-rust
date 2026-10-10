// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{Tool, ToolContext, ToolType};

/// Interactive Push/Pull tool for extruding faces into solids and recessing geometry.
#[derive(Debug, Default, Clone)]
pub struct PushPullTool {
    pub hovered_face: Option<usize>,
    pub selected_face: Option<usize>,
    pub start_point: Option<DVec3>,
    pub extrusion_distance: f64,
    pub is_dragging: bool,
    pub last_distance: Option<f64>,
    pub keep_base: bool,
    pub error: Option<String>,
}

impl PushPullTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.hovered_face = None;
        self.selected_face = None;
        self.start_point = None;
        self.extrusion_distance = 0.0;
        self.is_dragging = false;
        self.keep_base = false;
        self.error = None;
    }

    /// Prepare the whole extrusion before changing the scene. A free face
    /// becomes a closed prism; a bordered surface loses its old base. Closed
    /// solids use union/difference so inward pushes remove material and outward
    /// pushes remove the former cap/internal partitions.
    pub fn extrude_face(scene: &mut Scene, face_id: usize, distance: f64) -> Result<(), String> {
        Self::extrude_face_with_options(scene, face_id, distance, false)
    }
    /// Ctrl copies a segment and retains the original cap as a slab division.
    pub fn extrude_face_with_options(
        scene: &mut Scene,
        face_id: usize,
        distance: f64,
        keep_base: bool,
    ) -> Result<(), String> {
        use ingetrazo_core::{
            math::newell_vector,
            mesh::{Mesh, MeshFace},
            solids::{perform_solid_op, solid_report, SolidOp},
        };
        let mesh = &scene.mesh;
        let face = mesh.faces.get(face_id).ok_or("Invalid face ID")?;
        if mesh.vertices.iter().any(|v| !v.position.is_finite())
            || mesh.faces.iter().any(|f| {
                f.vertices
                    .iter()
                    .chain(f.holes.iter().flatten())
                    .any(|&v| v >= mesh.vertices.len())
            })
            || mesh.edges.iter().any(|e| {
                e.v0 >= mesh.vertices.len()
                    || e.v1 >= mesh.vertices.len()
                    || e.faces.iter().any(|&f| f >= mesh.faces.len())
            })
        {
            return Err("Document mesh has invalid references or coordinates".into());
        }
        if !distance.is_finite() {
            return Err("Distance must be finite".into());
        }
        if face.vertices.len() < 3
            || face.holes.iter().any(|h| h.len() < 3)
            || face
                .vertices
                .iter()
                .chain(face.holes.iter().flatten())
                .any(|&v| v >= mesh.vertices.len() || !mesh.vertices[v].position.is_finite())
        {
            return Err("Invalid face boundary".into());
        }
        if distance.abs() < 1e-6 {
            return Ok(());
        }
        let normal = mesh.face_normal(face_id);
        let origin = mesh.vertices[face.vertices[0]].position;
        if face
            .vertices
            .iter()
            .chain(face.holes.iter().flatten())
            .any(|&v| (mesh.vertices[v].position - origin).dot(normal).abs() > 1e-7)
        {
            return Err("Push/Pull requires a planar face".into());
        }
        let outer: Vec<_> = face
            .vertices
            .iter()
            .map(|&v| mesh.vertices[v].position)
            .collect();
        if newell_vector(&outer).length_squared() < 1e-16 {
            return Err("Push/Pull requires a nondegenerate face".into());
        }
        let mut rings = vec![outer];
        for hole in &face.holes {
            let mut positions: Vec<_> = hole.iter().map(|&v| mesh.vertices[v].position).collect();
            // Outer and hole loops must have opposite winding.
            if newell_vector(&positions).dot(normal) > 0.0 {
                positions.reverse();
            }
            rings.push(positions);
        }
        let translation = normal * distance;
        fn add_polygon(
            mesh: &mut Mesh,
            outer: &[DVec3],
            holes: &[Vec<DVec3>],
            source: &MeshFace,
        ) -> Result<(), String> {
            let ids: Vec<_> = outer.iter().map(|&p| mesh.add_vertex(p)).collect();
            let hole_ids: Vec<Vec<_>> = holes
                .iter()
                .map(|h| h.iter().map(|&p| mesh.add_vertex(p)).collect())
                .collect();
            if ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != ids.len()
                || hole_ids.iter().any(|h| {
                    h.iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != h.len()
                })
            {
                return Err("Extrusion is below the document welding tolerance".into());
            }
            let id = mesh
                .add_face(ids, hole_ids)
                .ok_or("Invalid extrusion face")?;
            mesh.faces[id].attrs = source.attrs.clone();
            mesh.faces[id].layer = source.layer.clone();
            Ok(())
        }
        let shifted: Vec<Vec<_>> = rings
            .iter()
            .map(|r| r.iter().map(|&p| p + translation).collect())
            .collect();
        let mut prism = Mesh::new();
        let mut bottom = rings.clone();
        let mut top = shifted.clone();
        if distance > 0.0 {
            for ring in &mut bottom {
                ring.reverse();
            }
        } else {
            for ring in &mut top {
                ring.reverse();
            }
        }
        add_polygon(&mut prism, &bottom[0], &bottom[1..], face)?;
        add_polygon(&mut prism, &top[0], &top[1..], face)?;
        for (base, cap) in rings.iter().zip(&shifted) {
            for i in 0..base.len() {
                let j = (i + 1) % base.len();
                let mut wall = vec![base[i], base[j], cap[j], cap[i]];
                if distance < 0.0 {
                    wall.reverse();
                }
                add_polygon(&mut prism, &wall, &[], face)?;
            }
        }
        if !solid_report(&prism).0 {
            return Err("Extrusion did not form a closed prism".into());
        }
        // Treat the selected connected face component independently so loose
        // construction edges or another object cannot disable the solid guard.
        // Derive connectivity from authoritative face rings, not cached edge
        // incidence (public face holes may have been added by an editing tool).
        let mut adjacent = std::collections::HashMap::<(usize, usize), Vec<usize>>::new();
        for (fid, original) in mesh.faces.iter().enumerate() {
            for ring in std::iter::once(&original.vertices).chain(&original.holes) {
                for i in 0..ring.len() {
                    let a = ring[i];
                    let b = ring[(i + 1) % ring.len()];
                    adjacent.entry((a.min(b), a.max(b))).or_default().push(fid);
                }
            }
        }
        let mut component_faces = std::collections::HashSet::from([face_id]);
        let mut pending = vec![face_id];
        while let Some(fid) = pending.pop() {
            let original = &mesh.faces[fid];
            for ring in std::iter::once(&original.vertices).chain(&original.holes) {
                for i in 0..ring.len() {
                    let a = ring[i];
                    let b = ring[(i + 1) % ring.len()];
                    for &neighbor in &adjacent[&(a.min(b), a.max(b))] {
                        if component_faces.insert(neighbor) {
                            pending.push(neighbor);
                        }
                    }
                }
            }
        }
        let mut component = Mesh::new();
        for (fid, original) in mesh.faces.iter().enumerate() {
            if !component_faces.contains(&fid) {
                continue;
            }
            let points: Vec<_> = original
                .vertices
                .iter()
                .map(|&v| mesh.vertices[v].position)
                .collect();
            let holes: Vec<Vec<_>> = original
                .holes
                .iter()
                .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                .collect();
            add_polygon(&mut component, &points, &holes, original)?;
            component.faces.last_mut().unwrap().interior = original.interior;
        }
        let was_solid = solid_report(&component).0;
        let mut candidate = if keep_base && component_faces.len() > 1 {
            if was_solid {
                ingetrazo_core::solids::validate_solid_mesh(&component)?;
            }
            ingetrazo_core::solids::validate_solid_mesh(&prism)?;
            let mut result = mesh.clone();
            let mut wall_planes = vec![];
            for (id, original) in prism.faces.iter().enumerate() {
                if id == 0 {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| prism.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|r| r.iter().map(|&v| prism.vertices[v].position).collect())
                    .collect();
                if id >= 2 {
                    wall_planes.push((points[0], prism.face_normal(id)));
                }
                add_polygon(&mut result, &points, &holes, original)?;
            }
            // Coplanar inward strips are overlapping boundary subdivisions,
            // not coincident walls. Rebuild each touched wall plane while the
            // kept slab and its belt remain explicit document geometry.
            for (point, normal) in wall_planes {
                let selected = result.faces.iter().position(|f| {
                    f.vertices
                        .iter()
                        .all(|&v| (result.vertices[v].position - point).dot(normal).abs() < 1e-6)
                });
                if let Some(id) = selected {
                    result = ingetrazo_core::planar::rebuild_faces(&result, &[id])?;
                }
            }
            result = ingetrazo_core::planar::stitch_boundaries(&result)?;
            ingetrazo_core::heal::orient_faces(&result)?
        } else if was_solid {
            let anchor = component.vertices[0].position;
            let signed_volume: f64 = (0..component.faces.len())
                .flat_map(|fid| component.triangulate_face(fid))
                .map(|(a, b, c)| (a - anchor).dot((b - anchor).cross(c - anchor)) / 6.0)
                .sum();
            // Direction is physical, independent of globally reversed input winding.
            let op = if distance * signed_volume > 0.0 {
                SolidOp::Union
            } else {
                SolidOp::Subtract
            };
            let mut result = perform_solid_op(op, &component, &prism)?;
            if result.faces.is_empty() {
                return Err("Push/Pull would remove the whole solid".into());
            }
            for (fid, original) in mesh.faces.iter().enumerate() {
                if component_faces.contains(&fid) {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| mesh.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                    .collect();
                add_polygon(&mut result, &points, &holes, original)?;
            }
            result
        } else {
            // An embedded sheet face is consumed in either direction. All
            // unrelated faces and loose edges remain part of the document.
            let attached = std::iter::once(&face.vertices)
                .chain(&face.holes)
                .any(|ring| {
                    (0..ring.len()).any(|i| {
                        let a = ring[i];
                        let b = ring[(i + 1) % ring.len()];
                        adjacent[&(a.min(b), a.max(b))].len() > 1
                    })
                });
            let mut result = Mesh::new();
            for (id, original) in mesh.faces.iter().enumerate() {
                if id == face_id {
                    continue;
                }
                let points: Vec<_> = original
                    .vertices
                    .iter()
                    .map(|&v| mesh.vertices[v].position)
                    .collect();
                let holes: Vec<Vec<_>> = original
                    .holes
                    .iter()
                    .map(|h| h.iter().map(|&v| mesh.vertices[v].position).collect())
                    .collect();
                add_polygon(&mut result, &points, &holes, original)?;
            }
            if attached {
                add_polygon(&mut result, &shifted[0], &shifted[1..], face)?;
                for (base, cap) in rings.iter().zip(&shifted) {
                    for i in 0..base.len() {
                        let j = (i + 1) % base.len();
                        add_polygon(&mut result, &[base[i], base[j], cap[j], cap[i]], &[], face)?;
                    }
                }
            } else {
                for original in &prism.faces {
                    let points: Vec<_> = original
                        .vertices
                        .iter()
                        .map(|&v| prism.vertices[v].position)
                        .collect();
                    let holes: Vec<Vec<_>> = original
                        .holes
                        .iter()
                        .map(|h| h.iter().map(|&v| prism.vertices[v].position).collect())
                        .collect();
                    add_polygon(&mut result, &points, &holes, original)?;
                }
            }
            result
        };
        // Reattach original wire edges and preserve metadata wherever an original
        // edge survives. This also covers the kernel-generated solid branch.
        for edge in &mesh.edges {
            if !edge.faces.is_empty()
                && edge.attrs.is_empty()
                && !edge.soft
                && edge.curve_id.is_none()
                && edge.layer.is_none()
                && !edge.hidden
            {
                continue;
            }
            let a = mesh.vertices[edge.v0].position;
            let b = mesh.vertices[edge.v1].position;
            let va = candidate.vertices.iter().position(|v| v.position == a);
            let vb = candidate.vertices.iter().position(|v| v.position == b);
            let id = if edge.faces.is_empty() {
                let va = candidate.add_vertex(a);
                let vb = candidate.add_vertex(b);
                candidate.add_edge(va, vb)
            } else {
                va.zip(vb).and_then(|(a, b)| candidate.find_edge(a, b))
            };
            if let Some(id) = id {
                candidate.edges[id].attrs = edge.attrs.clone();
                candidate.edges[id].soft = edge.soft;
                candidate.edges[id].curve_id = edge.curve_id;
                candidate.edges[id].layer = edge.layer.clone();
                candidate.edges[id].hidden = edge.hidden;
            }
        }
        for vertex in &mesh.vertices {
            if vertex.edges.is_empty() {
                candidate.add_vertex(vertex.position);
            }
        }
        candidate.preserve_vertex_uids_from(mesh);
        // No partial mutation or version bump escapes a failed preparation.
        ingetrazo_core::heal::refresh_interior(&mut candidate)?;
        if keep_base && was_solid {
            ingetrazo_core::solids::validate_solid_mesh(&candidate)
                .map_err(|e|format!("Ctrl copy would leave an invalid solid: {e}. Use a normal Push/Pull or a smaller copy."))?;
        }
        scene.mesh = candidate;
        scene.clear_selection();
        scene.bump_version();
        Ok(())
    }
}

impl Tool for PushPullTool {
    fn tool_type(&self) -> ToolType {
        ToolType::PushPull
    }

    fn name(&self) -> &'static str {
        "Push / Pull"
    }

    fn shortcut(&self) -> Option<&'static str> {
        Some("P")
    }

    fn description(&self) -> &'static str {
        "Push or pull a face to extrude it into a solid or cut it back."
    }

    fn vcb_label(&self) -> &'static str {
        "Distance"
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

    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            self.hovered_face = crate::base::pick_scene_face(scene, &ctx.ray).map(|(id, _, _)| id);
        } else if let (Some(face_id), Some(start)) = (self.selected_face, self.start_point) {
            let normal = scene.mesh.face_normal(face_id);
            self.extrusion_distance = (ctx.world - start).dot(normal);
        }
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.keep_base = ctx.ctrl_held;
        if !self.is_dragging {
            let face_pick = self
                .hovered_face
                .or_else(|| crate::base::pick_scene_face(scene, &ctx.ray).map(|(id, _, _)| id));

            if let Some(face_id) = face_pick {
                self.selected_face = Some(face_id);
                self.start_point = Some(ctx.world);
                self.is_dragging = true;
                self.extrusion_distance = 0.0;
            }
        } else if let Some(face_id) = self.selected_face {
            let dist = self.extrusion_distance;
            match Self::extrude_face_with_options(scene, face_id, dist, self.keep_base) {
                Ok(()) => {
                    self.last_distance = Some(dist);
                    self.reset();
                }
                Err(error) => self.error = Some(error),
            }
        }
    }

    fn on_double_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if let Some(last_d) = self.last_distance {
            let face_pick = self
                .hovered_face
                .or_else(|| crate::base::pick_scene_face(scene, &ctx.ray).map(|(id, _, _)| id));

            if let Some(face_id) = face_pick {
                match Self::extrude_face_with_options(scene, face_id, last_d, ctx.ctrl_held) {
                    Ok(()) => self.reset(),
                    Err(error) => self.error = Some(error),
                }
                return;
            }
        }
        self.on_click(scene, ctx);
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let dist: f64 = text
            .trim()
            .parse()
            .map_err(|e| format!("Invalid distance: {e}"))?;

        if let Some(face_id) = self.selected_face {
            if let Err(error) =
                Self::extrude_face_with_options(scene, face_id, dist, self.keep_base)
            {
                self.error = Some(error.clone());
                return Err(error);
            }
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
        } else if let Some(face_id) = self.hovered_face {
            if let Err(error) =
                Self::extrude_face_with_options(scene, face_id, dist, self.keep_base)
            {
                self.error = Some(error.clone());
                return Err(error);
            }
            self.last_distance = Some(dist);
            self.reset();
            Ok(())
        } else {
            Err("No face selected for Push/Pull".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::math::Ray;

    fn make_test_square_scene() -> Scene {
        let mut scene = Scene::new();
        let v0 = scene.mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = scene.mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = scene.mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = scene.mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));
        scene.mesh.add_face(vec![v0, v1, v2, v3], vec![]);
        scene
    }

    #[test]
    fn repeated_ctrl_invalid_source_classification_is_rejected_atomically() {
        // Original accepts two slabs but its orient pass loses both interior
        // flags (parity cancellation), leaving a nonsolid 16-face result.
        // Retain the known source failure as an atomic refusal in Rust.
        let mut scene = make_test_square_scene();
        scene.mesh.faces[0]
            .attrs
            .insert("mat".into(), serde_json::json!("Stone"));
        PushPullTool::extrude_face(&mut scene, 0, 3.).unwrap();
        let top = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 3.).abs() < 1e-8)
            })
            .unwrap();
        PushPullTool::extrude_face_with_options(&mut scene, top, 2., true).unwrap();
        let before = scene.clone();
        let top = scene
            .mesh
            .faces
            .iter()
            .position(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 5.).abs() < 1e-8)
            })
            .unwrap();
        let error = PushPullTool::extrude_face_with_options(&mut scene, top, 2., true).unwrap_err();
        assert!(error.contains("Ctrl copy would leave an invalid solid"));
        assert_eq!(scene, before);
    }

    #[test]
    fn ctrl_cap_copy_retains_slab_belts_and_inward_material() {
        for distance in [2., -2.] {
            let mut scene = make_test_square_scene();
            PushPullTool::extrude_face(&mut scene, 0, 3.).unwrap();
            let top = scene
                .mesh
                .faces
                .iter()
                .enumerate()
                .find(|(_, f)| {
                    f.vertices
                        .iter()
                        .all(|&v| (scene.mesh.vertices[v].position.z - 3.).abs() < 1e-8)
                })
                .map(|(i, _)| i)
                .unwrap();
            let ids: Vec<_> = scene.mesh.vertices.iter().map(|v| v.uid).collect();
            PushPullTool::extrude_face_with_options(&mut scene, top, distance, true).unwrap();
            assert_eq!(scene.mesh.faces.len(), 11);
            assert_eq!(scene.mesh.faces.iter().filter(|f| f.interior).count(), 1);
            assert!(
                (scene.mesh.volume().unwrap() - if distance > 0. { 5. } else { 3. }).abs() < 1e-8
            );
            assert!(ids
                .iter()
                .all(|id| scene.mesh.vertices.iter().any(|v| v.uid == *id)));
            let belt = if distance > 0. { 3. } else { 1. };
            let edges: Vec<_> = scene
                .mesh
                .edges
                .iter()
                .filter(|e| {
                    (scene.mesh.vertices[e.v0].position.z - belt).abs() < 1e-8
                        && (scene.mesh.vertices[e.v1].position.z - belt).abs() < 1e-8
                })
                .collect();
            assert_eq!(edges.len(), 4);
            assert!(edges.iter().all(|e| e.faces.len() == 3));
            ingetrazo_core::solids::validate_solid_mesh(&scene.mesh).unwrap();
        }
    }

    #[test]
    fn test_pushpull_extrude_cube() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        // Hover & click face 0
        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_move(&mut scene, &ctx0);
        assert_eq!(tool.hovered_face, Some(0));

        tool.on_click(&mut scene, &ctx0);
        assert!(tool.is_dragging);

        // Move up by 1.0
        let ctx1 = ToolContext::new(DVec3::new(0.5, 0.5, 1.0), ray);
        tool.on_move(&mut scene, &ctx1);
        assert!((tool.extrusion_distance - 1.0).abs() < 1e-4);

        // Commit click
        tool.on_click(&mut scene, &ctx1);
        assert!(!tool.is_dragging);
        assert_eq!(tool.last_distance, Some(1.0));

        // Result is a 6-sided solid cube!
        assert_eq!(scene.mesh.faces.len(), 6);
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_pushpull_vcb_and_repeat() {
        let mut scene = make_test_square_scene();
        let mut tool = PushPullTool::new();

        let ray = Ray::new(DVec3::new(0.5, 0.5, 5.0), -DVec3::Z);
        let ctx0 = ToolContext::new(DVec3::new(0.5, 0.5, 0.0), ray);
        tool.on_click(&mut scene, &ctx0);

        // Extrude 2.5 via VCB
        assert!(tool.on_vcb_input(&mut scene, "2.5").is_ok());
        assert_eq!(tool.last_distance, Some(2.5));
        assert!(scene.mesh.is_manifold_solid());
        let vol = scene.mesh.volume().unwrap();
        assert!((vol - 2.5).abs() < 1e-3);
    }
    #[test]
    fn repeated_cap_pushes_keep_whole_polygon_and_metadata() {
        let mut scene = make_test_square_scene();
        scene.mesh.faces[0]
            .attrs
            .insert("mat".into(), "stone".into());
        PushPullTool::extrude_face(&mut scene, 0, 2.0).unwrap();
        for (distance, expected) in [(1.0, 3.0), (-0.5, 2.5)] {
            let top = (0..scene.mesh.faces.len())
                .max_by(|&a, &b| {
                    scene
                        .mesh
                        .face_centroid(a)
                        .z
                        .total_cmp(&scene.mesh.face_centroid(b).z)
                })
                .unwrap();
            assert_eq!(scene.mesh.faces[top].vertices.len(), 4);
            PushPullTool::extrude_face(&mut scene, top, distance).unwrap();
            assert!((scene.mesh.volume().unwrap() - expected).abs() < 1e-8);
            assert!(scene
                .mesh
                .faces
                .iter()
                .all(|f| f.attrs.get("mat") == Some(&"stone".into())));
        }
    }

    #[test]
    fn invalid_or_sub_weld_extrusion_is_atomic() {
        let mut scene = make_test_square_scene();
        for distance in [0.00002, f64::NAN, f64::INFINITY] {
            let before = scene.clone();
            assert!(PushPullTool::extrude_face(&mut scene, 0, distance).is_err());
            assert_eq!(scene, before);
        }
        scene.mesh.vertices[0].position.z = 0.01;
        let before = scene.clone();
        assert!(PushPullTool::extrude_face(&mut scene, 0, 1.0).is_err());
        assert_eq!(scene, before);
    }
}
