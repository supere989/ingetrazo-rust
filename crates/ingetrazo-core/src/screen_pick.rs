// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Screen-distance hover references shared by drawing tools and snapping.
//! Source `Viewport.pick_edge_any` selects the closest projected segment,
//! preferring loose geometry on ties; `PlaneLock.hovered_reference` prefers
//! that edge's direction over the face normal. No scene copies are needed.

use crate::{
    camera::OrbitCamera,
    groups::Group,
    math::{ray_triangle_intersect, Ray},
    mesh::Mesh,
    scene::Scene,
};
use glam::{DMat4, DVec2, DVec3};

#[derive(Debug, Clone, Copy)]
pub struct PixelViewport {
    pub matrix: DMat4,
    pub width: f64,
    pub height: f64,
}
impl PixelViewport {
    pub fn new(camera: &OrbitCamera, width: f64, height: f64) -> Self {
        Self {
            matrix: camera.projection_matrix() * camera.view_matrix(),
            width,
            height,
        }
    }
    pub fn project(&self, point: DVec3) -> Option<DVec2> {
        if self.width <= 0. || self.height <= 0. || !point.is_finite() {
            return None;
        }
        let clip = self.matrix * point.extend(1.);
        if !clip.is_finite() || clip.w <= 1e-12 {
            return None;
        }
        let p = clip.truncate() / clip.w;
        if p.z < 0. || p.z > 1. {
            return None;
        }
        Some(DVec2::new(
            (p.x + 1.) * self.width * 0.5,
            (1. - p.y) * self.height * 0.5,
        ))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Edge,
    Face,
}
#[derive(Debug, Clone, Copy)]
pub struct HoverReference {
    pub normal: DVec3,
    pub kind: ReferenceKind,
}

pub(crate) fn visit_meshes(scene: &Scene, mut visit: impl FnMut(&Mesh, DMat4, bool)) {
    fn groups(
        scene: &Scene,
        gs: &[Group],
        parent: DMat4,
        visit: &mut impl FnMut(&Mesh, DMat4, bool),
    ) {
        for g in gs {
            if g.hidden || !scene.is_layer_visible(g.layer.as_deref()) {
                continue;
            }
            let world = parent * g.transform;
            visit(&g.mesh, world, true);
            groups(scene, &g.children, world, visit);
        }
    }
    visit(&scene.mesh, DMat4::IDENTITY, false);
    groups(scene, &scene.groups, DMat4::IDENTITY, &mut visit);
}

pub fn pixel_segment_distance(cursor: DVec2, a: DVec2, b: DVec2) -> (f64, f64) {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-12 {
        ((cursor - a).dot(ab) / ab.length_squared()).clamp(0., 1.)
    } else {
        0.
    };
    ((a + ab * t - cursor).length(), t)
}

/// Source hover uses eight logical pixels, independent of zoom or depth.
/// Soft loose edges remain eligible for reference inference; group references
/// come from the original hard-edge placement index.
pub fn hover_edge(
    scene: &Scene,
    viewport: &PixelViewport,
    cursor: DVec2,
    threshold: f64,
) -> Option<(DVec3, DVec3)> {
    let mut best = threshold;
    let mut result = None;
    visit_meshes(scene, |mesh, world, group| {
        for (id, e) in mesh.edges.iter().enumerate() {
            if !scene.mesh_edge_visible(mesh, id) || (group && e.soft) {
                continue;
            }
            let a = world.transform_point3(mesh.vertices[e.v0].position);
            let b = world.transform_point3(mesh.vertices[e.v1].position);
            let Some((a, b)) = scene.section_segment(a, b) else {
                continue;
            };
            let (Some(pa), Some(pb)) = (viewport.project(a), viewport.project(b)) else {
                continue;
            };
            let (d, _) = pixel_segment_distance(cursor, pa, pb);
            if d < best {
                best = d;
                result = Some((a, b));
            }
        }
    });
    result
}

/// Face placements are intersected in world space, including nested affine
/// transforms; deriving the normal from transformed triangle positions also
/// handles nonuniform and mirrored placements correctly.
pub fn hover_face(scene: &Scene, ray: &Ray) -> Option<(DVec3, DVec3, f64)> {
    let mut best = f64::INFINITY;
    let mut result = None;
    visit_meshes(scene, |mesh, world, _| {
        for id in 0..mesh.faces.len() {
            if !scene.mesh_face_visible(mesh, id) {
                continue;
            }
            for (a, b, c) in mesh.triangulate_face(id) {
                let a = world.transform_point3(a);
                let b = world.transform_point3(b);
                let c = world.transform_point3(c);
                if let Some((t, p)) = ray_triangle_intersect(ray, a, b, c) {
                    if t < best && scene.section_point_visible(p) {
                        let normal = (b - a).cross(c - a).normalize_or_zero();
                        if normal.length_squared() > 0. {
                            best = t;
                            result = Some((p, normal, t));
                        }
                    }
                }
            }
        }
    });
    result
}

pub fn hovered_reference(
    scene: &Scene,
    viewport: &PixelViewport,
    cursor: DVec2,
    ray: &Ray,
) -> Option<HoverReference> {
    if let Some((a, b)) = hover_edge(scene, viewport, cursor, 8.) {
        let normal = (b - a).normalize_or_zero();
        if normal.length_squared() > 0. {
            return Some(HoverReference {
                normal,
                kind: ReferenceKind::Edge,
            });
        }
    }
    hover_face(scene, ray).map(|(_, normal, _)| HoverReference {
        normal,
        kind: ReferenceKind::Face,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(distance: f64) -> (OrbitCamera, PixelViewport) {
        let mut c = OrbitCamera::new();
        c.set_standard_view(crate::camera::StandardView::Top);
        c.distance = distance;
        c.perspective = false;
        (c.clone(), PixelViewport::new(&c, 800., 800.))
    }
    #[test]
    fn pixel_threshold_is_zoom_invariant_and_does_not_pick_behind_camera() {
        for d in [2., 200.] {
            let (c, v) = view(d);
            let mut s = Scene::new();
            let a = s.mesh.add_vertex(DVec3::new(-1., 0., 0.));
            let b = s.mesh.add_vertex(DVec3::new(1., 0., 0.));
            s.mesh.add_edge(a, b);
            let p = v.project(DVec3::ZERO).unwrap();
            assert!(hover_edge(&s, &v, p + DVec2::Y * 7.99, 8.).is_some());
            assert!(hover_edge(&s, &v, p + DVec2::Y * 8.01, 8.).is_none());
            assert!(v.project(c.eye() - c.forward()).is_none());
        }
    }
    #[test]
    fn face_reference_respects_holes_and_mirrored_placement() {
        let (c, v) = view(20.);
        let mut mesh = Mesh::new();
        let outer = [
            DVec3::new(-2., -2., 0.),
            DVec3::new(2., -2., 0.),
            DVec3::new(2., 2., 0.),
            DVec3::new(-2., 2., 0.),
        ]
        .map(|p| mesh.add_vertex(p));
        let hole = [
            DVec3::new(-0.5, -0.5, 0.),
            DVec3::new(-0.5, 0.5, 0.),
            DVec3::new(0.5, 0.5, 0.),
            DVec3::new(0.5, -0.5, 0.),
        ]
        .map(|p| mesh.add_vertex(p));
        mesh.add_face(outer.to_vec(), vec![hole.to_vec()]);
        let mut g = Group::with_mesh("mirrored", mesh);
        g.transform = DMat4::from_scale(DVec3::new(-2., 1., 1.));
        let mut scene = Scene::new();
        scene.groups.push(g);
        let cursor = v.project(DVec3::new(2., 1., 0.)).unwrap();
        let ray = c.screen_to_ray(cursor.x, cursor.y, 800., 800.);
        let reference = hovered_reference(&scene, &v, cursor, &ray).unwrap();
        assert_eq!(reference.kind, ReferenceKind::Face);
        assert!((reference.normal + DVec3::Z).length() < 1e-9);
        let cursor = v.project(DVec3::ZERO).unwrap();
        let ray = c.screen_to_ray(cursor.x, cursor.y, 800., 800.);
        assert!(hover_face(&scene, &ray).is_none());
        scene.groups[0].hidden = true;
        assert!(hover_face(&scene, &c.screen_to_ray(400., 300., 800., 800.)).is_none());
    }

    #[test]
    fn nested_group_edges_use_world_placement_and_loose_ties_win() {
        let (_, v) = view(20.);
        let mut s = Scene::new();
        let a = s.mesh.add_vertex(-DVec3::X);
        let b = s.mesh.add_vertex(DVec3::X);
        s.mesh.add_edge(a, b);
        let mut m = Mesh::new();
        let a = m.add_vertex(-DVec3::Y);
        let b = m.add_vertex(DVec3::Y);
        m.add_edge(a, b);
        let mut child = Group::with_mesh("child", m);
        child.transform = DMat4::from_translation(DVec3::X);
        let mut parent = Group::new("parent");
        parent.transform = DMat4::from_translation(-DVec3::X);
        parent.children.push(child);
        s.groups.push(parent);
        let p = v.project(DVec3::ZERO).unwrap();
        let (a, b) = hover_edge(&s, &v, p, 8.).unwrap();
        assert_eq!((b - a).normalize(), DVec3::X);
        s.mesh.edges[0].hidden = true;
        let (a, b) = hover_edge(&s, &v, p, 8.).unwrap();
        assert_eq!((b - a).normalize(), DVec3::Y);
        s.groups[0].children[0].mesh.edges[0].soft = true;
        assert!(hover_edge(&s, &v, p, 8.).is_none());
    }
}
