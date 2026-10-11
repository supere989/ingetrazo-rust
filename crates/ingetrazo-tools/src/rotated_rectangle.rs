// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    base::{Tool, ToolContext, ToolType},
    shape_plane::{self as qt, ShapePlane},
};
use glam::DVec3;
use ingetrazo_core::{
    scene::Scene,
    screen_pick::PixelViewport,
    snap::{SnapKind, SnapResult},
};
#[derive(Debug, Clone)]
pub struct RotatedRectangleTool {
    first_corner: Option<DVec3>,
    second_corner: Option<DVec3>,
    hover_point: Option<DVec3>,
    plane: ShapePlane,
    angle: f64,
    held_dir: Option<DVec3>,
    held_angle: Option<f64>,
    forced: Option<DVec3>,
    axis: Option<char>,
    point_snap: bool,
    disc_radius: f64,
    near_disc: bool,
    error: Option<String>,
}
impl Default for RotatedRectangleTool {
    fn default() -> Self {
        Self {
            first_corner: None,
            second_corner: None,
            hover_point: None,
            plane: Default::default(),
            angle: 0.,
            held_dir: None,
            held_angle: None,
            forced: None,
            axis: None,
            point_snap: false,
            disc_radius: 1.,
            near_disc: false,
            error: None,
        }
    }
}
impl RotatedRectangleTool {
    fn reset(&mut self) {
        self.first_corner = None;
        self.second_corner = None;
        self.plane.reset();
        self.angle = 0.;
        self.held_dir = None;
        self.held_angle = None;
        self.forced = None;
        self.axis = None;
        self.near_disc = false;
        self.error = None;
    }
    fn normal(&self) -> DVec3 {
        self.plane
            .captured
            .map(|p| qt::norm(p.1))
            .unwrap_or(DVec3::Z)
    }
    fn edge(&self) -> DVec3 {
        qt::norm(qt::sub(
            self.second_corner.unwrap(),
            self.first_corner.unwrap(),
        ))
    }
    fn perp(&self) -> DVec3 {
        qt::norm(qt::cross(self.normal(), self.edge()))
    }
    fn in_plane(&self, d: DVec3) -> DVec3 {
        qt::sub(d, qt::mul(self.normal(), qt::dot(d, self.normal())))
    }
    fn width_dir(&self, angle: f64) -> DVec3 {
        let a = angle.to_radians();
        qt::norm(qt::add(
            qt::mul(self.perp(), a.cos()),
            qt::mul(self.normal(), a.sin()),
        ))
    }
    fn edge_angle(&self, d: DVec3) -> f64 {
        let (u, v) = self.plane.axes_for(self.normal());
        qt::dot(d, v)
            .atan2(qt::dot(d, u))
            .to_degrees()
            .rem_euclid(360.)
    }
    fn direction(&self, angle: f64) -> DVec3 {
        let (u, v) = self.plane.axes_for(self.normal());
        let a = angle.to_radians();
        qt::norm(qt::add(qt::mul(u, a.cos()), qt::mul(v, a.sin())))
    }
    fn width(&self, p: DVec3, forced: Option<DVec3>) -> (f64, f64) {
        let edge = self.edge();
        let mut d = qt::sub(p, self.second_corner.unwrap());
        d = qt::sub(d, qt::mul(edge, qt::dot(d, edge)));
        if let Some(f) = forced {
            d = qt::mul(f, qt::dot(d, f));
        }
        let w = qt::length(d);
        if w < 1e-9 {
            return (0., self.angle);
        }
        let mut angle = qt::dot(d, self.normal())
            .atan2(qt::dot(d, self.perp()))
            .to_degrees();
        if self.near_disc && forced.is_none() {
            angle = (angle / 15.).round_ties_even() * 15.;
        }
        for t in [-180., -90., 0., 90., 180.] {
            if (angle - t).abs() <= 3. {
                angle = t;
                break;
            }
        }
        (w, angle)
    }
    fn width_cursor(&self, c: &ToolContext) -> DVec3 {
        if self.point_snap {
            return qt::q(c.world);
        }
        let n = self.edge();
        let denominator = qt::dot(c.ray.direction, n);
        if denominator.abs() < 0.02 {
            return qt::q(c.world);
        }
        let t = qt::dot(qt::sub(self.second_corner.unwrap(), c.ray.origin), n) / denominator;
        if t <= 0. {
            return qt::q(c.world);
        }
        qt::add(c.ray.origin, qt::mul(c.ray.direction, t))
    }
    fn corners(&self, width: f64, angle: f64) -> Option<[DVec3; 4]> {
        let dir = self.width_dir(angle);
        if qt::length(dir) < 1e-6 {
            return None;
        }
        let off = qt::mul(dir, width);
        let s = self.first_corner?;
        let e = self.second_corner?;
        Some([s, e, qt::add(e, off), qt::add(s, off)])
    }
    fn metrics(&mut self, c: &ToolContext, center: DVec3, n: DVec3) {
        self.near_disc = false;
        let Some(v) = self.plane.viewport else {
            return;
        };
        let (u, _) = self.plane.axes_for(n);
        let (Some(p0), Some(p1)) = (
            v.project_for_snapping(center),
            v.project_for_snapping(qt::add(center, u)),
        ) else {
            return;
        };
        let px = p0.distance(p1);
        if px > 1e-6 {
            self.disc_radius = 60. / px;
        }
        self.near_disc = c.screen.is_some_and(|p| p.distance(p0) <= 75.);
    }
    fn disc(&self, center: DVec3, u: DVec3, v: DVec3) -> Vec<(DVec3, DVec3)> {
        if qt::length(u) < 1e-9 || qt::length(v) < 1e-9 {
            return vec![];
        }
        let rim = |a: f64, k: f64| {
            qt::add(
                center,
                qt::mul(
                    qt::add(qt::mul(u, a.cos()), qt::mul(v, a.sin())),
                    self.disc_radius * k,
                ),
            )
        };
        let pts = (0..48)
            .map(|i| rim(std::f64::consts::TAU * i as f64 / 48., 1.))
            .collect::<Vec<_>>();
        let mut out = (0..48)
            .map(|i| (pts[i], pts[(i + 1) % 48]))
            .collect::<Vec<_>>();
        for k in 0..24 {
            let a = (k as f64 * 15.).to_radians();
            out.push((rim(a, if k % 6 == 0 { 0.75 } else { 0.86 }), rim(a, 1.)));
        }
        out
    }
    fn commit(&mut self, mut corners: [DVec3; 4], s: &mut Scene) -> Result<(), String> {
        let normal = ingetrazo_core::math::newell_vector(&corners);
        if normal.dot(self.normal()) < 0. {
            corners.reverse();
        }
        let segments = (0..4)
            .map(|i| (corners[i], corners[(i + 1) % 4]))
            .collect::<Vec<_>>();
        let mesh = ingetrazo_core::drafting::add_segments_tagged_with_id(
            &s.mesh,
            &segments,
            Some(&corners),
            None,
            s.active_ifc.as_ref(),
            s.bim_draw_id.as_ref(),
        )?;
        s.mesh = mesh;
        s.bump_version();
        self.reset();
        Ok(())
    }
    fn click_result(&mut self, s: &mut Scene, c: &ToolContext) -> Result<(), String> {
        self.plane.axes = s.drawing_axes();
        if self.first_corner.is_none() {
            let p = qt::q(c.world);
            self.plane.start(p);
            if self.plane.captured.is_none() {
                if let Some((_, normal, _)) = ingetrazo_core::screen_pick::hover_face(s, &c.ray) {
                    self.plane.captured = Some((p, qt::q(normal)));
                }
            }
            self.first_corner = Some(p);
            return Ok(());
        }
        if self.second_corner.is_none() {
            let p = if self.held_dir.is_some() {
                self.hover_point.unwrap_or(qt::q(c.world))
            } else {
                qt::q(c.world)
            };
            if qt::length(qt::sub(p, self.first_corner.unwrap())) < 1e-6 {
                return Ok(());
            }
            self.second_corner = Some(p);
            self.held_dir = None;
            if qt::length(self.perp()) < 1e-6 {
                self.second_corner = None;
                return Err("Baseline is perpendicular to the drawing plane".into());
            }
            return Ok(());
        }
        let (mut w, mut a) = self.width(self.width_cursor(c), self.forced);
        if self.forced.is_some() && w < 1e-6 {
            if let Some(p) = self.hover_point {
                (w, a) = self.width(p, self.forced);
            }
        }
        if w < 1e-6 {
            return Err("The width is zero".into());
        }
        let corners = self.corners(w, a).ok_or("Invalid rectangle")?;
        self.commit(corners, s)
    }
}
impl Tool for RotatedRectangleTool {
    fn tool_type(&self) -> ToolType {
        ToolType::RotatedRectangle
    }
    fn name(&self) -> &'static str {
        "Rotated Rectangle"
    }
    fn vcb_label(&self) -> &'static str {
        if self.second_corner.is_none() {
            "Length; angle"
        } else {
            "Width; angle"
        }
    }
    fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }
    fn magnetic_axis_deg(&self) -> Option<f64> {
        self.second_corner.is_none().then_some(3.)
    }
    fn screen_axis_px(&self) -> Option<f64> {
        self.magnetic_axis_deg().map(|_| 9.)
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.first_corner
    }
    fn note_cursor_plane(&mut self, p: DVec3, n: DVec3) {
        self.plane.note(p, n);
    }
    fn note_pick_viewport(&mut self, v: PixelViewport, _: DVec3, _: u64) {
        self.plane.viewport = Some(v);
    }
    fn note_snap_result(&mut self, r: &SnapResult) {
        self.point_snap = matches!(
            r.kind,
            SnapKind::Endpoint
                | SnapKind::Midpoint
                | SnapKind::Intersection
                | SnapKind::OnEdge
                | SnapKind::Close
                | SnapKind::Origin
                | SnapKind::Center
        );
        self.axis = if r.kind == SnapKind::AxisLock {
            r.axis
        } else {
            None
        };
    }
    fn lock_drawing_plane(&mut self, n: DVec3) -> bool {
        self.first_corner.is_none() && self.plane.lock_axis(n)
    }
    fn lock_drawing_reference(
        &mut self,
        r: Option<ingetrazo_core::screen_pick::HoverReference>,
    ) -> bool {
        self.first_corner.is_none() && self.plane.lock_reference(r)
    }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> {
        self.first_corner
            .map(|p| (p, self.normal()))
            .or_else(|| self.plane.locked(self.hover_point))
    }
    fn on_activate(&mut self, s: &mut Scene) {
        self.plane.axes = s.drawing_axes();
        self.reset();
        self.hover_point = None;
    }
    fn on_deactivate(&mut self, _: &mut Scene) {
        self.reset();
        self.hover_point = None;
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_click(&mut self, s: &mut Scene, c: &ToolContext) {
        if let Err(e) = self.click_result(s, c) {
            self.error = Some(e);
        }
    }
    fn on_move(&mut self, s: &mut Scene, c: &ToolContext) {
        self.plane.axes = s.drawing_axes();
        self.hover_point = Some(if self.second_corner.is_some() {
            self.width_cursor(c)
        } else {
            qt::q(c.world)
        });
        self.forced = None;
        if let Some(start) = self.first_corner {
            if self.second_corner.is_none() {
                self.metrics(c, start, self.normal());
                if self.near_disc {
                    let d = self.in_plane(qt::sub(c.world, start));
                    if qt::length(d) > 1e-9 {
                        let a = (self.edge_angle(d) / 15.).round_ties_even() * 15.;
                        self.hover_point =
                            Some(qt::add(start, qt::mul(self.direction(a), qt::length(d))));
                    }
                }
                if !c.shift_held {
                    self.held_dir = None;
                } else {
                    let d = self.in_plane(qt::sub(self.hover_point.unwrap(), start));
                    if self.held_dir.is_none() && qt::length(d) > 1e-9 {
                        self.held_dir = Some(qt::norm(d));
                    }
                    if let Some(dir) = self.held_dir {
                        self.hover_point = Some(qt::add(
                            start,
                            qt::mul(dir, qt::dot(qt::sub(c.world, start), dir)),
                        ));
                    }
                }
            } else {
                if let Some(axis) = self.axis.and_then(|a| self.plane.axes.axis(a)) {
                    let d = qt::sub(axis, qt::mul(self.edge(), qt::dot(axis, self.edge())));
                    if qt::length(d) > 1e-6 {
                        self.forced = Some(qt::norm(d));
                    }
                }
                self.metrics(c, self.second_corner.unwrap(), self.edge());
                if !c.shift_held {
                    self.held_angle = None;
                } else if self.held_angle.is_none() {
                    self.held_angle = Some(self.angle);
                }
                if let Some(a) = self.held_angle {
                    self.forced = Some(self.width_dir(a));
                }
                let (w, a) = self.width(self.hover_point.unwrap(), self.forced);
                if w > 1e-6 {
                    self.angle = a;
                }
            }
        }
    }
    fn on_vcb_input(&mut self, s: &mut Scene, text: &str) -> Result<(), String> {
        let start = self.first_corner.ok_or("Place the first corner first")?;
        let scale = ingetrazo_core::units::model_scale(s);
        let values = ingetrazo_core::units::vcb::parse_values(text, scale, true)?;
        if values.len() > 2 {
            return Err("Enter length or length;angle".into());
        }
        if self.second_corner.is_none() {
            let mut length = values[0];
            let mut dir = if values.len() == 2 {
                self.direction(values[1] / scale)
            } else {
                let d = self
                    .hover_point
                    .map(|p| qt::sub(p, start))
                    .unwrap_or(DVec3::ZERO);
                if qt::length(d) > 1e-9 {
                    qt::norm(d)
                } else {
                    self.direction(0.)
                }
            };
            if length == 0. {
                return Err("Baseline length is zero".into());
            }
            if length < 0. {
                length = -length;
                dir = -dir;
            }
            let base = qt::add(start, qt::mul(dir, length));
            self.second_corner = Some(base);
            if qt::length(self.perp()) < 1e-6 {
                self.second_corner = None;
                return Err("Baseline is perpendicular to the drawing plane".into());
            }
            return Ok(());
        }
        if self.hover_point.is_none() {
            return Err("Move to set the width direction first".into());
        }
        let w = values[0];
        let angle = if values.len() == 2 {
            if w <= 0. {
                return Err("Width must be positive".into());
            }
            values[1] / scale
        } else {
            if w == 0. {
                return Err("Width is zero".into());
            }
            self.angle
        };
        let corners = self.corners(w, angle).ok_or("Invalid rectangle")?;
        self.commit(corners, s)
    }
    fn preview_color(&self) -> Option<[f32; 3]> {
        self.plane.lock_color()
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        let (Some(start), Some(hover)) = (self.first_corner, self.hover_point) else {
            return vec![];
        };
        if self.second_corner.is_none() {
            let (u, v) = self.plane.axes_for(self.normal());
            let mut out = vec![(start, hover)];
            out.extend(self.disc(start, u, v));
            return out;
        }
        let (w, a) = self.width(hover, self.forced);
        let mut out = if let Some(c) = self.corners(w, a) {
            (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect::<Vec<_>>()
        } else {
            vec![(start, self.second_corner.unwrap())]
        };
        out.extend(self.disc(self.second_corner.unwrap(), self.perp(), self.normal()));
        out
    }
}
