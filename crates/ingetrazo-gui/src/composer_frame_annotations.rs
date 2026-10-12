//! Model annotations projected into a ViewFrame, in source paper millimetres.
//! The same immutable plan supplies native canvas callbacks and physical export.
use crate::composer_frame_projection::Projector;
use glam::DVec3;
use ingetrazo_core::{composition::ViewFrame, scene::Scene};
use ingetrazo_render::{annotations as model, paper_scene::PaperSceneView};
use serde_json::{json, Value};
use std::sync::Arc;
const MAX_PRIMITIVES: usize = 100_000;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
#[path = "composer_frame_overlay_cache.rs"]
mod cache;
pub(crate) use cache::{build, Ground};
pub(super) use cache::{paint, State};
#[derive(Clone, Debug)]
pub(crate) enum Primitive {
    Line([f64; 4]),
    Arrow([f64; 4]),
    Text {
        at: [f64; 2],
        angle: f64,
        text: Arc<str>,
        size: f64,
    },
    Poly(Vec<[f64; 2]>),
    Section {
        line: [f64; 4],
        direction: [f64; 2],
        label: Arc<str>,
        size: f64,
    },
}
#[derive(Clone, Default, Debug)]
pub(crate) struct Plan {
    pub primitives: Vec<Primitive>,
    text_bytes: usize,
    pub(super) bytes: usize,
    commands: usize,
}
impl Plan {
    fn push(&mut self, primitive: Primitive) -> Result<(), String> {
        let finite = |v: &[f64]| v.iter().all(|x| x.is_finite() && x.abs() <= 1_000_000.);
        let (valid, bytes) = match &primitive {
            Primitive::Line(v) | Primitive::Arrow(v) => (finite(v), 0),
            Primitive::Poly(v) => (v.len() <= MAX_PRIMITIVES && v.iter().all(|p| finite(p)), 0),
            Primitive::Text {
                at,
                angle,
                text,
                size,
            } => (
                finite(at) && finite(&[*angle, *size]) && *size > 0.,
                text.len(),
            ),
            Primitive::Section {
                line,
                direction,
                label,
                size,
            } => (
                finite(line) && finite(direction) && finite(&[*size]) && *size > 0.,
                label.len(),
            ),
        };
        if !valid {
            return Err("Invalid frame annotation primitive".into());
        }
        let text_bytes = self
            .text_bytes
            .checked_add(bytes)
            .filter(|n| *n <= MAX_TEXT_BYTES)
            .ok_or("Frame annotation text budget exceeded")?;
        if self.primitives.len() >= MAX_PRIMITIVES {
            return Err("Frame annotation primitive budget exceeded".into());
        }
        let (extra, commands) = match &primitive {
            Primitive::Poly(v) => (v.capacity() * std::mem::size_of::<[f64; 2]>(), v.len() * 20),
            Primitive::Text { text, .. } => (text.len(), text.len() * 9 + 90),
            Primitive::Section { label, .. } => (label.len(), label.len() * 2 + 100),
            _ => (0, 20),
        };
        let total = self
            .bytes
            .checked_add(std::mem::size_of::<Primitive>() + extra)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or("Frame overlay plan budget exceeded")?;
        let commands = self
            .commands
            .checked_add(commands)
            .filter(|n| *n <= ingetrazo_formats::sheet_pdf::MAX_COMMANDS)
            .ok_or("Frame overlay command budget exceeded")?;
        self.primitives
            .try_reserve(1)
            .map_err(|_| "Frame annotation allocation refused")?;
        self.primitives.push(primitive);
        self.text_bytes = text_bytes;
        self.bytes = total;
        self.commands = commands;
        Ok(())
    }
    fn line(&mut self, a: [f64; 2], b: [f64; 2]) -> Result<(), String> {
        self.push(Primitive::Line([a[0], a[1], b[0], b[1]]))
    }
    fn text(
        &mut self,
        at: [f64; 2],
        angle: f64,
        text: impl Into<Arc<str>>,
        size: f64,
    ) -> Result<(), String> {
        self.push(Primitive::Text {
            at,
            angle,
            text: text.into(),
            size,
        })
    }
    pub(crate) fn observation(&self) -> Value {
        Value::Array(
            self.primitives
                .iter()
                .map(|p| match p {
                    Primitive::Line(v) => json!(["line", v[0], v[1], v[2], v[3]]),
                    Primitive::Arrow(v) => json!(["arrow", v[0], v[1], v[2], v[3]]),
                    Primitive::Poly(v) => json!(["poly", v]),
                    Primitive::Text {
                        at,
                        angle,
                        text,
                        size,
                    } => json!(["text", at[0], at[1], angle, text.as_ref(), size]),
                    Primitive::Section {
                        line,
                        direction,
                        label,
                        size,
                    } => json!([
                        "secmark",
                        line[0],
                        line[1],
                        line[2],
                        line[3],
                        direction[0],
                        direction[1],
                        label.as_ref(),
                        size
                    ]),
                })
                .collect(),
        )
    }
}
fn readable(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees() + 90.).rem_euclid(180.) - 90.
}
fn source_point(p: DVec3) -> DVec3 {
    p.as_vec3().as_dvec3()
}
fn text_size(frame: &ViewFrame) -> Result<f64, String> {
    let n = if frame.annot_text_mm == 0. {
        2.8
    } else {
        frame.annot_text_mm
    };
    if !n.is_finite() || n <= 0. || n > 10000. {
        Err("Invalid frame annotation text size".into())
    } else {
        Ok(n)
    }
}
/// `ground` follows the viewport's PhotoMesh-first, visible-terrain fallback.
/// `bounds` is the current visible, uncut source triangle extent, computed cold.
pub(crate) fn plan(
    scene: &Scene,
    frame: &ViewFrame,
    project: &Projector,
    mut ground: impl FnMut(DVec3) -> Result<DVec3, String>,
    bounds: Option<(DVec3, DVec3)>,
) -> Result<Plan, String> {
    let saved = frame
        .view_key
        .strip_prefix("scene:")
        .and_then(|name| scene.saved_views.iter().find(|s| s.name == name));
    let view = PaperSceneView::new(scene, saved);
    let mut out = Plan::default();
    for path in &scene.geo_paths {
        if path.points.len() < 2 {
            continue;
        }
        if path.points.len() > MAX_PRIMITIVES {
            return Err("Frame path vertex budget exceeded".into());
        }
        let mut points = Vec::new();
        points
            .try_reserve(path.points.len() + 1)
            .map_err(|_| "Frame path allocation refused")?;
        for p in &path.points {
            points.push(project.point(ground(source_point(*p))?)?);
        }
        if path.closed && points.len() > 2 {
            points.push(points[0]);
        }
        out.push(Primitive::Poly(points))?;
    }
    if frame.km_marks {
        chainages(&mut out, scene, frame, project, &mut ground)?;
    }
    if frame.section_marks {
        sections(&mut out, scene, frame, project, bounds)?;
    }
    if !frame.annotations {
        return Ok(out);
    }
    let size = text_size(frame)?;
    let style = scene.dimension_style();
    let units = style.get("units").and_then(Value::as_str).unwrap_or("m");
    let decimals = style.get("decimals").and_then(Value::as_i64).unwrap_or(2);
    let decimals = i32::try_from(decimals).map_err(|_| "Dimension precision exceeds domain")?;
    let ends = style
        .get("ends")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("arrow");
    for dim in &scene.dimensions {
        if !view.is_layer_visible(dim.layer.as_deref()) {
            continue;
        }
        let input = model::DimensionInput {
            a: dim.start.as_vec3(),
            b: dim.end.as_vec3(),
            offset: dim.offset.as_vec3(),
            axis: dim.axis.as_deref(),
            text: dim.text_override.as_deref(),
            highlighted: false,
            hidden: false,
        };
        let (ap, bp, value) = model::dimension_geometry(input);
        let a = project.point(input.a.as_dvec3())?;
        let b = project.point(input.b.as_dvec3())?;
        let a2 = project.point(ap.as_dvec3())?;
        let b2 = project.point(bp.as_dvec3())?;
        out.line(a, a2)?;
        out.line(b, b2)?;
        out.line(a2, b2)?;
        let angle = (b2[1] - a2[1]).atan2(b2[0] - a2[0]);
        for (p, heading) in [(a2, angle), (b2, angle + std::f64::consts::PI)] {
            if ends == "tick" {
                let a = angle + 45_f64.to_radians();
                let (dy, dx) = a.sin_cos();
                out.line(
                    [p[0] - 1.6 * dx, p[1] - 1.6 * dy],
                    [p[0] + 1.6 * dx, p[1] + 1.6 * dy],
                )?;
            } else if ends != "none" {
                out.push(Primitive::Arrow([p[0], p[1], heading.cos(), heading.sin()]))?;
            }
        }
        let measured = model::format_length(f64::from(value), units, decimals)?;
        let label = dim
            .text_override
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(measured.clone(), |s| s.replace("<>", &measured));
        out.text(
            [(a2[0] + b2[0]) / 2., (a2[1] + b2[1]) / 2.],
            readable(a2, b2),
            label,
            size,
        )?;
    }
    for label in &scene.text_labels {
        if !view.is_layer_visible(label.layer.as_deref()) {
            continue;
        }
        let a = project.point(source_point(label.anchor))?;
        let p = project.point((label.anchor.as_vec3() + label.offset.as_vec3()).as_dvec3())?;
        out.line(a, p)?;
        let mut lines = model::splitlines(&label.text, MAX_PRIMITIVES)?;
        if lines.is_empty() {
            lines.push("");
        }
        let side = if p[0] >= a[0] { 1. } else { -1. };
        for (i, line) in lines.into_iter().enumerate() {
            let width = (line.chars().count() as f64 * size * 0.55).max(1.);
            out.text(
                [
                    p[0] + side * (1.5 + width / 2.),
                    p[1] + i as f64 * size * 1.3,
                ],
                0.,
                line,
                size,
            )?;
        }
    }
    Ok(out)
}
fn chainage_step(length: f64, user: f64) -> f64 {
    if user > 0. {
        return user;
    }
    let raw = length / 6.;
    let mag = 10_f64.powf(raw.log10().floor());
    let step = [1., 2., 2.5, 5., 10.]
        .into_iter()
        .map(|m| m * mag)
        .find(|v| raw <= *v)
        .unwrap_or(length);
    // Source nice-ticks rounds each tick to six decimal places.
    let second = (step * 1e6).round_ties_even() / 1e6;
    if second > 0. && step <= length + 1e-6 {
        second
    } else {
        length
    }
}
fn chainages(
    out: &mut Plan,
    scene: &Scene,
    frame: &ViewFrame,
    p: &Projector,
    ground: &mut impl FnMut(DVec3) -> Result<DVec3, String>,
) -> Result<(), String> {
    let size = text_size(frame)?;
    for path in &scene.geo_paths {
        let nodes = path.profile_points();
        let length = nodes
            .windows(2)
            .map(|s| (s[1].x - s[0].x).hypot(s[1].y - s[0].y))
            .sum::<f64>();
        if nodes.len() < 2 || length <= 1e-9 {
            continue;
        }
        let step = chainage_step(length, frame.km_step_m);
        if !step.is_finite() || step <= 0. || length / step > MAX_PRIMITIVES as f64 {
            return Err("Frame chainage inventory exceeds budget".into());
        }
        let mut stations = Vec::new();
        let mut s = 0.;
        while s <= length + 1e-6 {
            stations.push(s.min(length));
            s += step;
        }
        if length - stations.last().copied().unwrap_or(0.) > 0.3 * step {
            stations.push(length);
        }
        for s in stations {
            let (x, y) = ingetrazo_core::georef::point_at_station(&nodes, s);
            let here = p.point(ground(source_point(DVec3::new(x, y, 0.)))?)?;
            let ahead = s + 0.5 <= length;
            let (x, y) = ingetrazo_core::georef::point_at_station(
                &nodes,
                if ahead { s + 0.5 } else { (s - 0.5).max(0.) },
            );
            let q = p.point(ground(source_point(DVec3::new(x, y, 0.)))?)?;
            let (dx, dy) = if ahead {
                (q[0] - here[0], q[1] - here[1])
            } else {
                (here[0] - q[0], here[1] - q[1])
            };
            let norm = dx.hypot(dy);
            let (dx, dy) = if norm < 1e-9 {
                (1., 0.)
            } else {
                (dx / norm, dy / norm)
            };
            let (nx, ny) = (-dy, dx);
            let tick = 0.9;
            out.line(
                [here[0] - nx * tick, here[1] - ny * tick],
                [here[0] + nx * tick, here[1] + ny * tick],
            )?;
            let km = (s / 1000.).floor() as u64;
            let m = s.rem_euclid(1000.);
            let text = if step >= 1. {
                format!("{km}+{m:03.0}")
            } else {
                format!("{km}+{m:06.2}")
            };
            let angle = (ny.atan2(nx).to_degrees() + 90.).rem_euclid(180.) - 90.;
            let width = (text.chars().count() as f64 * size * 0.55).max(1.);
            let off = tick + 0.6 + width / 2.;
            out.text([here[0] + nx * off, here[1] + ny * off], angle, text, size)?;
        }
    }
    Ok(())
}
fn sections(
    out: &mut Plan,
    scene: &Scene,
    frame: &ViewFrame,
    p: &Projector,
    bounds: Option<(DVec3, DVec3)>,
) -> Result<(), String> {
    let Some((lo, hi)) = bounds else {
        return Ok(());
    };
    let size = text_size(frame)?;
    let (mut low, mut high) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for x in [lo.x, hi.x] {
        for y in [lo.y, hi.y] {
            for z in [lo.z, hi.z] {
                let q = p.point(source_point(DVec3::new(x, y, z)))?;
                for i in 0..2 {
                    low[i] = low[i].min(q[i]);
                    high[i] = high[i].max(q[i]);
                }
            }
        }
    }
    let frame_low = [1., 1.];
    let frame_high = [frame.rect.w - 1., frame.rect.h - 1.];
    for i in 0..2 {
        low[i] = low[i].max(frame_low[i]);
        high[i] = high[i].min(frame_high[i]);
    }
    if high[0] <= low[0] || high[1] <= low[1] {
        return Ok(());
    }
    for (i, sp) in scene.section_planes.iter().enumerate() {
        let n = source_point(sp.normal);
        if n.dot(p.forward()).abs() > 0.999 {
            continue;
        }
        let d = n.cross(p.forward());
        let d = d / d.length().max(1e-12);
        let o = source_point(sp.point);
        let a = p.point(o)?;
        let b = p.point(source_point(o + d))?;
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        if length < 1e-9 {
            continue;
        }
        let direction = [dx / length, dy / length];
        let (mut t0, mut t1) = (-1e9_f64, 1e9_f64);
        for axis in 0..2 {
            let d = direction[axis];
            if d.abs() < 1e-12 {
                if a[axis] < low[axis] || a[axis] > high[axis] {
                    t0 = 1.;
                    t1 = 0.;
                    break;
                }
                continue;
            }
            let u = (low[axis] - a[axis]) / d;
            let v = (high[axis] - a[axis]) / d;
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
        }
        if t1 <= t0 {
            continue;
        }
        let (mut ta, mut tb) = (t0 - 6., t1 + 6.);
        for axis in 0..2 {
            let d = direction[axis];
            if d.abs() < 1e-12 {
                continue;
            }
            let u = (frame_low[axis] - a[axis]) / d;
            let v = (frame_high[axis] - a[axis]) / d;
            ta = ta.max(u.min(v));
            tb = tb.min(u.max(v));
        }
        let q = p.point(source_point(o - n))?;
        let ax = q[0] - a[0];
        let ay = q[1] - a[1];
        let len = ax.hypot(ay);
        if len < 1e-9 {
            continue;
        }
        let label = if sp.symbol.trim().is_empty() {
            char::from(b'A' + (i % 26) as u8).to_string()
        } else {
            sp.symbol.trim().into()
        };
        out.push(Primitive::Section {
            line: [
                a[0] + direction[0] * ta,
                a[1] + direction[1] * ta,
                a[0] + direction[0] * tb,
                a[1] + direction[1] * tb,
            ],
            direction: [ax / len, ay / len],
            label: label.into(),
            size,
        })?;
    }
    Ok(())
}
