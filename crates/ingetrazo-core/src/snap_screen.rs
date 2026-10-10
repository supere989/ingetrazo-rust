// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Camera-aware snapping. Discrete snaps use source logical-pixel thresholds
//! rather than a world radius; edge hits use the camera ray, not screen lerp.
use crate::{
    math::Ray,
    scene::Scene,
    screen_pick::{hover_face, pixel_segment_distance, visit_meshes, PixelViewport},
    snap::*,
};
use glam::{DVec2, DVec3};

#[derive(Debug, Clone, Copy)]
pub struct ScreenSnapContext {
    pub viewport: PixelViewport,
    pub cursor: DVec2,
    pub candidate: DVec3,
    pub plane_normal: DVec3,
}
#[derive(Clone, Copy)]
struct Edge {
    a: DVec3,
    b: DVec3,
    distance: f64,
    group: bool,
    guide: bool,
}
fn result(point: DVec3, kind: SnapKind, color: [f32; 3], label: &str) -> SnapResult {
    SnapResult {
        point,
        kind,
        color,
        label: label.into(),
        axis: None,
        guides: vec![],
    }
}
fn visible(scene: &Scene, context: &ScreenSnapContext, p: DVec3) -> bool {
    if !scene.section_point_visible(p) {
        return false;
    }
    let Some(pixel) = context.viewport.project(p) else {
        return false;
    };
    let Some(ray) = context.viewport.ray(pixel) else {
        return false;
    };
    let depth = (p - ray.origin).dot(ray.dir);
    if let Some((_, _, front)) = hover_face(scene, &ray) {
        front >= depth - 1e-6 * depth.abs().max(1.)
    } else {
        true
    }
}
fn project_line(ray: &Ray, origin: DVec3, dir: DVec3) -> DVec3 {
    let w = ray.origin - origin;
    let a = ray.dir.length_squared();
    let b = ray.dir.dot(dir);
    let c = dir.length_squared();
    let denom = a * c - b * b;
    let t = if denom.abs() > 1e-12 {
        (a * dir.dot(w) - b * ray.dir.dot(w)) / denom
    } else {
        0.
    };
    origin + dir * t
}
fn crossing(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> Option<DVec3> {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let uu = u.length_squared();
    let vv = v.length_squared();
    let uv = u.dot(v);
    let det = uu * vv - uv * uv;
    if uu < 1e-12 || vv < 1e-12 || det.abs() < 1e-12 * uu * vv {
        return None;
    }
    let t = (uv * v.dot(w) - vv * u.dot(w)) / det;
    let s = (uu * v.dot(w) - uv * u.dot(w)) / det;
    if !(-1e-7..=1. + 1e-7).contains(&t) || !(-1e-7..=1. + 1e-7).contains(&s) {
        return None;
    }
    let p = a + u * t;
    let q = c + v * s;
    if p.distance(q) > 1e-4 {
        return None;
    }
    Some((p + q) * 0.5)
}
fn edges(scene: &Scene, ctx: &ScreenSnapContext) -> Vec<Edge> {
    let mut loose = vec![];
    let mut groups = vec![];
    let mut append = |a: DVec3, b: DVec3, group: bool, guide: bool| {
        if scene.section_segment(a, b).is_none() {
            return;
        }
        let (Some(pa), Some(pb)) = (ctx.viewport.project(a), ctx.viewport.project(b)) else {
            return;
        };
        let (distance, _) = pixel_segment_distance(ctx.cursor, pa, pb);
        if distance >= if group { 48. } else { 64. } {
            return;
        }
        let edge = Edge {
            a,
            b,
            distance,
            group,
            guide,
        };
        if group {
            groups.push(edge)
        } else {
            loose.push(edge)
        }
    };
    visit_meshes(scene, |m, world, group| {
        for (id, e) in m.edges.iter().enumerate() {
            if scene.mesh_edge_visible(m, id) && (!group || !e.soft) {
                append(
                    world.transform_point3(m.vertices[e.v0].position),
                    world.transform_point3(m.vertices[e.v1].position),
                    group,
                    false,
                );
            }
        }
    });
    for g in &scene.guides {
        use crate::guides::GuideType;
        match g.kind {
            GuideType::Point(p) => append(p, p, false, true),
            GuideType::Segment { a, b } => append(a, b, false, true),
            GuideType::InfiniteLine { point, dir } => {
                append(point - dir * 10000., point + dir * 10000., false, true)
            }
        }
    }
    if scene.mesh.edges.len() > 3000 && loose.len() > 400 {
        loose.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        loose.truncate(400);
    }
    if groups.len() > 48 {
        groups.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        groups.truncate(48);
    }
    loose.extend(groups);
    loose
}

fn from_point(
    scene: &Scene,
    ctx: &ScreenSnapContext,
    edges: &[Edge],
    extra: Option<DVec3>,
    start: DVec3,
    dir: DVec3,
    hovered: bool,
) -> Option<SnapResult> {
    let pixels = |p: DVec3| {
        ctx.viewport
            .project(p)
            .map(|q| q.distance(ctx.cursor))
            .unwrap_or(f64::INFINITY)
    };
    let mut refs = vec![];
    if let Some(p) = extra {
        refs.push((p, SnapKind::FromPoint, COLOR_ENDPOINT));
    }
    for e in edges {
        refs.push((e.a, SnapKind::FromPoint, COLOR_ENDPOINT));
        refs.push((e.b, SnapKind::FromPoint, COLOR_ENDPOINT));
        refs.push(((e.a + e.b) * 0.5, SnapKind::Midpoint, COLOR_MIDPOINT));
    }
    refs.push((DVec3::ZERO, SnapKind::Origin, COLOR_ORIGIN));
    let mut best = None;
    let mut distance = 9.;
    for (reference, kind, color) in refs {
        let reach = (reference - start).dot(dir);
        if !hovered && reach <= 1e-6 {
            continue;
        }
        let foot = start + dir * reach;
        if foot.distance(reference) < 1e-4 {
            continue;
        }
        let d = if hovered {
            pixels(foot).min(pixels(reference))
        } else {
            pixels(foot)
        };
        if d <= distance && visible(scene, ctx, foot) {
            if best.is_none() || d < distance {
                distance = d;
                let mut r = result(foot, kind, color, "From point");
                r.guides.push((reference, foot, color));
                best = Some(r);
            }
        }
    }
    best
}
fn axis_direction(draw: DVec3, degrees: f64) -> Option<DVec3> {
    if draw.length() < 1e-6 {
        return None;
    }
    let draw = draw.normalize();
    [DVec3::X, DVec3::Y, DVec3::Z]
        .into_iter()
        .find(|d| draw.dot(*d).abs() >= degrees.to_radians().cos())
        .map(|d| d * draw.dot(d).signum())
}
fn memory_snap(
    engine: &SnapEngine,
    scene: &Scene,
    ctx: &ScreenSnapContext,
    on_face: bool,
) -> Option<SnapResult> {
    let pixels = |p: DVec3| {
        ctx.viewport
            .project(p)
            .map(|q| q.distance(ctx.cursor))
            .unwrap_or(f64::INFINITY)
    };
    if engine.encouraged_points.len() >= 2 {
        let a = engine.encouraged_points[engine.encouraged_points.len() - 1];
        let b = engine.encouraged_points[engine.encouraged_points.len() - 2];
        let mut best = None;
        let mut distance = 9.;
        for x in [DVec3::X, DVec3::Y, DVec3::Z] {
            for y in [DVec3::X, DVec3::Y, DVec3::Z] {
                if x == y {
                    continue;
                }
                if let Some(p) = crossing(
                    a - x * 10000.,
                    a + x * 10000.,
                    b - y * 10000.,
                    b + y * 10000.,
                ) {
                    let d = pixels(p);
                    if d <= distance && visible(scene, ctx, p) {
                        distance = d;
                        let mut r = result(
                            p,
                            SnapKind::TwoPointFromPoint,
                            COLOR_ENDPOINT,
                            "From points",
                        );
                        r.guides
                            .extend([(a, p, COLOR_ENDPOINT), (b, p, COLOR_ENDPOINT)]);
                        best = Some(r);
                    }
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    let reference = engine.encouraged_points.last().copied()?;
    let mut best = None;
    let mut distance = 9.;
    let n = if on_face {
        ctx.plane_normal.normalize_or_zero()
    } else {
        DVec3::ZERO
    };
    for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
        let an = axis.dot(n);
        let reach = if an.abs() < 1e-6 {
            if (reference - ctx.candidate).dot(n).abs() > 1e-4 {
                continue;
            }
            (ctx.candidate - reference).dot(axis)
        } else {
            (ctx.candidate - reference).dot(n) / an
        };
        if reach.abs() < 1e-3 {
            continue;
        }
        let foot = reference + axis * reach;
        let d = pixels(foot);
        if d <= distance && visible(scene, ctx, foot) {
            distance = d;
            let mut r = result(foot, SnapKind::FromPoint, COLOR_ENDPOINT, "From point");
            r.guides.push((reference, foot, COLOR_ENDPOINT));
            best = Some(r);
        }
    }
    best
}

impl SnapEngine {
    /// Viewport fast path. World-only `snap` remains available to headless callers.
    pub fn snap_screen(
        &mut self,
        ray: &Ray,
        scene: &Scene,
        start: Option<DVec3>,
        close: Option<DVec3>,
        ctx: &ScreenSnapContext,
    ) -> SnapResult {
        let edges = edges(scene, ctx);
        let pixels = |p: DVec3| {
            ctx.viewport
                .project(p)
                .map(|q| q.distance(ctx.cursor))
                .unwrap_or(f64::INFINITY)
        };
        let line = self
            .locked_line
            .map(|(a, b)| (a, (b - a).normalize_or_zero(), None))
            .or_else(|| {
                self.locked_axis.zip(start).map(|(axis, p)| {
                    let d = match axis {
                        'x' | 'X' => DVec3::X,
                        'y' | 'Y' => DVec3::Y,
                        _ => DVec3::Z,
                    };
                    (p, d, Some(axis.to_ascii_lowercase()))
                })
            });
        let on_line = |p: DVec3| line.is_none_or(|(a, d, _)| (p - a).cross(d).length() < 1e-5);
        if let Some(p) =
            close.filter(|p| on_line(*p) && pixels(*p) <= 9. && scene.section_point_visible(*p))
        {
            return result(p, SnapKind::Close, COLOR_CLOSE, "Close Loop");
        }
        // Stable half-pixel buckets preserve source loose-context tie precedence.
        let mut points = vec![(pixels(DVec3::ZERO), DVec3::ZERO, SnapKind::Origin, false)];
        for e in &edges {
            if !e.guide {
                for p in [e.a, e.b] {
                    points.push((pixels(p), p, SnapKind::Endpoint, e.group));
                }
            }
        }
        for p in &scene.geo_points {
            points.push((pixels(p.position), p.position, SnapKind::Endpoint, false));
        }
        points.retain(|(d, p, _, _)| *d <= 9. && on_line(*p));
        points.sort_by(|a, b| {
            (a.0 / 0.5)
                .floor()
                .total_cmp(&(b.0 / 0.5).floor())
                .then(a.3.cmp(&b.3))
        });
        for (_, p, kind, _) in points {
            if visible(scene, ctx, p) {
                if kind == SnapKind::Endpoint {
                    self.encourage_point(p);
                }
                return result(
                    p,
                    kind,
                    if kind == SnapKind::Origin {
                        COLOR_ORIGIN
                    } else {
                        COLOR_ENDPOINT
                    },
                    if kind == SnapKind::Origin {
                        "Origin"
                    } else {
                        "Endpoint"
                    },
                );
            }
        }
        if let Some((origin, dir, axis)) = line {
            let mut hits = vec![];
            for e in &edges {
                let reach = (e.a - origin).length().max((e.b - origin).length()).max(1.) * 2.;
                if let Some(p) = crossing(origin - dir * reach, origin + dir * reach, e.a, e.b) {
                    let d = pixels(p);
                    if d <= 9. {
                        hits.push((d, p));
                    }
                }
            }
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, p) in hits {
                if visible(scene, ctx, p) {
                    return result(p, SnapKind::Intersection, COLOR_ENDPOINT, "Intersection");
                }
            }
            if let Some(r) = from_point(
                scene,
                ctx,
                &edges,
                self.encouraged_points.last().copied(),
                origin,
                dir,
                true,
            ) {
                return r;
            }
            let p = project_line(ray, origin, dir);
            let color = match axis {
                Some('x') => COLOR_AXIS_X,
                Some('y') => COLOR_AXIS_Y,
                Some('z') => COLOR_AXIS_Z,
                _ => COLOR_REFERENCE,
            };
            let mut r = result(p, SnapKind::AxisLock, color, "Locked Line");
            r.axis = axis;
            r.guides
                .push((origin - dir * 100., origin + dir * 100., color));
            return r;
        }
        if let Some(start) = start {
            if let Some(dir) = axis_direction(ctx.candidate - start, 10.) {
                if let Some(r) = from_point(
                    scene,
                    ctx,
                    &edges,
                    self.encouraged_points.last().copied(),
                    start,
                    dir,
                    false,
                ) {
                    return r;
                }
            }
        }
        let near: Vec<_> = edges.iter().filter(|e| e.distance <= 14.).collect();
        let mut hits = vec![];
        for (i, a) in near.iter().enumerate() {
            for b in &near[i + 1..] {
                if let Some(p) = crossing(a.a, a.b, b.a, b.b) {
                    let d = pixels(p);
                    if d <= 14. {
                        hits.push((d, p));
                    }
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, p) in hits {
            if visible(scene, ctx, p) {
                return result(p, SnapKind::Intersection, COLOR_ENDPOINT, "Intersection");
            }
        }
        let mut midpoints = edges
            .iter()
            .filter(|e| !e.guide && e.a.distance(e.b) > 1e-9)
            .map(|e| (pixels((e.a + e.b) * 0.5), (e.a + e.b) * 0.5, e.group))
            .filter(|(d, _, _)| *d <= 9.)
            .collect::<Vec<_>>();
        midpoints.sort_by(|a, b| {
            (a.0 / 0.5)
                .floor()
                .total_cmp(&(b.0 / 0.5).floor())
                .then(a.2.cmp(&b.2))
        });
        for (_, p, _) in midpoints {
            if visible(scene, ctx, p) {
                self.encourage_point(p);
                return result(p, SnapKind::Midpoint, COLOR_MIDPOINT, "Midpoint");
            }
        }
        let mut on_edges = near;
        on_edges.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        for e in on_edges {
            let ab = e.b - e.a;
            let p = if ab.length_squared() > 1e-18 {
                let q = project_line(ray, e.a, ab);
                let t = ((q - e.a).dot(ab) / ab.length_squared()).clamp(0., 1.);
                e.a + ab * t
            } else {
                e.a
            };
            if visible(scene, ctx, p) {
                return result(
                    p,
                    if e.guide {
                        SnapKind::Guide
                    } else {
                        SnapKind::OnEdge
                    },
                    COLOR_ON_EDGE,
                    if e.guide { "On Guide Line" } else { "On Edge" },
                );
            }
        }
        if let (Some(start), Some((a, b))) = (start, self.reference_edge) {
            let draw = (ctx.candidate - start).normalize_or_zero();
            let parallel = (b - a).normalize_or_zero();
            let perpendicular = ctx.plane_normal.cross(parallel).normalize_or_zero();
            for (direction, label) in [
                (parallel, "Parallel to edge"),
                (perpendicular, "Perpendicular to edge"),
            ] {
                if direction.length() > 1e-9
                    && draw.dot(direction).abs() >= 3_f64.to_radians().cos()
                {
                    let point = project_line(ray, start, direction);
                    let mut r = result(point, SnapKind::Reference, COLOR_REFERENCE, label);
                    r.guides.push((start, point, COLOR_REFERENCE));
                    return r;
                }
            }
        }
        if let Some(r) = memory_snap(self, scene, ctx, hover_face(scene, ray).is_some()) {
            return r;
        }
        // Weak angular cues preserve the raw candidate, as source rule9 does.
        if let Some(start) = start {
            let d = ctx.candidate - start;
            if d.length() > 1e-6 {
                let n = d.normalize();
                for (axis, dir, color) in [
                    ('x', DVec3::X, COLOR_AXIS_X),
                    ('y', DVec3::Y, COLOR_AXIS_Y),
                    ('z', DVec3::Z, COLOR_AXIS_Z),
                ] {
                    if n.dot(dir).abs() >= 3_f64.to_radians().cos() {
                        let mut r = result(
                            ctx.candidate,
                            SnapKind::AxisInference,
                            color,
                            "Axis inference",
                        );
                        r.axis = Some(axis);
                        r.guides.push((start, start + dir * d.dot(dir), color));
                        return r;
                    }
                }
            }
        }
        result(ctx.candidate, SnapKind::None, [0.; 3], "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(
        camera: &crate::camera::OrbitCamera,
        candidate: DVec3,
        offset: DVec2,
    ) -> (Ray, ScreenSnapContext) {
        let viewport = PixelViewport::new(camera, 800., 800.);
        let cursor = viewport.project(candidate).unwrap() + offset;
        let ray = viewport.ray(cursor).unwrap();
        (
            ray,
            ScreenSnapContext {
                viewport,
                cursor,
                candidate,
                plane_normal: DVec3::Z,
            },
        )
    }
    fn camera(distance: f64) -> crate::camera::OrbitCamera {
        let mut c = crate::camera::OrbitCamera::new();
        c.set_standard_view(crate::camera::StandardView::Top);
        c.distance = distance;
        c.perspective = false;
        c
    }
    #[test]
    fn endpoint_threshold_uses_pixels_at_near_and_far_zooms() {
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(2., 2., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(4., 2., 0.));
        scene.mesh.add_edge(a, b);
        for distance in [2., 200.] {
            let c = camera(distance);
            let (ray, ctx) = context(&c, DVec3::new(2., 2., 0.), DVec2::Y * 8.99);
            let snap = SnapEngine::default().snap_screen(&ray, &scene, None, None, &ctx);
            assert_eq!(snap.kind, SnapKind::Endpoint);
            assert_eq!(snap.point, DVec3::new(2., 2., 0.));
            let (ray, ctx) = context(&c, DVec3::new(2., 2., 0.), DVec2::Y * 9.01);
            let snap = SnapEngine::default().snap_screen(&ray, &scene, None, None, &ctx);
            assert_ne!(snap.kind, SnapKind::Endpoint);
        }
    }
    #[test]
    fn priority_close_origin_midpoint_and_skew_crossings() {
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(-DVec3::X);
        let b = scene.mesh.add_vertex(DVec3::X);
        scene.mesh.add_edge(a, b);
        let c = camera(5.);
        let (ray, ctx) = context(&c, DVec3::ZERO, DVec2::ZERO);
        assert_eq!(
            SnapEngine::default()
                .snap_screen(&ray, &scene, None, None, &ctx)
                .kind,
            SnapKind::Origin
        );
        assert_eq!(
            SnapEngine::default()
                .snap_screen(&ray, &scene, Some(DVec3::X), Some(DVec3::ZERO), &ctx)
                .kind,
            SnapKind::Close
        );
        scene.mesh.vertices[a].position += DVec3::Y * 2.;
        scene.mesh.vertices[b].position += DVec3::Y * 2.;
        let (ray, ctx) = context(&c, DVec3::Y * 2., DVec2::ZERO);
        let mut engine = SnapEngine::default();
        engine.encourage_point(-DVec3::X);
        assert_eq!(
            engine.snap_screen(&ray, &scene, None, None, &ctx).kind,
            SnapKind::Midpoint
        );
        assert!(crossing(
            -DVec3::X,
            DVec3::X,
            -DVec3::Y + DVec3::Z * 0.01,
            DVec3::Y + DVec3::Z * 0.01
        )
        .is_none());
    }
    #[test]
    fn perspective_on_edge_is_camera_ray_projection_not_pixel_lerp() {
        let mut c = crate::camera::OrbitCamera::new();
        c.target = DVec3::new(2., 2., 1.);
        c.distance = 10.;
        let a = DVec3::new(-2., 2., 0.);
        let b = DVec3::new(6., 2., 3.);
        let expected = a + (b - a) * 0.23;
        let mut scene = Scene::new();
        let aa = scene.mesh.add_vertex(a);
        let bb = scene.mesh.add_vertex(b);
        scene.mesh.add_edge(aa, bb);
        let (ray, ctx) = context(&c, expected, DVec2::ZERO);
        let result = SnapEngine::default().snap_screen(&ray, &scene, None, None, &ctx);
        assert_eq!(result.kind, SnapKind::OnEdge);
        assert!(result.point.distance(expected) < 1e-8);
        let (pa, pb) = (
            ctx.viewport.project(a).unwrap(),
            ctx.viewport.project(b).unwrap(),
        );
        let (_, t) = pixel_segment_distance(ctx.cursor, pa, pb);
        assert!((a + (b - a) * t).distance(expected) > 0.1);
    }
    #[test]
    fn occluded_endpoint_does_not_win_and_axis_cue_does_not_move_candidate() {
        let c = camera(10.);
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(1., 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(2., 0., 0.));
        scene.mesh.add_edge(a, b);
        let ids = [
            DVec3::new(0.5, -0.5, 1.),
            DVec3::new(2.5, -0.5, 1.),
            DVec3::new(2.5, 0.5, 1.),
            DVec3::new(0.5, 0.5, 1.),
        ]
        .map(|p| scene.mesh.add_vertex(p));
        scene.mesh.add_face(ids.to_vec(), vec![]);
        let (ray, ctx) = context(&c, DVec3::new(1., 0., 0.), DVec2::ZERO);
        assert_ne!(
            SnapEngine::default()
                .snap_screen(&ray, &scene, None, None, &ctx)
                .kind,
            SnapKind::Endpoint
        );
        let candidate = DVec3::new(10., 0.2, 0.);
        let (ray, ctx) = context(&c, candidate, DVec2::ZERO);
        let r =
            SnapEngine::default().snap_screen(&ray, &Scene::new(), Some(DVec3::ZERO), None, &ctx);
        assert_eq!(r.kind, SnapKind::AxisInference);
        assert_eq!(r.point, candidate);
    }
    #[test]
    fn memory_lines_yield_to_midpoint_and_preserve_wall_plane() {
        let c = camera(10.);
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(2., 2., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(6., 2., 0.));
        scene.mesh.add_edge(a, b);
        let mut engine = SnapEngine::default();
        engine.encourage_point(DVec3::new(4., 4., 0.));
        let (ray, ctx) = context(&c, DVec3::new(4., 2., 0.), DVec2::ZERO);
        assert_eq!(
            engine.snap_screen(&ray, &scene, None, None, &ctx).kind,
            SnapKind::Midpoint
        );
        let (ray, ctx) = context(&c, DVec3::new(4., 6., 0.), DVec2::X * 2.);
        let r = engine.snap_screen(&ray, &Scene::new(), None, None, &ctx);
        assert_eq!(r.kind, SnapKind::FromPoint);
        assert_eq!(r.point, DVec3::new(4., 6., 0.));
    }
    #[test]
    fn lock_fetches_off_line_reference_height_without_breaking_axis() {
        let c = camera(10.);
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(3., 2., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(4., 2., 0.));
        scene.mesh.add_edge(a, b);
        let mut engine = SnapEngine::default();
        engine.locked_axis = Some('x');
        let (ray, ctx) = context(&c, DVec3::new(3., 2., 0.), DVec2::ZERO);
        let r = engine.snap_screen(&ray, &scene, Some(DVec3::ZERO), None, &ctx);
        assert_eq!(r.kind, SnapKind::FromPoint);
        assert_eq!(r.point, DVec3::new(3., 0., 0.));
    }

    #[test]
    fn locked_axis_still_snaps_endpoints_and_closure_in_both_directions() {
        let c = camera(10.);
        let mut scene = Scene::new();
        let a = scene.mesh.add_vertex(DVec3::new(-2., 0., 0.));
        let b = scene.mesh.add_vertex(DVec3::new(2., 0., 0.));
        scene.mesh.add_edge(a, b);
        let mut engine = SnapEngine::default();
        engine.locked_axis = Some('x');
        for x in [-2., 2.] {
            let (ray, ctx) = context(&c, DVec3::X * x, DVec2::Y * 3.);
            let r = engine.snap_screen(&ray, &scene, Some(DVec3::ZERO), None, &ctx);
            assert_eq!(r.kind, SnapKind::Endpoint);
            assert_eq!(r.point.x, x);
        }
        let (ray, ctx) = context(&c, DVec3::new(-3., 0., 0.), DVec2::Y * 30.);
        let r = engine.snap_screen(&ray, &scene, Some(DVec3::ZERO), None, &ctx);
        assert_eq!(r.kind, SnapKind::AxisLock);
        assert!((r.point.x + 3.).abs() < 1e-8);
    }
}
