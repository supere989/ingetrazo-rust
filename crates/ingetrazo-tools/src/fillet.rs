// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Fillet tool: round edges and corners.

use glam::DVec3;
use ingetrazo_core::scene::Scene;

use crate::base::{pick_edge, Tool, ToolContext, ToolType};

use ingetrazo_core::{
    math::{newell_vector, PointKey},
    mesh::{Mesh, MeshFace},
};
use std::collections::HashMap;

fn inward(mesh: &Mesh, face: usize, a: usize, b: usize) -> Result<DVec3, String> {
    let f = &mesh.faces[face];
    for ring in std::iter::once(&f.vertices).chain(&f.holes) {
        for i in 0..ring.len() {
            let (v, w) = (ring[i], ring[(i + 1) % ring.len()]);
            if (v == a && w == b) || (v == b && w == a) {
                let direction = mesh.vertices[w].position - mesh.vertices[v].position;
                let into = mesh.face_normal(face).cross(direction).normalize_or_zero();
                if into.length_squared() > 0.5 {
                    return Ok(into);
                }
            }
        }
    }
    Err("Cannot read the faces around this edge".into())
}

fn inside_face(mesh: &Mesh, face: usize, p: DVec3) -> bool {
    let normal = mesh.face_normal(face).abs();
    let axis = if normal.x >= normal.y && normal.x >= normal.z {
        0
    } else if normal.y >= normal.z {
        1
    } else {
        2
    };
    let xy = |p: DVec3| match axis {
        0 => (p.y, p.z),
        1 => (p.x, p.z),
        _ => (p.x, p.y),
    };
    let (x, y) = xy(p);
    let contains = |ring: &Vec<usize>| {
        let mut inside = false;
        for i in 0..ring.len() {
            let (x0, y0) = xy(mesh.vertices[ring[i]].position);
            let (x1, y1) = xy(mesh.vertices[ring[(i + 1) % ring.len()]].position);
            if (y0 > y) != (y1 > y) && x < (x1 - x0) * (y - y0) / (y1 - y0) + x0 {
                inside = !inside;
            }
        }
        inside
    };
    contains(&mesh.faces[face].vertices) && !mesh.faces[face].holes.iter().any(contains)
}

fn add_polygon(
    mesh: &mut Mesh,
    ring: &[DVec3],
    holes: &[Vec<DVec3>],
    source: &MeshFace,
) -> Result<(), String> {
    let ids: Vec<_> = ring.iter().map(|&p| mesh.add_vertex(p)).collect();
    let holes: Vec<Vec<_>> = holes
        .iter()
        .map(|h| h.iter().map(|&p| mesh.add_vertex(p)).collect())
        .collect();
    if ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != ids.len()
    {
        return Err("Fillet is below document welding tolerance".into());
    }
    let id = mesh
        .add_face(ids, holes)
        .ok_or("Fillet would collapse a face")?;
    mesh.faces[id].attrs = source.attrs.clone();
    mesh.faces[id].layer = source.layer.clone();
    Ok(())
}

