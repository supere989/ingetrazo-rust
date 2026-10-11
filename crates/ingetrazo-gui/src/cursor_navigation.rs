// SPDX-License-Identifier: GPL-3.0-or-later
//! Gesture-owned orbit pivot and source wheel focus. No geometry copies or
//! per-frame picks: the existing bounded visible-ray index owns cold geometry.
use crate::IngeTrazoApp;
use glam::{DVec2, DVec3, Vec3};
use ingetrazo_core::{camera::OrbitCamera, math::Ray, scene::Scene};
#[derive(Clone, Copy)]
struct Drag {
    button: egui::PointerButton,
    last: DVec2,
    pivot: DVec3,
    pan: bool,
    depth: f64,
}
#[derive(Clone, Copy)]
struct Focus {
    pose: [f64; 6],
    pixel: DVec2,
    revision: (u64, u64, u64),
    point: Option<DVec3>,
}
#[derive(Default)]
pub(crate) struct CursorNavigation {
    pointer: Option<egui::Pos2>,
    drag: Option<Drag>,
    focus: Option<Focus>,
    bounds_key: Option<(u64, u64, u64)>,
    bounds: Option<(DVec3, DVec3)>,
    pub acquisitions: u64,
}
fn pose(c: &OrbitCamera) -> [f64; 6] {
    let values = [
        c.yaw, c.pitch, c.distance, c.target.x, c.target.y, c.target.z,
    ];
    std::array::from_fn(|i| {
        let scale = if i < 2 { 1e9 } else { 1e6 };
        (values[i] * scale).round_ties_even() / scale
    })
}
fn revision(a: &IngeTrazoApp) -> (u64, u64, u64) {
    (a.scene.version, a.scene.view_version, a.model_revision)
}
fn bounds(scene: &Scene) -> Option<(DVec3, DVec3)> {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    fn add(p: DVec3, lo: &mut DVec3, hi: &mut DVec3) {
        if p.is_finite() {
            *lo = lo.min(p);
            *hi = hi.max(p);
        }
    }
    for e in &scene.mesh.edges {
        if scene.is_layer_visible(e.layer.as_deref()) {
            for i in [e.v0, e.v1] {
                if let Some(v) = scene.mesh.vertices.get(i) {
                    add(v.position.as_vec3().as_dvec3(), &mut lo, &mut hi);
                }
            }
        }
    }
    for (i, f) in scene.mesh.faces.iter().enumerate() {
        if scene.mesh_face_visible(&scene.mesh, i) {
            for &i in &f.vertices {
                if let Some(v) = scene.mesh.vertices.get(i) {
                    add(v.position.as_vec3().as_dvec3(), &mut lo, &mut hi);
                }
            }
        }
    }
    fn groups(
        scene: &Scene,
        g: &ingetrazo_core::groups::Group,
        parent: glam::DMat4,
        lo: &mut DVec3,
        hi: &mut DVec3,
    ) {
        let m = ingetrazo_core::change_axes::multiply(parent, g.transform);
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            for v in &g.mesh.vertices {
                add(m.transform_point3(v.position.as_vec3().as_dvec3()), lo, hi);
            }
        }
        for c in &g.children {
            groups(scene, c, m, lo, hi);
        }
    }
    for g in &scene.groups {
        if !g.hidden && !g.billboard && scene.is_layer_visible(g.layer.as_deref()) {
            groups(scene, g, glam::DMat4::IDENTITY, &mut lo, &mut hi);
        }
    }
    if lo.is_finite() && hi.is_finite() {
        Some((lo.as_vec3().as_dvec3(), hi.as_vec3().as_dvec3()))
    } else {
        None
    }
}
fn span(ray: &Ray, lo: DVec3, hi: DVec3) -> Option<(f64, f64)> {
    let mut near = 0f64;
    let mut far = f64::INFINITY;
    for i in 0..3 {
        if ray.dir[i].abs() < 1e-12 {
            if ray.origin[i] < lo[i] - 1e-9 || ray.origin[i] > hi[i] + 1e-9 {
                return None;
            }
        } else {
            let a = (lo[i] - ray.origin[i]) / ray.dir[i];
            let b = (hi[i] - ray.origin[i]) / ray.dir[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    if far.is_finite() {
        Some((near, far))
    } else {
        None
    }
}
fn qt_point(ray: &Ray, t: f64) -> DVec3 {
    (ray.origin.as_vec3() + ray.dir.as_vec3() * (t as f32)).as_dvec3()
}
fn ray(a: &IngeTrazoApp, p: DVec2, r: egui::Rect) -> Option<Ray> {
    crate::navigation_ray::cursor_ray(
        &a.camera,
        p.x,
        p.y,
        f64::from(r.width()),
        f64::from(r.height()),
    )
}
fn acquire(a: &mut IngeTrazoApp, p: DVec2, r: egui::Rect, orbit: bool) -> Option<DVec3> {
    let cursor_ray = ray(a, p, r)?;
    a.navigation.prepare_collision(&a.scene, a.model_revision);
    a.cursor_navigation.acquisitions = a.cursor_navigation.acquisitions.wrapping_add(1);
    if let Some((_, _, t)) = a.navigation.hit(&a.scene, &cursor_ray) {
        return Some(qt_point(&cursor_ray, t));
    }
    let key = revision(a);
    if a.cursor_navigation.bounds_key != Some(key) {
        a.cursor_navigation.bounds = bounds(&a.scene);
        a.cursor_navigation.bounds_key = Some(key);
    }
    if orbit {
        if let Some((lo, hi)) = a.cursor_navigation.bounds {
            let center = ray(
                a,
                DVec2::new(f64::from(r.width()) / 2., f64::from(r.height()) / 2.),
                r,
            )?;
            if let Some((n, f)) = span(&center, lo, hi) {
                return Some(qt_point(&center, (n + f) * 0.5));
            }
        }
        return Some(a.camera.target.as_vec3().as_dvec3());
    }
    if cursor_ray.dir.z.abs() > 1e-6 {
        let t = -cursor_ray.origin.z / cursor_ray.dir.z;
        if t > 0. {
            return Some(qt_point(&cursor_ray, t));
        }
    }
    let view = (a.camera.target.as_vec3()
        - Vec3::from_array(crate::navigation_ray::camera_snapshot(&a.camera)?.eye))
    .normalize_or_zero();
    let denominator = cursor_ray.dir.as_vec3().dot(view);
    if denominator.abs() > 1e-6 {
        let t = (a.camera.target.as_vec3() - cursor_ray.origin.as_vec3()).dot(view) / denominator;
        if t > 0. {
            return Some(qt_point(&cursor_ray, f64::from(t)));
        }
    }
    None
}
fn project(a: &IngeTrazoApp, p: DVec3, r: egui::Rect) -> Option<DVec2> {
    let m = crate::navigation_ray::camera_snapshot(&a.camera)?.matrix;
    let q = glam::Mat4::from_cols_array_2d(&m) * p.as_vec3().extend(1.);
    if q.w <= 0. || !q.is_finite() {
        return None;
    }
    Some(DVec2::new(
        (f64::from(q.x) / f64::from(q.w) * 0.5 + 0.5) * f64::from(r.width()),
        (1. - (f64::from(q.y) / f64::from(q.w) * 0.5 + 0.5)) * f64::from(r.height()),
    ))
}
pub(crate) fn wheel(a: &mut IngeTrazoApp, p: DVec2, r: egui::Rect, steps: f64) -> bool {
    if !p.is_finite() || !steps.is_finite() {
        return false;
    }
    let key = revision(a);
    let old = a.cursor_navigation.focus;
    let close = |q: DVec2| (q.x - p.x).abs() < 24. && (q.y - p.y).abs() < 24.;
    let focus = if let Some(old) = old.filter(|f| f.revision == key) {
        if old.pose == pose(&a.camera) && close(old.pixel) {
            old.point
        } else if old
            .point
            .and_then(|point| project(a, point, r))
            .is_some_and(close)
        {
            old.point
        } else {
            acquire(a, p, r, false)
        }
    } else {
        acquire(a, p, r, false)
    };
    if a.cursor_navigation.bounds_key != Some(key) {
        a.cursor_navigation.bounds = bounds(&a.scene);
        a.cursor_navigation.bounds_key = Some(key);
    }
    let accepted = if let Some(focus) = focus {
        let diag = a
            .cursor_navigation
            .bounds
            .map(|(lo, hi)| f64::from((hi.as_vec3() - lo.as_vec3()).length()))
            .unwrap_or(0.);
        a.camera.zoom_to(steps, focus, (0.01 * diag).max(0.05))
    } else {
        a.camera.zoom_steps(steps)
    };
    if accepted {
        a.cursor_navigation.focus = Some(Focus {
            pose: pose(&a.camera),
            pixel: p,
            revision: key,
            point: focus,
        });
    }
    accepted
}
/// Borrow the original event batch: each wheel notch and pointer transition is
/// handled once, rather than reapplying egui cumulative drag_delta/smoothed scroll.
pub(crate) fn input(a: &mut IngeTrazoApp, ui: &egui::Ui, r: egui::Rect, owns_pointer: bool) {
    if a.navigation_suspended() {
        a.cursor_navigation.drag = None;
        return;
    }
    ui.input(|i| {
        if !i.focused {
            a.cursor_navigation.drag = None;
            return;
        }
        // MouseWheel has no position field: retain prior ingress position, then
        // advance it only at the actual PointerMoved/Button event.
        let mut pointer = a.cursor_navigation.pointer;
        for event in &i.events {
            match event {
                egui::Event::PointerMoved(pos) => {
                    pointer = Some(*pos);
                    if let Some(mut drag) = a.cursor_navigation.drag {
                        let p = DVec2::new(f64::from(pos.x - r.left()), f64::from(pos.y - r.top()));
                        let rounded = DVec2::new(p.x.round(), p.y.round());
                        let delta = rounded - drag.last;
                        drag.last = rounded;
                        if drag.pan {
                            a.camera.pan_pixels(
                                delta.x,
                                delta.y,
                                f64::from(r.height()),
                                Some(drag.depth),
                            );
                        } else {
                            let invert = a
                                .document_session
                                .preferences
                                .extra
                                .get("nav/invert_orbit_y")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            a.camera.orbit_about(
                                drag.pivot,
                                delta.x,
                                if invert { -delta.y } else { delta.y },
                                f64::from(r.height()),
                            );
                        }
                        a.cursor_navigation.drag = Some(drag);
                    }
                }
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    pointer = Some(*pos);
                    if !pressed {
                        if a.cursor_navigation
                            .drag
                            .is_some_and(|d| d.button == *button)
                        {
                            a.cursor_navigation.drag = None;
                        }
                    } else if owns_pointer
                        && r.contains(*pos)
                        && *button == egui::PointerButton::Middle
                    {
                        let p = DVec2::new(f64::from(pos.x - r.left()), f64::from(pos.y - r.top()));
                        if let Some(pivot) = acquire(a, p, r, true) {
                            let eye = Vec3::from_array(
                                crate::navigation_ray::camera_snapshot(&a.camera)
                                    .map(|s| s.eye)
                                    .unwrap_or([0.; 3]),
                            );
                            let forward = (a.camera.target.as_vec3() - eye).normalize_or_zero();
                            let depth = f64::from((pivot.as_vec3() - eye).dot(forward));
                            a.cursor_navigation.drag = Some(Drag {
                                button: *button,
                                last: DVec2::new(p.x.round(), p.y.round()),
                                pivot,
                                pan: modifiers.shift,
                                depth,
                            });
                        }
                    }
                }
                egui::Event::MouseWheel { unit, delta, .. } => {
                    if let Some(pos) = pointer.filter(|p| owns_pointer && r.contains(*p)) {
                        let steps = match unit {
                            egui::MouseWheelUnit::Line => f64::from(delta.y),
                            egui::MouseWheelUnit::Point => 0.,
                            egui::MouseWheelUnit::Page => 0.,
                        };
                        let steps = if a
                            .document_session
                            .preferences
                            .extra
                            .get("nav/invert_wheel")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false)
                        {
                            -steps
                        } else {
                            steps
                        };
                        if steps != 0. {
                            wheel(
                                a,
                                DVec2::new(f64::from(pos.x - r.left()), f64::from(pos.y - r.top())),
                                r,
                                steps,
                            );
                        }
                    }
                }
                egui::Event::PointerGone => {
                    a.cursor_navigation.drag = None;
                    pointer = None;
                }
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => a.cursor_navigation.drag = None,
                _ => {}
            }
        }
        a.cursor_navigation.pointer = pointer;
    });
}

impl CursorNavigation {
    pub(crate) fn cancel(&mut self) {
        self.drag = None;
    }
}

impl IngeTrazoApp {
    #[doc(hidden)]
    pub fn cursor_navigation_probe(&self) -> (u64, Option<[f64; 3]>) {
        (
            self.cursor_navigation.acquisitions,
            self.cursor_navigation.drag.map(|d| d.pivot.to_array()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(a: &mut IngeTrazoApp, ctx: &egui::Context, events: Vec<egui::Event>, owns: bool) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                input_events(a, ui, owns);
            });
        });
    }
    fn input_events(a: &mut IngeTrazoApp, ui: &egui::Ui, owns: bool) {
        input(
            a,
            ui,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.)),
            owns,
        );
    }
    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Middle,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn unowned_press_and_pixel_scroll_do_not_move_camera() {
        let mut a = IngeTrazoApp::new();
        let c = a.camera.clone();
        let ctx = egui::Context::default();
        let p = egui::pos2(400., 300.);
        frame(
            &mut a,
            &ctx,
            vec![egui::Event::PointerMoved(p), button(p, true)],
            false,
        );
        assert_eq!(a.cursor_navigation.acquisitions, 0);
        frame(
            &mut a,
            &ctx,
            vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., 120.),
                modifiers: Default::default(),
            }],
            true,
        );
        assert_eq!(a.camera, c);
    }
    #[test]
    fn released_outside_and_escape_retire_pivot() {
        let mut a = IngeTrazoApp::new();
        let ctx = egui::Context::default();
        let p = egui::pos2(400., 300.);
        frame(&mut a, &ctx, vec![button(p, true)], true);
        assert!(a.cursor_navigation.drag.is_some());
        frame(
            &mut a,
            &ctx,
            vec![button(egui::pos2(900., 700.), false)],
            false,
        );
        assert!(a.cursor_navigation.drag.is_none());
        let c = a.camera.clone();
        frame(
            &mut a,
            &ctx,
            vec![egui::Event::PointerMoved(egui::pos2(450., 330.))],
            true,
        );
        assert_eq!(a.camera, c);
        frame(
            &mut a,
            &ctx,
            vec![
                button(p, true),
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                },
            ],
            true,
        );
        assert!(a.cursor_navigation.drag.is_none());
    }
    #[test]
    fn wheel_position_follows_event_order_not_final_hover() {
        let mut a = IngeTrazoApp::new();
        let ctx = egui::Context::default();
        let first = egui::pos2(400., 300.);
        let next = egui::pos2(600., 450.);
        frame(&mut a, &ctx, vec![egui::Event::PointerMoved(first)], true);
        frame(
            &mut a,
            &ctx,
            vec![
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0., 1.),
                    modifiers: Default::default(),
                },
                egui::Event::PointerMoved(next),
            ],
            true,
        );
        assert_eq!(
            a.cursor_navigation.focus.unwrap().pixel,
            DVec2::new(400., 300.)
        );
        assert_eq!(a.cursor_navigation.pointer, Some(next));
    }
}
