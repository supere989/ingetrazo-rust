// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Camera-aware snapping. Discrete snaps use source logical-pixel thresholds
//! rather than a world radius; edge hits use the camera ray, not screen lerp.
use crate::{
    math::Ray,
    scene::Scene,
    screen_pick::{hover_face, pixel_segment_distance, visit_placements, PixelViewport},
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
#[derive(Debug,Default)]
pub struct ScreenSnapPolicy<'a>{
    pub exclusions:Option<&'a SnapExclusions>,
    pub magnetic_axis_deg:Option<f64>,
    pub screen_axis_px:Option<f64>,
    /// Source compute_snap defaults this false; only a viewport caller with
    /// an actual face hit opts into the OnFace fallback.
    pub face_under_cursor:bool,
}
/// Geometry currently moved by a transform is absent from source inference.
/// Group identities exclude their entire nested placement subtree.
#[derive(Debug,Default,Clone)]
pub struct SnapExclusions {
    pub loose_edges: std::collections::HashSet<usize>,
    pub groups: std::collections::HashSet<uuid::Uuid>,
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
    let Some(pixel) = context.viewport.project_for_snapping(p) else {
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
fn edges(scene: &Scene, ctx: &ScreenSnapContext, exclusions: Option<&SnapExclusions>) -> Vec<Edge> {
    let mut loose = vec![];
    let mut groups = vec![];
    let mut append = |a: DVec3, b: DVec3, group: bool, guide: bool| {
        if scene.section_segment(a, b).is_none() {
            return;
        }
        let distance = match (ctx.viewport.project_for_snapping(a),ctx.viewport.project_for_snapping(b)) {
            (Some(pa),Some(pb)) => pixel_segment_distance(ctx.cursor,pa,pb).0,
            // Source small loose scenes consider each endpoint independently.
            // A segment crossing the eye must retain its front endpoint without
            // offering an invalid projected segment for line/intersection snaps.
            _ => f64::INFINITY,
        };
        if (group && distance >= 48.)
            || (!group && !guide && scene.mesh.edges.len() > 3000 && distance >= 64.)
        {
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
    visit_placements(scene, |m, world, path| {
        let group=!path.is_empty();
        if exclusions.is_some_and(|exclude|path.iter().any(|id|exclude.groups.contains(id))){return;}
        for (id, e) in m.edges.iter().enumerate() {
            if !group && exclusions.is_some_and(|exclude|exclude.loose_edges.contains(&id)){continue;}
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
            .project_for_snapping(p)
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
fn axis_direction(draw: DVec3, degrees: f64, axes: crate::drawing_axes::DrawingAxes) -> Option<DVec3> {
    if draw.length() < 1e-6 {
        return None;
    }
    let draw = draw.normalize();
    [axes.x, axes.y, axes.z]
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
            .project_for_snapping(p)
            .map(|q| q.distance(ctx.cursor))
            .unwrap_or(f64::INFINITY)
    };
    if engine.encouraged_points.len() >= 2 {
        let a = engine.encouraged_points[engine.encouraged_points.len() - 1];
        let b = engine.encouraged_points[engine.encouraged_points.len() - 2];
        let mut best = None;
        let mut distance = 9.;
        for x in [engine.drawing_axes.x,engine.drawing_axes.y,engine.drawing_axes.z] {
            for y in [engine.drawing_axes.x,engine.drawing_axes.y,engine.drawing_axes.z] {
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
    for axis in [engine.drawing_axes.x,engine.drawing_axes.y,engine.drawing_axes.z] {
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

#[derive(Debug, Clone)]
enum CenterKey {
    Curve {
        curve: usize,
        ends: [uuid::Uuid; 2],
    },
    Face {
        outer: Vec<uuid::Uuid>,
        run: (usize, usize),
    },
}
#[derive(Debug, Clone)]
pub(crate) struct CenterReference {
    path: Vec<uuid::Uuid>,
    key: CenterKey,
    point: DVec3,
    radius: f64,
    version: u64,
}
fn curve_center(
    mesh: &crate::mesh::Mesh,
    curve: usize,
    world: glam::DMat4,
) -> Option<(DVec3, f64)> {
    let mut ids = vec![];
    let mut seen = std::collections::HashSet::new();
    for e in &mesh.edges {
        if e.curve_id == Some(curve) {
            for id in [e.v0, e.v1] {
                if seen.insert(id) {
                    ids.push(id);
                }
            }
        }
    }
    let points = ids
        .iter()
        .map(|&id| mesh.vertices[id].position)
        .collect::<Vec<_>>();
    let (center, radius) = crate::circle_inference::fit_circle(&points)?;
    Some((world.transform_point3(center), radius))
}
impl SnapEngine {
    fn update_center(&mut self, scene: &Scene, ray: &Ray, ctx: &ScreenSnapContext) {
        use crate::screen_pick::{hover_edge_placement_for_snapping, hover_face_placement, visit_placements};
        self.hovered_center = None;
        if self
            .center_reference
            .as_ref()
            .is_some_and(|r| self.center_cache_dirty || r.version != scene.version)
        {
            let reference = self.center_reference.take().unwrap();
            let mut fresh = None;
            visit_placements(scene, |mesh, world, path| {
                if path == reference.path {
                    let value = match &reference.key {
                        CenterKey::Curve { curve, ends } => mesh
                            .edges
                            .iter()
                            .find(|e| {
                                e.curve_id == Some(*curve)
                                    && [mesh.vertices[e.v0].uid, mesh.vertices[e.v1].uid] == *ends
                            })
                            .and_then(|_| curve_center(mesh, *curve, world)),
                        CenterKey::Face { outer, run } => mesh
                            .faces
                            .iter()
                            .position(|f| {
                                f.vertices
                                    .iter()
                                    .map(|&v| mesh.vertices[v].uid)
                                    .eq(outer.iter().copied())
                            })
                            .and_then(|id| {
                                crate::circle_inference::face_centers(mesh, id, world)
                                    .into_iter()
                                    .find(|(_, _, key)| key == run)
                                    .map(|(c, r, _)| (c, r))
                            }),
                    };
                    if let Some((point, radius)) = value {
                        fresh = Some(CenterReference {
                            point,
                            radius,
                            version: scene.version,
                            ..reference.clone()
                        });
                    }
                }
            });
            self.center_reference = fresh;
        }
        self.center_cache_dirty = false;
        let mut found = None;
        if let Some(hit) = hover_edge_placement_for_snapping(scene, &ctx.viewport, ctx.cursor, 8.) {
            let edge = &hit.mesh.edges[hit.edge];
            if let Some(curve) = edge.curve_id {
                if let Some((point, radius)) = curve_center(hit.mesh, curve, hit.world) {
                    found = Some(CenterReference {
                        path: hit.group_path,
                        key: CenterKey::Curve {
                            curve,
                            ends: [
                                hit.mesh.vertices[edge.v0].uid,
                                hit.mesh.vertices[edge.v1].uid,
                            ],
                        },
                        point,
                        radius,
                        version: scene.version,
                    });
                }
            }
        }
        if found.is_none() {
            if let Some(hit) = hover_face_placement(scene, ray) {
                let mut centers =
                    crate::circle_inference::face_centers(hit.mesh, hit.face, hit.world);
                centers.sort_by(|(a, _, _), (b, _, _)| {
                    let d = |p| {
                        ctx.viewport
                            .project_for_snapping(p)
                            .map(|p| p.distance(ctx.cursor))
                            .unwrap_or(f64::INFINITY)
                    };
                    d(*a).total_cmp(&d(*b))
                });
                if let Some((point, radius, run)) = centers.first().copied() {
                    found = Some(CenterReference {
                        path: hit.group_path,
                        key: CenterKey::Face {
                            outer: hit.mesh.faces[hit.face]
                                .vertices
                                .iter()
                                .map(|&id| hit.mesh.vertices[id].uid)
                                .collect(),
                            run,
                        },
                        point,
                        radius,
                        version: scene.version,
                    });
                }
            }
        }
        if let Some(reference) = found {
            self.hovered_center = Some(reference.point);
            self.center_reference = Some(reference);
        }
    }
    /// Original viewport center approach mark: an acquired curve/face reference
    /// becomes visible inside min(40px, half its projected radius), but a
    /// projected radius below 12px stays quiet. This is observational only;
    /// snap_screen refreshes the reference from the current model beforehand.
    pub fn center_hint_pixel(&self, viewport: &crate::screen_pick::PixelViewport, cursor: DVec2) -> Option<DVec2> {
        let reference=self.center_reference.as_ref()?;
        let pixel=viewport.project_for_snapping(reference.point)?;
        let radius_px=[DVec3::X,DVec3::Y].into_iter()
            .filter_map(|axis|viewport.project_for_snapping(reference.point+axis*reference.radius))
            .map(|p|p.distance(pixel)).fold(0_f64,f64::max);
        (radius_px>=12. && cursor.distance(pixel)<=40_f64.min(radius_px*0.5)).then_some(pixel)
    }
    /// Viewport supplies its monotonic frame time. A 250ms dwell acquires two
    /// source tracking points; drawing over a vertex acquires it instantly.
    pub fn note_screen_result(&mut self, result: &SnapResult, drawing: bool, now: f64) {
        if !now.is_finite() {
            return;
        }
        if drawing && result.kind == SnapKind::Endpoint {
            self.acquired_point = Some(result.point);
        }
        let definite = matches!(
            result.kind,
            SnapKind::Endpoint
                | SnapKind::Midpoint
                | SnapKind::Center
                | SnapKind::Origin
                | SnapKind::Intersection
                | SnapKind::Close
        );
        // Original viewport encourages the centre of a hovered loose curve
        // even when its rim vertex is a definite endpoint snap. Face arcs
        // retain the real corner's precedence.
        let curve_center = self.hovered_center.filter(|_| self.center_reference.as_ref()
            .is_some_and(|r| matches!(r.key,CenterKey::Curve {..})));
        let point = curve_center.or_else(|| if definite {
            Some(result.point)
        } else {
            self.hovered_center
        });
        let Some(point) = point else {
            self.screen_dwell = None;
            return;
        };
        if let Some((old, since)) = self.screen_dwell {
            if old.distance(point) < 1e-6 && now >= since {
                if now - since >= 0.250 {
                    self.encouraged_points.retain(|p| p.distance(point) > 1e-6);
                    self.encouraged_points.push(point);
                    let excess = self.encouraged_points.len().saturating_sub(2);
                    self.encouraged_points.drain(..excess);
                    self.acquired_point = Some(point);
                }
                return;
            }
        }
        self.screen_dwell = Some((point, now));
    }
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
        self.snap_screen_excluding(ray,scene,start,close,ctx,None)
    }

    /// Same source inference with a borrowed transform exclusion set; no scene
    /// clone, geometry rewrite, or per-frame identity allocation is required.
    pub fn snap_screen_excluding(
        &mut self,ray:&Ray,scene:&Scene,start:Option<DVec3>,close:Option<DVec3>,
        ctx:&ScreenSnapContext,exclusions:Option<&SnapExclusions>,
    )->SnapResult {
        self.snap_screen_with_policy(ray,scene,start,close,ctx,&ScreenSnapPolicy{exclusions,..Default::default()})
    }
    pub fn snap_screen_with_policy(
        &mut self,ray:&Ray,scene:&Scene,start:Option<DVec3>,close:Option<DVec3>,ctx:&ScreenSnapContext,policy:&ScreenSnapPolicy,
    )->SnapResult{
        self.sync_drawing_frame(scene.drawing_frame);
        self.update_center(scene, ray, ctx);
        let edges = edges(scene, ctx, policy.exclusions);
        let pixels = |p: DVec3| {
            ctx.viewport
                .project_for_snapping(p)
                .map(|q| q.distance(ctx.cursor))
                .unwrap_or(f64::INFINITY)
        };
        let line = self
            .locked_line
            .map(|(a, b)| (a, (b - a).normalize_or_zero(), None))
            .or_else(|| {
                self.locked_axis.zip(start).map(|(axis, p)| {
                    let d = match axis {
                        'x' | 'X' => self.drawing_axes.x,
                        'y' | 'Y' => self.drawing_axes.y,
                        _ => self.drawing_axes.z,
                    };
                    (p, d, Some(axis.to_ascii_lowercase()))
                })
            })
            .or_else(|| self.shift_lock.map(|(a,b)| (a,(b-a).normalize_or_zero(),None)));
        let on_line = |p: DVec3| line.is_none_or(|(a, d, _)| (p - a).cross(d).length() < 1e-5);
        if let Some(p) =
            close.filter(|p| on_line(*p) && pixels(*p) <= 9. && scene.section_point_visible(*p))
        {
            return result(p, SnapKind::Close, COLOR_CLOSE, "Close Loop");
        }
        // Stable half-pixel buckets preserve source loose-context tie precedence.
        let mut points = vec![];
        if self.center_hint_pixel(&ctx.viewport,ctx.cursor).is_some() {
            if let Some(reference)=&self.center_reference {
                points.push((pixels(reference.point),reference.point,SnapKind::Center,false));
            }
        }
        points.push((pixels(DVec3::ZERO), DVec3::ZERO, SnapKind::Origin, false));
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
                return result(
                    p,
                    kind,
                    if kind == SnapKind::Origin {
                        COLOR_ORIGIN
                    } else if kind == SnapKind::Center {
                        COLOR_ENDPOINT
                    } else {
                        COLOR_ENDPOINT
                    },
                    if kind == SnapKind::Origin {
                        "Origin"
                    } else if kind == SnapKind::Center {
                        "Center"
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
                self.acquired_point
                    .or_else(|| self.encouraged_points.last().copied()),
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
            let mut r = result(p, if axis.is_some() { SnapKind::AxisLock } else { SnapKind::Reference }, color, "Locked Line");
            r.axis = axis;
            r.guides
                .push((origin - dir * 100., origin + dir * 100., color));
            return r;
        }
        if let Some(start) = start {
            if let Some(dir) = axis_direction(ctx.candidate - start, 10.,self.drawing_axes) {
                if let Some(r) = from_point(
                    scene,
                    ctx,
                    &edges,
                    self.acquired_point
                        .or_else(|| self.encouraged_points.last().copied()),
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
        let face_under_cursor=hover_face(scene,ray).is_some();
        if let Some(r) = memory_snap(self, scene, ctx, face_under_cursor) {
            return r;
        }
        // Weak angular cues preserve the raw candidate, as source rule9 does.
        if let Some(start) = start {
            let d = ctx.candidate - start;
            if d.length() > 1e-6 {
                let n = d.normalize();
                let axes=[('x',self.drawing_axes.x,COLOR_AXIS_X),('y',self.drawing_axes.y,COLOR_AXIS_Y),('z',self.drawing_axes.z,COLOR_AXIS_Z)];
                if let Some(angle)=policy.magnetic_axis_deg {
                    if let Some(&(axis,dir,color))=axes.iter().find(|(_,dir,_)|n.dot(*dir).abs()>=angle.to_radians().cos()){
                        let point=project_line(ray,start,dir);
                        let mut r=result(point,SnapKind::AxisLock,color,"Axis");r.axis=Some(axis);r.guides.push((start,point,color));return r;
                    }
                }
                if let (Some(threshold),Some(pixel))=(policy.screen_axis_px,ctx.viewport.project_for_snapping(start)){
                    let mut best=None;
                    for (axis,dir,color) in axes {
                        let Some(other)=ctx.viewport.project_for_snapping(start+dir*d.length())else{continue;};
                        let heading=other-pixel;let span=heading.length();if span<12.{continue;}
                        let cursor=ctx.cursor-pixel;let distance=(cursor.x*heading.y-cursor.y*heading.x).abs()/span;
                        if distance<=threshold&&best.is_none_or(|(old,_,_,_)|distance<old){best=Some((distance,axis,dir,color));}
                    }
                    if let Some((_,axis,dir,color))=best{
                        let point=project_line(ray,start,dir);
                        if point.distance(start)<=5.*d.length(){let mut r=result(point,SnapKind::AxisLock,color,"Axis");r.axis=Some(axis);r.guides.push((start,point,color));return r;}
                    }
                }
                for (axis, dir, color) in [
                    ('x', self.drawing_axes.x, COLOR_AXIS_X),
                    ('y', self.drawing_axes.y, COLOR_AXIS_Y),
                    ('z', self.drawing_axes.z, COLOR_AXIS_Z),
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
        if policy.face_under_cursor {return result(ctx.candidate,SnapKind::OnFace,COLOR_ON_FACE,"On Face");}
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
    fn face_fallback_reports_surface_but_not_a_hole(){
        let c=camera(10.);let mut scene=Scene::new();
        let outer=[DVec3::new(1.,1.,0.),DVec3::new(5.,1.,0.),DVec3::new(5.,5.,0.),DVec3::new(1.,5.,0.)].map(|p|scene.mesh.add_vertex(p));
        scene.mesh.add_face(outer.to_vec(),vec![]);
        let (ray,ctx)=context(&c,DVec3::new(3.,3.,0.),DVec2::ZERO);
        let mut engine=SnapEngine::default();
        assert_eq!(engine.snap_screen(&ray,&scene,None,None,&ctx).kind,SnapKind::None,"standalone source caller defaults face_under_cursor to false");
        let r=engine.snap_screen_with_policy(&ray,&scene,None,None,&ctx,&ScreenSnapPolicy{face_under_cursor:true,..Default::default()});
        assert_eq!(r.kind,SnapKind::OnFace);assert_eq!(r.point,ctx.candidate);
        let hole=[DVec3::new(2.,2.,0.),DVec3::new(2.,4.,0.),DVec3::new(4.,4.,0.),DVec3::new(4.,2.,0.)].map(|p|scene.mesh.add_vertex(p));
        scene.mesh.faces.clear();scene.mesh.add_face(outer.to_vec(),vec![hole.to_vec()]);engine.note_model_revision(2);
        assert_eq!(engine.snap_screen(&ray,&scene,None,None,&ctx).kind,SnapKind::None);
    }

    #[test]
    fn transform_exclusions_leave_independent_endpoint_and_copy_restores_original(){
        let c=camera(10.);let mut scene=Scene::new();
        let a=scene.mesh.add_vertex(DVec3::new(2.,3.,0.));let b=scene.mesh.add_vertex(DVec3::new(4.,3.,0.));
        let d=scene.mesh.add_vertex(DVec3::new(2.,3.05,0.));let e=scene.mesh.add_vertex(DVec3::new(2.,4.,0.));
        let moving=scene.mesh.add_edge(a,b).unwrap();scene.mesh.add_edge(d,e);
        let (ray,ctx)=context(&c,DVec3::new(2.02,3.,0.),DVec2::ZERO);
        let exclude=SnapExclusions{loose_edges:std::collections::HashSet::from([moving]),groups:Default::default()};
        let mut engine=SnapEngine::default();
        let r=engine.snap_screen_excluding(&ray,&scene,Some(scene.mesh.vertices[a].position),None,&ctx,Some(&exclude));
        assert_eq!(r.kind,SnapKind::Endpoint);assert_eq!(r.point,scene.mesh.vertices[d].position);
        let r=engine.snap_screen(&ray,&scene,Some(scene.mesh.vertices[a].position),None,&ctx);
        assert_eq!(r.kind,SnapKind::Endpoint);assert_eq!(r.point,scene.mesh.vertices[a].position);
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
    fn moving_curve_exclusion_keeps_source_center_and_dwell_reference() {
        let c=camera(10.);let mut scene=Scene::new();let center=DVec3::new(3.,2.,0.);
        let ids=(0..24).map(|i|{let a=std::f64::consts::TAU*i as f64/24.;scene.mesh.add_vertex(center+DVec3::new(a.cos(),a.sin(),0.))}).collect::<Vec<_>>();
        for i in 0..24 {let e=scene.mesh.add_edge(ids[i],ids[(i+1)%24]).unwrap();scene.mesh.edges[e].curve_id=Some(71);}
        let exclusions=SnapExclusions{loose_edges:(0..24).collect(),groups:Default::default()};
        let policy=ScreenSnapPolicy{exclusions:Some(&exclusions),..Default::default()};
        let mut engine=SnapEngine::default();let before=scene.clone();
        let (ray,ctx)=context(&c,center+DVec3::X,DVec2::ZERO);
        let rim=engine.snap_screen_with_policy(&ray,&scene,None,None,&ctx,&policy);
        assert_ne!(rim.kind,SnapKind::Endpoint,"moving rim edges are excluded");
        assert!(engine.hovered_center.is_some(),"source acquires center before edge exclusion");
        engine.note_screen_result(&rim,true,0.);engine.note_screen_result(&rim,true,0.3);
        assert_eq!(engine.encouraged_points,vec![center]);
        let (ray,ctx)=context(&c,center,DVec2::ZERO);
        let r=engine.snap_screen_with_policy(&ray,&scene,None,None,&ctx,&policy);
        assert_eq!(r.kind,SnapKind::Center);assert!(r.point.distance(center)<1e-6);
        assert_eq!(scene,before);
    }
    #[test]
    fn center_reference_refreshes_with_same_scene_version_and_retires_after_deletion() {
        let c = camera(10.);
        let mut scene = Scene::new();
        let center = DVec3::new(3., 2., 0.);
        let ids = (0..24)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 24.;
                scene
                    .mesh
                    .add_vertex(center + DVec3::new(a.cos(), a.sin(), 0.))
            })
            .collect::<Vec<_>>();
        scene.mesh.add_face(ids, vec![]);
        let before = scene.clone();
        let mut engine = SnapEngine::default();
        let (ray, ctx) = context(&c, center, DVec2::ZERO);
        let r = engine.snap_screen(&ray, &scene, None, None, &ctx);
        assert_eq!(r.kind, SnapKind::Center);
        assert!(r.point.distance(center) < 1e-6);
        assert_eq!(scene, before);
        assert!(engine.center_hint_pixel(&ctx.viewport,ctx.cursor+DVec2::X*20.).is_some());
        assert!(engine.center_hint_pixel(&ctx.viewport,ctx.cursor+DVec2::X*41.).is_none());
        for vertex in &mut scene.mesh.vertices {
            vertex.position += DVec3::X;
        }
        engine.note_model_revision(1);
        let (ray, ctx) = context(&c, center + DVec3::X, DVec2::ZERO);
        let r = engine.snap_screen(&ray, &scene, None, None, &ctx);
        assert_eq!(r.kind, SnapKind::Center);
        assert!(r.point.distance(center + DVec3::X) < 1e-6);
        scene.mesh.clear();
        engine.note_model_revision(2);
        assert_ne!(
            engine.snap_screen(&ray, &scene, None, None, &ctx).kind,
            SnapKind::Center
        );
        assert!(engine.center_reference.is_none());
        assert!(engine.center_hint_pixel(&ctx.viewport,ctx.cursor).is_none());
    }
    #[test]
    fn dwell_acquires_after_250ms_only_and_keeps_two_source_points() {
        let mut engine = SnapEngine::default();
        let p = DVec3::new(2., 3., 0.);
        let r = result(p, SnapKind::Endpoint, COLOR_ENDPOINT, "Endpoint");
        engine.note_screen_result(&r, false, 0.);
        engine.note_screen_result(&r, false, 0.249);
        assert!(engine.encouraged_points.is_empty());
        engine.note_screen_result(&r, false, 0.250);
        assert_eq!(engine.encouraged_points, vec![p]);
        for (i, p) in [DVec3::X, DVec3::Y].into_iter().enumerate() {
            let r = result(p, SnapKind::Midpoint, COLOR_MIDPOINT, "Midpoint");
            let t = 1. + i as f64;
            engine.note_screen_result(&r, false, t);
            engine.note_screen_result(&r, false, t + 0.250);
        }
        assert_eq!(engine.encouraged_points, vec![DVec3::X, DVec3::Y]);
        engine.note_screen_result(&r, true, 3.);
        assert_eq!(engine.acquired_point, Some(p));
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

#[cfg(test)]
mod operation_reset_tests {
    use super::*;
    #[test]
    fn reset_retires_references_and_preserves_configuration() {
        let mut engine = SnapEngine::new(0.123);
        engine.max_encouraged = 7;
        engine.locked_axis = Some('x');
        engine.locked_line = Some((DVec3::ZERO,DVec3::X));
        engine.reference_edge = Some((DVec3::ZERO,DVec3::Y));
        engine.encouraged_points.push(DVec3::ONE);
        engine.screen_dwell = Some((DVec3::ONE,1.));
        engine.acquired_point = Some(DVec3::ONE);
        engine.hovered_center = Some(DVec3::ONE);
        engine.note_model_revision(42);
        engine.reset_operation_state();
        assert_eq!(engine.snap_distance,0.123);
        assert_eq!(engine.max_encouraged,7);
        assert_eq!(engine.model_revision,Some(42));
        assert!(engine.locked_axis.is_none() && engine.locked_line.is_none());
        assert!(engine.reference_edge.is_none() && engine.encouraged_points.is_empty());
        assert!(engine.screen_dwell.is_none() && engine.acquired_point.is_none());
        assert!(engine.hovered_center.is_none() && engine.center_reference.is_none());
    }
}

#[cfg(test)]
mod shift_lock_tests {
    use super::*;
    #[test]
    fn press_captures_once_release_preserves_arrow_and_no_late_capture() {
        let mut engine=SnapEngine::default();
        let start=DVec3::new(2.,3.,0.);
        let mut inferred=result(start-DVec3::X*2.,SnapKind::AxisInference,COLOR_AXIS_X,"X");
        inferred.axis=Some('x');
        engine.note_inference_result(&inferred);
        engine.update_shift_state(true,Some(start));
        assert_eq!(engine.shift_lock,Some((start,start-DVec3::X)));
        engine.note_inference_result(&result(start+DVec3::Y, SnapKind::Reference,COLOR_REFERENCE,"Parallel"));
        engine.update_shift_state(true,Some(start));
        assert_eq!(engine.shift_lock,Some((start,start-DVec3::X)));
        engine.locked_axis=Some('z');
        engine.update_shift_state(false,Some(start));
        assert!(engine.shift_lock.is_none());
        assert_eq!(engine.locked_axis,Some('z'));
        engine.note_inference_result(&result(start,SnapKind::Endpoint,COLOR_ENDPOINT,"Endpoint"));
        engine.update_shift_state(true,Some(start));
        assert!(engine.shift_lock.is_none());
        engine.note_inference_result(&inferred);
        engine.update_shift_state(true,Some(start));
        assert!(engine.shift_lock.is_none(),"moving onto an inference while already holding Shift does not capture it");
        engine.reset_operation_state();
        engine.note_inference_result(&inferred);
        engine.update_shift_state(true,None);
        assert!(engine.shift_lock.is_none(),"source requires an active start point");
    }
}

#[cfg(test)]
mod curve_dwell_priority_tests {
    use super::*;
    #[test]
    fn curve_rim_encourages_center_but_face_corner_keeps_priority() {
        let center=DVec3::new(2.,3.,1.);
        let corner=center+DVec3::X;
        let endpoint=result(corner,SnapKind::Endpoint,COLOR_ENDPOINT,"Endpoint");
        let mut engine=SnapEngine::default();
        engine.hovered_center=Some(center);
        engine.center_reference=Some(CenterReference{path:vec![],key:CenterKey::Curve{curve:71,ends:[uuid::Uuid::new_v4(),uuid::Uuid::new_v4()]},point:center,radius:1.,version:0});
        engine.note_screen_result(&endpoint,false,0.);
        engine.note_screen_result(&endpoint,false,0.3);
        assert_eq!(engine.encouraged_points,vec![center]);
        engine.reset_operation_state();
        engine.hovered_center=Some(center);
        engine.center_reference=Some(CenterReference{path:vec![],key:CenterKey::Face{outer:vec![],run:(0,0)},point:center,radius:1.,version:0});
        engine.note_screen_result(&endpoint,false,1.);
        engine.note_screen_result(&endpoint,false,1.3);
        assert_eq!(engine.encouraged_points,vec![corner]);
    }
}

#[cfg(test)]
mod context_axes_source_acceptance {
    use super::*;
    use glam::DMat4;
    fn point(v:&serde_json::Value)->DVec3 {DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
    #[test]
    fn frozen_original_context_axes_and_shift_directions() {
        let evidence:serde_json::Value=serde_json::from_str(include_str!("../../../docs/audits/component-snap-axes-source.json")).unwrap();
        for row in evidence["cases"].as_array().unwrap() {
            let raw:Vec<_>=row["frame"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap()).collect();let mut scene=Scene::new();scene.drawing_frame=Some(DMat4::from_cols_array(&raw.try_into().unwrap()));
            let mut engine=SnapEngine::default();engine.sync_drawing_frame(scene.drawing_frame);let start=row["start"].as_array().map(|_|point(&row["start"]));engine.locked_axis=row["axis"].as_str().and_then(|s|s.chars().next());
            if !row["shift_capture"].is_null() {
                let capture=&row["shift_capture"];engine.note_inference_result(&SnapResult{point:point(&capture["point"]),kind:SnapKind::AxisInference,axis:Some('x'),..Default::default()});engine.update_shift_state(true,start);
                let (a,b)=engine.shift_lock.unwrap();assert!((b-a).normalize().distance(point(&capture["source_direction"]))<4e-6);
                if row["mode"]=="shift-released" {engine.update_shift_state(false,start);assert!(engine.shift_lock.is_none());}
            }
            let candidate=point(&row["candidate"]);let viewport=PixelViewport{matrix:DMat4::from_translation(DVec3::new(0.,0.,0.5))*DMat4::from_scale(DVec3::new(0.1,0.1,-0.001)),width:2000.,height:2000.};let ctx=ScreenSnapContext{viewport,cursor:viewport.project_for_snapping(candidate).unwrap(),candidate,plane_normal:DVec3::Z};let policy=ScreenSnapPolicy{magnetic_axis_deg:row["magnetic"].as_f64(),..Default::default()};
            let snap=engine.snap_screen_with_policy(&Ray::new(DVec3::new(candidate.x,candidate.y,100.),-DVec3::Z),&scene,start,None,&ctx,&policy);
            let kind=match snap.kind {SnapKind::AxisLock=>"axis",SnapKind::AxisInference=>"axis_inference",SnapKind::Reference=>"reference",SnapKind::Origin=>"origin",SnapKind::None=>"none",_=>"other"};assert_eq!(kind,row["original"]["kind"].as_str().unwrap(),"{}",row["name"]);assert_eq!(snap.axis.map(|c|c.to_string()),row["original"]["axis"].as_str().map(|s|s.to_string()));assert!(snap.point.distance(point(&row["original"]["point"]))<4e-6,"{}",row["name"]);
            engine.reset_operation_state();engine.sync_drawing_frame(None);assert_eq!(engine.drawing_axes,crate::drawing_axes::DrawingAxes::default());assert!(engine.shift_lock.is_none());assert!(engine.locked_axis.is_none());
        }
    }
}