/// Round one solid edge with a tangent cylindrical strip. The adjacent planar
/// faces are trimmed to tangent lines and end caps receive the sampled arc.
/// Supports simple trihedral endpoints; multiple-edge chains/corners require a
/// separate joint plan and are deliberately not simulated by edge softening.
pub fn fillet_edge(
    scene: &mut Scene,
    edge_id: usize,
    radius: f64,
    segments: usize,
) -> Result<(), String> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err("Radius must be finite and positive".into());
    }
    if !(1..=256).contains(&segments) {
        return Err("Fillet segments must be between 1 and 256".into());
    }
    let mesh = &scene.mesh;
    let edge = mesh.edges.get(edge_id).ok_or("Invalid fillet edge")?;
    ingetrazo_core::solids::validate_solid_mesh(mesh)?;
    if edge.faces.len() != 2 {
        return Err("Fillet needs exactly two adjacent faces".into());
    }
    let (f1, f2) = (edge.faces[0], edge.faces[1]);
    let p0 = mesh.vertices[edge.v0].position;
    let p1 = mesh.vertices[edge.v1].position;
    let direction = (p1 - p0).normalize_or_zero();
    if direction.length_squared() < 0.5 {
        return Err("Cannot round a zero-length edge".into());
    }
    let w1 = inward(mesh, f1, edge.v0, edge.v1)?;
    let w2 = inward(mesh, f2, edge.v0, edge.v1)?;
    let alpha = w1.dot(w2).clamp(-1., 1.).acos();
    if alpha < 2.0_f64.to_radians() || alpha > 178.0_f64.to_radians() {
        return Err("Faces meet too flat or too sharply to round".into());
    }
    let tangent = radius / (alpha * 0.5).tan();
    let center_offset = (w1 + w2) * (radius / alpha.sin());
    let a = (w1 * tangent - center_offset).normalize();
    let rb = w2 * tangent - center_offset;
    let b = (rb - a * rb.dot(a)).normalize();
    let phi = a.dot(rb.normalize()).clamp(-1., 1.).acos();
    let offsets: Vec<_> = (0..=segments)
        .map(|k| {
            let angle = phi * k as f64 / segments as f64;
            center_offset + radius * (a * angle.cos() + b * angle.sin())
        })
        .collect();
    let sign = if mesh.face_normal(f1).dot(w1 * tangent - center_offset) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut replacements = HashMap::<(usize, usize), Vec<DVec3>>::new();
    let mut sections = Vec::new();
    for vertex in [edge.v0, edge.v1] {
        let caps: Vec<_> = mesh
            .faces
            .iter()
            .enumerate()
            .filter(|(id, f)| {
                *id != f1
                    && *id != f2
                    && f.vertices
                        .iter()
                        .chain(f.holes.iter().flatten())
                        .any(|&v| v == vertex)
            })
            .map(|(id, _)| id)
            .collect();
        if caps.len() != 1 {
            return Err("Fillet requires one end cap at each edge endpoint".into());
        }
        let cap = caps[0];
        let normal = mesh.face_normal(cap);
        let origin = mesh.vertices[mesh.faces[cap].vertices[0]].position;
        let denominator = direction.dot(normal);
        if denominator.abs() < 0.05 {
            return Err("End cap is too oblique to round".into());
        }
        let section: Vec<_> = offsets
            .iter()
            .map(|&offset| {
                let point = mesh.vertices[vertex].position + offset;
                point + direction * (origin - point).dot(normal) / denominator
            })
            .collect();
        for (face, point, into) in [(f1, section[0], w1), (f2, section[segments], w2)] {
            let mid = (p0 + p1) * 0.5;
            let probe = point + ((mid - point) - into * tangent) * 1e-3;
            if !inside_face(mesh, face, probe) {
                return Err("Radius is too large for an adjacent face".into());
            }
            replacements.insert((face, vertex), vec![point]);
        }
        replacements.insert((cap, vertex), section.clone());
        sections.push(section);
    }
    if (sections[1][0] - sections[0][0]).dot(direction) <= 1e-6 {
        return Err("Fillet endpoints cross: radius is too large".into());
    }
    let mut result = Mesh::new();
    for (fid, face) in mesh.faces.iter().enumerate() {
        let rebuild = |ring: &Vec<usize>| -> Result<Vec<DVec3>, String> {
            let mut points = Vec::new();
            for i in 0..ring.len() {
                let vertex = ring[i];
                if vertex == edge.v0 || vertex == edge.v1 {
                    let mut replacement = replacements
                        .get(&(fid, vertex))
                        .ok_or("Unresolved fillet corner")?
                        .clone();
                    if replacement.len() > 1 {
                        let prev = mesh.vertices[ring[(i + ring.len() - 1) % ring.len()]].position;
                        let next = mesh.vertices[ring[(i + 1) % ring.len()]].position;
                        let forward = (prev - replacement[0]).length()
                            + (next - *replacement.last().unwrap()).length();
                        let backward = (prev - *replacement.last().unwrap()).length()
                            + (next - replacement[0]).length();
                        if backward < forward {
                            replacement.reverse();
                        }
                    }
                    points.extend(replacement);
                } else {
                    points.push(mesh.vertices[vertex].position);
                }
            }
            points.dedup_by(|a, b| (*a - *b).length() < 1e-8);
            if points.len() > 1 && (points[0] - *points.last().unwrap()).length() < 1e-8 {
                points.pop();
            }
            if points.len() < 3 {
                return Err("Radius would collapse an adjacent face".into());
            }
            let origin = mesh.vertices[face.vertices[0]].position;
            let normal = mesh.face_normal(fid);
            if points
                .iter()
                .any(|p| (*p - origin).dot(normal).abs() > 1e-7 * (*p - origin).length().max(1.))
            {
                return Err("Fillet would bend a planar face".into());
            }
            Ok(points)
        };
        let ring = rebuild(&face.vertices)?;
        let holes = face
            .holes
            .iter()
            .map(rebuild)
            .collect::<Result<Vec<_>, _>>()?;
        add_polygon(&mut result, &ring, &holes, face)?;
    }
    let center = (sections[0][0] + sections[1][0]) * 0.5 - w1 * tangent + center_offset;
    let mut strip_source = MeshFace::new(vec![]);
    let material_keys = ["color", "texture", "mat", "opacity"];
    if material_keys
        .iter()
        .all(|key| mesh.faces[f1].attrs.get(*key) == mesh.faces[f2].attrs.get(*key))
    {
        for key in material_keys {
            if let Some(value) = mesh.faces[f1].attrs.get(key) {
                strip_source.attrs.insert(key.into(), value.clone());
            }
        }
    }
    if mesh.faces[f1].layer == mesh.faces[f2].layer {
        strip_source.layer = mesh.faces[f1].layer.clone();
    }
    for i in 0..segments {
        let mut quad = vec![
            sections[0][i],
            sections[0][i + 1],
            sections[1][i + 1],
            sections[1][i],
        ];
        let centroid = quad.iter().sum::<DVec3>() / 4.;
        if newell_vector(&quad).dot(centroid - center) * sign < 0.0 {
            quad.reverse();
        }
        add_polygon(&mut result, &quad, &[], &strip_source)?;
    }
    ingetrazo_core::solids::validate_solid_mesh(&result)?;
    let positions: HashMap<_, _> = result
        .vertices
        .iter()
        .enumerate()
        .map(|(id, v)| (PointKey::from_vec3(v.position), id))
        .collect();
    // Copy metadata onto surviving shortened segments of old edges.
    for original in &mesh.edges {
        if original.attrs.is_empty()
            && !original.soft
            && original.curve_id.is_none()
            && original.layer.is_none()
            && !original.hidden
        {
            continue;
        }
        let a = mesh.vertices[original.v0].position;
        let b = mesh.vertices[original.v1].position;
        let d = b - a;
        let length = d.length_squared();
        if length < 1e-20 {
            continue;
        }
        for edge in &mut result.edges {
            let on = |p: DVec3| {
                let t = (p - a).dot(d) / length;
                (-1e-9..=1. + 1e-9).contains(&t) && (p - (a + t * d)).length() < 1e-8
            };
            if on(result.vertices[edge.v0].position) && on(result.vertices[edge.v1].position) {
                edge.attrs = original.attrs.clone();
                edge.soft = original.soft;
                edge.curve_id = original.curve_id;
                edge.layer = original.layer.clone();
                edge.hidden = original.hidden;
            }
        }
    }
    let first_curve = mesh
        .edges
        .iter()
        .filter_map(|e| e.curve_id)
        .max()
        .unwrap_or(0)
        + 1;
    for k in 0..=segments {
        let a = positions[&PointKey::from_vec3(sections[0][k])];
        let b = positions[&PointKey::from_vec3(sections[1][k])];
        if let Some(id) = result.find_edge(a, b) {
            result.edges[id].soft = true;
        }
    }
    for (end, section) in sections.iter().enumerate() {
        for i in 0..segments {
            let a = positions[&PointKey::from_vec3(section[i])];
            let b = positions[&PointKey::from_vec3(section[i + 1])];
            if let Some(id) = result.find_edge(a, b) {
                result.edges[id].curve_id = Some(first_curve + end);
                result.edges[id].soft = false;
            }
        }
    }
    scene.mesh = result;
    scene.clear_selection();
    scene.bump_version();
    Ok(())
}

pub struct FilletTool {
    pub radius: f64,
    pub segments: usize,
    selected_edge: Option<usize>,
    hover_point: Option<DVec3>,
    pub last_error: Option<String>,
}

impl Default for FilletTool {
    fn default() -> Self {
        Self {
            radius: 0.1,
            segments: 8,
            selected_edge: None,
            hover_point: None,
            last_error: None,
        }
    }
}

impl Tool for FilletTool {
    fn error_message(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    fn tool_type(&self) -> ToolType {
        ToolType::Fillet
    }

    fn name(&self) -> &'static str {
        "Fillet"
    }

    fn description(&self) -> &'static str {
        "Round the edges of geometry to the specified radius."
    }

    fn vcb_label(&self) -> &'static str {
        "Radius"
    }

    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.last_error = None;
        if self.selected_edge.is_none() {
            if let Some(edge_id) = pick_edge(&scene.mesh, &ctx.ray, 0.3) {
                self.selected_edge = Some(edge_id);
            }
        } else {
            if let Some(edge_id) = self.selected_edge {
                self.last_error = fillet_edge(scene, edge_id, self.radius, self.segments).err();
            }
            self.selected_edge = None;
            self.hover_point = None;
        }
    }

    fn on_move(&mut self, _scene: &mut Scene, ctx: &ToolContext) {
        self.hover_point = Some(ctx.world);
    }

    fn on_cancel(&mut self, _scene: &mut Scene) {
        self.selected_edge = None;
        self.hover_point = None;
    }

    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let radius: f64 = text
            .trim()
            .parse()
            .map_err(|_| "Enter a valid positive radius")?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err("Enter a finite radius >0".into());
        }
        self.radius = radius;
        if let Some(edge) = self.selected_edge {
            fillet_edge(scene, edge, self.radius, self.segments)?;
            self.selected_edge = None;
        }
        Ok(())
    }

    fn rubber_band(&self) -> Option<(DVec3, DVec3)> {
        self.hover_point
            .map(|h| (h, h + DVec3::new(self.radius, 0.0, 0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_scene() -> Scene {
        let mut scene = Scene::new();
        for p in [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [2., 0., 1.],
            [2., 1., 1.],
            [0., 1., 1.],
        ] {
            scene.mesh.add_vertex(DVec3::from_array(p));
        }
        for ring in [
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ] {
            let id = scene.mesh.add_face(ring, vec![]).unwrap();
            scene.mesh.faces[id]
                .attrs
                .insert("mat".into(), "stone".into());
        }
        scene
    }
    #[test]
    fn one_edge_creates_tangent_strip_caps_curves_and_analytic_volume() {
        let mut scene = box_scene();
        let edge = scene.mesh.find_edge(4, 5).unwrap();
        fillet_edge(&mut scene, edge, 0.2, 8).unwrap();
        assert_eq!(scene.mesh.faces.len(), 14);
        assert!(scene.mesh.is_manifold_solid());
        let removed = (0.2_f64.powi(2) - std::f64::consts::PI * 0.2_f64.powi(2) / 4.) * 2.;
        assert!((scene.mesh.volume().unwrap() - (2. - removed)).abs() < 0.001);
        for face in &scene.mesh.faces[6..] {
            for &vertex in &face.vertices {
                let p = scene.mesh.vertices[vertex].position;
                assert!(((p.y - 0.2).hypot(p.z - 0.8) - 0.2).abs() < 1e-8);
            }
        }
        let top = scene
            .mesh
            .faces
            .iter()
            .find(|f| {
                f.vertices
                    .iter()
                    .all(|&v| (scene.mesh.vertices[v].position.z - 1.).abs() < 1e-8)
            })
            .unwrap();
        assert!(
            (top.vertices
                .iter()
                .map(|&v| scene.mesh.vertices[v].position.y)
                .fold(f64::INFINITY, f64::min)
                - 0.2)
                .abs()
                < 1e-8
        );
        assert_eq!(scene.mesh.edges.iter().filter(|e| e.soft).count(), 9);
        assert_eq!(
            scene
                .mesh
                .edges
                .iter()
                .filter_map(|e| e.curve_id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2
        );
        assert!(scene
            .mesh
            .faces
            .iter()
            .all(|f| f.attrs.get("mat") == Some(&"stone".into())));
    }

    #[test]
    fn concave_edge_adds_material_and_invalid_radius_is_atomic() {
        let mut scene = Scene::new();
        let ids = [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [1., 1., 0.],
            [1., 2., 0.],
            [0., 2., 0.],
        ]
        .into_iter()
        .map(|p| scene.mesh.add_vertex(DVec3::from_array(p)))
        .collect();
        scene.mesh.add_face(ids, vec![]);
        crate::pushpull::PushPullTool::extrude_face(&mut scene, 0, 1.).unwrap();
        let v0 = scene
            .mesh
            .vertices
            .iter()
            .position(|v| v.position == DVec3::new(1., 1., 0.))
            .unwrap();
        let v1 = scene
            .mesh
            .vertices
            .iter()
            .position(|v| v.position == DVec3::new(1., 1., 1.))
            .unwrap();
        let edge = scene.mesh.find_edge(v0, v1).unwrap();
        let before = scene.clone();
        for radius in [f64::NAN, f64::INFINITY, -1., 0., 3.] {
            assert!(fillet_edge(&mut scene, edge, radius, 8).is_err());
            assert_eq!(scene, before);
        }
        fillet_edge(&mut scene, edge, 0.2, 8).unwrap();
        let added = 0.2_f64.powi(2) * (1. - std::f64::consts::PI / 4.);
        assert!((scene.mesh.volume().unwrap() - (3. + added)).abs() < 0.001);
    }
    #[test]
    fn transformed_solid_rounding_uses_world_radius() {
        let mut scene = box_scene();
        let transform = glam::DMat4::from_translation(DVec3::new(3., -5., 7.))
            * glam::DMat4::from_rotation_y(0.37)
            * glam::DMat4::from_scale(DVec3::new(2., 3., 0.5));
        for vertex in &mut scene.mesh.vertices {
            vertex.position = transform.transform_point3(vertex.position);
        }
        scene.mesh.rebuild_spatial_index();
        let edge = scene.mesh.find_edge(4, 5).unwrap();
        fillet_edge(&mut scene, edge, 0.1, 16).unwrap();
        let removed = (0.1_f64.powi(2) - std::f64::consts::PI * 0.1_f64.powi(2) / 4.) * 4.;
        assert!((scene.mesh.volume().unwrap() - (6. - removed)).abs() < 0.0001);
    }
}
