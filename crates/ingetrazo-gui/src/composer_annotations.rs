//! Literal six-family physical paper annotations, shared by PNG/JPEG and PDF.
//! Only cold export prepares draws; imported metadata remains borrowed and immutable.
use crate::composer_text_fields::Context;
use ingetrazo_core::composition::{Sheet, SheetItem, TextItem};
use ingetrazo_formats::{
    paper_text_layout::Alignment,
    paper_text_painter::{Painter, TextStyle},
    sheet_pdf::{Draw, PathOp, StrokeOptions},
};
use serde_json::Value;
#[path = "composer_annotation_arc.rs"]
mod qt_arc;
#[path = "composer_annotations_symbols.rs"]
mod symbols;
use symbols::{callout, label, level};
#[path = "composer_frame_overlay_paint.rs"]
mod frame_overlay;
pub(crate) use frame_overlay::draws as frame_overlay_draws;
const PI: f64 = std::f64::consts::PI;
pub(crate) struct RectangleFill {pub index:usize,pub a:[f64;2],pub b:[f64;2],pub ratio:f64}
pub(crate) struct Draws {
    pub draws: Vec<Draw>,
    pub rectangles: Vec<usize>,
    pub rectangle_fills:Vec<RectangleFill>,
    pub source_lines: Vec<usize>,
    pub source_points: Vec<usize>,
}
struct Output {
    origin: [f64; 2],
    draws: Vec<Draw>,
    rectangle_fills:Vec<RectangleFill>,
    rectangles: Vec<usize>,
    lines: Vec<usize>,
    points: Vec<usize>,
}
impl Output {
    fn new(origin: [f64; 2]) -> Self {
        Self {
            origin,
            draws: vec![],
            rectangles: vec![],
            rectangle_fills:vec![],
            lines: vec![],
            points: vec![],
        }
    }
    fn path(
        &mut self,
        path: Vec<PathOp>,
        fill: Option<[f32; 4]>,
        stroke: Option<([f32; 4], f64)>,
        cap: u8,
        join: u8,
        dash: &[f64],
    ) {
        self.draws.push(Draw {
            picture: None,
            rotation_deg: 0.,
            origin_mm: self.origin,
            text: None,
            even_odd: false,
            path,
            stroke,
            stroke_options: stroke.map(|_| StrokeOptions {
                cap,
                join,
                dash: dash.to_vec(),
            }),
            fill,
            clip: None,
            clip_intersection: None,
            flatten_transform: false,
        });
    }
    fn line(&mut self, a: [f64; 2], b: [f64; 2], ink: [f32; 4], width: f64, cap: u8) {
        if a == b {
            self.points.push(self.draws.len());
        } else if cap == 2 {
            self.lines.push(self.draws.len());
        }
        self.path(
            vec![PathOp::Move(a), PathOp::Line(b)],
            None,
            Some((ink, width)),
            cap,
            2,
            &[],
        );
    }
    fn rectangle(&mut self, r: [f64; 4], fill: [f32; 4]) {
        self.rectangles.push(self.draws.len());
        self.path(rect(r), Some(fill), None, 2, 2, &[]);
    }
    fn polygon(
        &mut self,
        points: &[[f64; 2]],
        fill: Option<[f32; 4]>,
        stroke: Option<([f32; 4], f64)>,
        join: u8,
    ) {
        let mut path = vec![PathOp::Move(points[0])];
        path.extend(points[1..].iter().copied().map(PathOp::Line));
        path.push(PathOp::Close);
        self.path(path, fill, stroke, 2, join, &[]);
    }
    fn finish(self) -> Result<Draws, String> {
        if self.draws.len() > ingetrazo_formats::sheet_pdf::MAX_DRAWS {
            return Err("Annotation draw inventory exceeds budget".into());
        }
        let commands = self
            .draws
            .iter()
            .try_fold(0usize, |n, d| {
                n.checked_add(
                    d.path.len()
                        + d.clip.as_ref().map_or(0, Vec::len)
                        + d.clip_intersection.as_ref().map_or(0, Vec::len)
                        + d.text.as_ref().map_or(0, |t| t.glyphs.len()),
                )
            })
            .ok_or("Annotation command budget overflow")?;
        if commands > ingetrazo_formats::sheet_pdf::MAX_COMMANDS {
            return Err("Annotation command budget exceeded".into());
        }
        Ok(Draws {
            draws: self.draws,
            rectangles: self.rectangles,
            rectangle_fills:self.rectangle_fills,
            source_lines: self.lines,
            source_points: self.points,
        })
    }
}
fn n(v: &Value, k: &str, d: f64) -> Result<f64, String> {
    let q = v
        .get(k)
        .map_or(Some(d), Value::as_f64)
        .filter(|q| q.is_finite() && q.abs() <= 1e6)
        .ok_or_else(|| format!("Invalid annotation {k}"))?;
    Ok(q)
}
fn b(v: &Value, k: &str, d: bool) -> Result<bool, String> {
    v.get(k).map_or(Ok(d), |q| {
        q.as_bool().ok_or_else(|| format!("Invalid annotation {k}"))
    })
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> Result<&'a str, String> {
    let q = v
        .get(k)
        .map_or(Some(d), Value::as_str)
        .filter(|q| q.len() <= 1024 * 1024)
        .ok_or_else(|| format!("Invalid annotation {k}"))?;
    Ok(q)
}
fn option<'a>(v: &'a Value, k: &str, d: &'a str) -> Result<&'a str, String> {
    let q = s(v, k, d)?;
    Ok(if q.is_empty() { d } else { q })
}
fn color(s: &str) -> Result<[f32; 4], String> {
    Ok(ingetrazo_core::sheet_color::parse_sheet_color(s)?.map(|q| q as f32 / 255.))
}
fn opacity(mut c: [f32; 4], a: f64) -> [f32; 4] {
    c[3] = ((a.clamp(0., 1.) * 65535.).round() / 65535.) as f32;
    c
}
fn rect(r: [f64; 4]) -> Vec<PathOp> {
    let [x, y, w, h] = r;
    vec![
        PathOp::Move([x, y]),
        PathOp::Line([x + w, y]),
        PathOp::Line([x + w, y + h]),
        PathOp::Line([x, y + h]),
        PathOp::Close,
    ]
}
fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Vec<PathOp> {
    // QPainter drawEllipse(center,rx,ry) first forms QRectF(center-radius,2radius).
    // Retain the qt_curves_for_arc positive-box operation order.
    let (x, y, w, h) = (cx - rx, cy - ry, 2. * rx, 2. * ry);
    let (w2, h2) = (w / 2., h / 2.);
    let (wk, hk) = (w2 * 0.5522847498, h2 * 0.5522847498);
    vec![
        PathOp::Move([x + w, y + h2]),
        PathOp::Cubic([x + w, y + h2 + hk, x + w2 + wk, y + h, x + w2, y + h]),
        PathOp::Cubic([x + w2 - wk, y + h, x, y + h2 + hk, x, y + h2]),
        PathOp::Cubic([x, y + h2 - hk, x + w2 - wk, y, x + w2, y]),
        PathOp::Cubic([x + w2 + wk, y, x + w, y + h2 - hk, x + w, y + h2]),
        PathOp::Close,
    ]
}

fn style<'a>(
    text: &'a str,
    r: [f64; 4],
    size: f64,
    bold: bool,
    italic: bool,
    underline: bool,
    alignment: Alignment,
    ink: [f32; 4],
) -> TextStyle<'a> {
    TextStyle {
        text,
        origin_mm: [r[0], r[1]],
        width_mm: r[2],
        size_pt: size * 72. / 25.4,
        family: "Sans Serif",
        bold,
        italic,
        underline,
        alignment,
        color: ink,
    }
}
fn map(p: [f64; 2], offset: [f64; 2], angle: f64) -> [f64; 2] {
    let (sn, cs) = angle.to_radians().sin_cos();
    [
        offset[0] + p[0] * cs - p[1] * sn,
        offset[1] + p[0] * sn + p[1] * cs,
    ]
}
fn map_path(path: &mut [PathOp], offset: [f64; 2], angle: f64) {
    for p in path {
        match p {
            PathOp::Move(p) | PathOp::Line(p) => *p = map(*p, offset, angle),
            PathOp::Cubic(p) => {
                for i in (0..6).step_by(2) {
                    let q = map([p[i], p[i + 1]], offset, angle);
                    p[i] = q[0];
                    p[i + 1] = q[1];
                }
            }
            PathOp::Close => {}
        }
    }
}
fn text(
    o: &mut Output,
    p: &mut Painter,
    t: &str,
    r: [f64; 4],
    size: f64,
    bold: bool,
    italic: bool,
    underline: bool,
    align: Alignment,
    vertical: u8,
    ink: [f32; 4],
    offset: [f64; 2],
    angle: f64,
) -> Result<(), String> {
    if t.is_empty() {
        return Ok(());
    }
    let mut draws = p.draw_box(
        &style(t, r, size, bold, italic, underline, align, ink),
        r[3],
        vertical,
    )?;
    for d in &mut draws {
        d.origin_mm = [o.origin[0] + offset[0], o.origin[1] + offset[1]];
        if let Some(t) = &mut d.text {
            t.rotation_deg = angle;
        }
        if angle != 0. {
            for path in [&mut d.clip, &mut d.clip_intersection]
                .into_iter()
                .flatten()
            {
                map_path(path, [0.; 2], angle);
            }
        }
    }
    o.draws.extend(draws);
    Ok(())
}
fn rotated_rectangle(
    o: &mut Output,
    r: [f64; 4],
    offset: [f64; 2],
    angle: f64,
    ink: [f32; 4],
    pdf: bool,
) {
    if angle == 0. {
        o.rectangle([offset[0] + r[0], offset[1] + r[1], r[2], r[3]], ink);
    } else {
        let mut path = rect(r);
        if !pdf {
            // Actual drawRect retains RectangleHint under uniform rotation.
            // Qt maps the original left/right midpoints then passes h/w.
            let middle=(r[1]+(r[1]+r[3]))*0.5;
            o.rectangle_fills.push(RectangleFill{index:o.draws.len(),a:map([r[0],middle],offset,angle),b:map([r[0]+r[2],middle],offset,angle),ratio:r[3]/r[2]});
            map_path(&mut path, offset, angle);
        }
        o.path(path, Some(ink), None, 2, 2, &[]);
        if pdf {
            let d = o.draws.last_mut().unwrap();
            d.origin_mm = [o.origin[0] + offset[0], o.origin[1] + offset[1]];
            d.rotation_deg = angle;
        }
    }
}
fn baseline(p: &mut Painter, size: f64, y: f64) -> Result<[f64; 4], String> {
    let m = p.font_metrics_mm(&style(
        "",
        [0., 0., 80., size],
        size,
        false,
        false,
        false,
        Alignment::Center,
        [0., 0., 0., 1.],
    ))?;
    Ok([-40., y - m[0], 80., m[0] + m[1]])
}
fn readable(x: f64, y: f64) -> f64 {
    (y.atan2(x).to_degrees() + 90.).rem_euclid(180.) - 90.
}
fn length(v: &Value, mm: f64, radial: Option<&str>) -> Result<String, String> {
    let distance = mm * n(v, "scale_n", 100.)? / 1000.;
    let decimals = n(v, "decimals", 2.)?.trunc().clamp(0., 6.) as i32;
    let unit = option(v, "units", "m")?;
    let value = if unit == "m" && distance >= 1000. {
        format!("{:.3} km", distance / 1000.)
    } else {
        ingetrazo_render::annotations::format_length(distance, unit, decimals)?
    };
    let auto = format!("{}{value}", radial.unwrap_or(""));
    let t = s(v, "text", "")?;
    Ok(if t.is_empty() {
        auto
    } else {
        t.replace("<>", &auto)
    })
}
fn arrows(
    o: &mut Output,
    ends: &str,
    tips: &[([f64; 2], f64)],
    stroke: f64,
    ink: [f32; 4],
    cap: u8,
) {
    if ends == "arrow" {
        let len = (stroke * 6.).max(1.8);
        let base = 12_f64.to_radians();
        for &(pt, d) in tips {
            o.polygon(
                &[
                    pt,
                    [
                        pt[0] + len * (d + base).cos(),
                        pt[1] + len * (d + base).sin(),
                    ],
                    [
                        pt[0] + len * (d - base).cos(),
                        pt[1] + len * (d - base).sin(),
                    ],
                ],
                Some(ink),
                None,
                2,
            );
        }
    } else if ends != "none" {
        for &(pt, a) in tips {
            let d = a + 45_f64.to_radians();
            o.line(
                [pt[0] - 1.6 * d.cos(), pt[1] - 1.6 * d.sin()],
                [pt[0] + 1.6 * d.cos(), pt[1] + 1.6 * d.sin()],
                ink,
                stroke,
                cap,
            );
        }
    }
}
fn expanded(sheet: &Sheet, t: &str, context: &Context<'_>, uid: &str) -> Result<String, String> {
    let frame = if uid.is_empty() {
        None
    } else {
        sheet.items.iter().find_map(|i| {
            if let SheetItem::ViewFrame(f) = i {
                (sheet
                    .native_items
                    .get(&f.id)
                    .and_then(|r| r.get("uid"))
                    .and_then(Value::as_str)
                    == Some(uid))
                .then_some(f)
            } else {
                None
            }
        })
    };
    let item = TextItem {
        id: uuid::Uuid::nil(),
        pos: (0., 0.),
        text: t.into(),
        font_size_pt: 10.,
        bold: false,
    };
    crate::composer_text_fields::expand_bound(sheet, &item, context, frame)
}
pub(crate) fn draws(
    sheet: &Sheet,
    key: &str,
    v: &Value,
    p: &mut Painter,
    context: &Context<'_>,
    pdf: bool,
) -> Result<Draws, String> {
    if !v.is_object() {
        return Err("Invalid physical annotation record".into());
    }
    let origin = [
        n(
            v,
            "x_mm",
            if matches!(key, "cotas" | "llamadas") {
                20.
            } else {
                0.
            },
        )?,
        n(
            v,
            "y_mm",
            if matches!(key, "cotas" | "llamadas") {
                20.
            } else {
                0.
            },
        )?,
    ];
    let mut out = Output::new(origin);
    match key {
        "cotas" => cota(&mut out, v, p, pdf)?,
        "cotas_ang" => angular(&mut out, v, p)?,
        "cotas_rad" => radial(&mut out, v, p, pdf)?,
        "etiquetas" => label(&mut out, sheet, v, p, context)?,
        "niveles" => level(&mut out, v, p)?,
        "llamadas" => callout(&mut out, sheet, v, p, context, pdf)?,
        _ => return Err("Unknown physical annotation family".into()),
    };
    out.finish()
}

fn cota(o: &mut Output, v: &Value, p: &mut Painter, pdf: bool) -> Result<(), String> {
    let (dx, dy) = (n(v, "dx_mm", 40.)?, n(v, "dy_mm", 0.)?);
    let sep = n(v, "sep_mm", 0.)?;
    let axis = s(v, "axis", "")?;
    let measured = match axis {
        "h" => dx.abs(),
        "v" => dy.abs(),
        _ => dx.hypot(dy),
    };
    let ln = dx.hypot(dy);
    let normal = if ln < 1e-9 {
        [0., -1.]
    } else {
        [-dy / ln, dx / ln]
    };
    let (a, b) = match axis {
        "h" => ([0., sep], [dx, sep]),
        "v" => ([sep, 0.], [sep, dy]),
        _ => (
            [normal[0] * sep, normal[1] * sep],
            [dx + normal[0] * sep, dy + normal[1] * sep],
        ),
    };
    let ink = color(s(v, "color", "#1e242c")?)?;
    let stroke = n(v, "stroke_mm", 0.25)?;
    if stroke < 0. {
        return Err("Invalid negative annotation stroke".into());
    }
    for (point, foot) in [([0., 0.], a), ([dx, dy], b)] {
        let (ex, ey) = (foot[0] - point[0], foot[1] - point[1]);
        let len = ex.hypot(ey);
        if len > 0.05 {
            let (ux, uy) = (ex / len, ey / len);
            o.line(
                [point[0] + ux, point[1] + uy],
                [foot[0] + ux * 1.2, foot[1] + uy * 1.2],
                ink,
                stroke,
                2,
            );
        }
    }
    let label = length(v, measured, None)?;
    let size = n(v, "text_mm", 2.8)?;
    let gap = n(v, "offset_mm", 4.)?;
    let width = label.chars().count() as f64 * size * 0.62 + 2.;
    let height = size * 1.3 + 0.8;
    let pos = option(v, "text_pos", "above")?;
    let along = option(v, "text_along", "middle")?;
    let horizontal = option(v, "text_align", "aligned")? == "horizontal";
    let drag = [n(v, "text_dx_mm", 0.)?, n(v, "text_dy_mm", 0.)?];
    let deg = readable(b[0] - a[0], b[1] - a[1]);
    let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
    let extent = if horizontal {
        let d = deg.to_radians();
        width * d.cos().abs() + height * d.sin().abs()
    } else {
        width
    };
    let line_length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let unit = if line_length < 1e-9 {
        [0., 0.]
    } else {
        [(b[0] - a[0]) / line_length, (b[1] - a[1]) / line_length]
    };
    if matches!(along, "start" | "end")
        && drag.iter().all(|q| q.abs() <= 1e-9)
        && line_length >= 1e-9
    {
        let reach = extent + 1.8;
        if along == "start" {
            o.line(
                a,
                [a[0] - unit[0] * reach, a[1] - unit[1] * reach],
                ink,
                stroke,
                2,
            );
        } else {
            o.line(
                b,
                [b[0] + unit[0] * reach, b[1] + unit[1] * reach],
                ink,
                stroke,
                2,
            );
        }
    }
    let mid = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.];
    if pos == "centered" && along == "middle" && drag.iter().all(|q| q.abs() < 1e-9) {
        let (ux, uy) = (angle.cos(), angle.sin());
        let th = size * 1.3 + 0.8;
        let ext = if horizontal {
            (width * ux).abs() + (th * uy).abs()
        } else {
            width
        };
        let half = ext / 2. + 0.5;
        if 2. * half < ln - 2. {
            o.line(a, [mid[0] - ux * half, mid[1] - uy * half], ink, stroke, 2);
            o.line([mid[0] + ux * half, mid[1] + uy * half], b, ink, stroke, 2);
        } else {
            o.line(a, b, ink, stroke, 2);
        }
    } else {
        o.line(a, b, ink, stroke, 2);
    }
    arrows(
        o,
        s(v, "ends", "tick")?,
        &[
            (a, angle),
            (
                b,
                if s(v, "ends", "tick")? == "arrow" {
                    angle + PI
                } else {
                    angle
                },
            ),
        ],
        stroke,
        ink,
        2,
    );
    let mut anchor = mid;
    if matches!(along, "start" | "end") && line_length > 1e-9 {
        let mut shift = line_length / 2. + extent / 2. + 1.;
        if along == "start" {
            shift = -shift;
        }
        anchor = [anchor[0] + unit[0] * shift, anchor[1] + unit[1] * shift];
    }
    anchor = [anchor[0] + drag[0], anchor[1] + drag[1]];
    let (mut rotation, mut offset) = (if horizontal { 0. } else { deg }, anchor);
    let (r, vertical) = match pos {
        "aside" | "aside_below" => {
            let d = deg.to_radians();
            let (mut ux, mut uy) = (d.sin(), -d.cos());
            if pos == "aside_below" {
                ux = -ux;
                uy = -uy;
            }
            let half = if horizontal {
                width / 2. * ux.abs() + height / 2. * uy.abs()
            } else {
                height / 2.
            };
            let dist = gap + half;
            offset = [anchor[0] + ux * dist, anchor[1] + uy * dist];
            rotation = if horizontal { 0. } else { deg };
            ([-40., -height / 2., 80., height], 1)
        }
        "below" => ([-40., gap, 80., size * 1.3], 0),
        "centered" => ([-40., -size * 0.65, 80., size * 1.3], 1),
        _ => (baseline(p, size, -gap)?, 0),
    };
    let tcol = if s(v, "text_color", "")?.is_empty() {
        ink
    } else {
        color(s(v, "text_color", "")?)?
    };
    let bg = s(v, "text_bg", "")?;
    if !bg.is_empty() && !label.is_empty() {
        rotated_rectangle(
            o,
            [-width / 2., r[1] - 0.4, width, r[3] + 0.8],
            offset,
            rotation,
            opacity(color(bg)?, n(v, "text_bg_opacity", 1.)?),
            pdf,
        );
    }
    text(
        o,
        p,
        &label,
        r,
        size,
        false,
        false,
        false,
        Alignment::Center,
        vertical,
        tcol,
        offset,
        rotation,
    )
}
fn angular(o: &mut Output, v: &Value, p: &mut Painter) -> Result<(), String> {
    let (a, b) = (
        [n(v, "ax_mm", 30.)?, n(v, "ay_mm", 0.)?],
        [n(v, "bx_mm", 0.)?, n(v, "by_mm", -30.)?],
    );
    let a0 = a[1].atan2(a[0]);
    let mut sweep = b[1].atan2(b[0]) - a0;
    while sweep > PI {
        sweep -= 2. * PI;
    }
    while sweep <= -PI {
        sweep += 2. * PI;
    }
    let r = n(v, "radius_mm", 15.)?.max(1.);
    let ink = color(s(v, "color", "#1e242c")?)?;
    let stroke = n(v, "stroke_mm", 0.25)?;
    for point in [a, b] {
        let l = point[0].hypot(point[1]);
        if l < 1e-9 {
            continue;
        }
        let reach = l.max(r + 1.2);
        o.line(
            [0., 0.],
            [point[0] / l * reach, point[1] / l * reach],
            ink,
            stroke,
            1,
        );
    }
    o.path(
        qt_arc::arc(
            [-r, -r, 2. * r, 2. * r],
            -a0.to_degrees(),
            -sweep.to_degrees(),
        )?,
        None,
        Some((ink, stroke)),
        1,
        2,
        &[],
    );
    let sign = if sweep >= 0. { 1. } else { -1. };
    let ends = s(v, "ends", "arrow")?;
    for (angle, d) in [(a0, sign), (a0 + sweep, -sign)] {
        let tip = [r * angle.cos(), r * angle.sin()];
        if ends == "arrow" {
            let heading = (d * angle.cos()).atan2(-d * angle.sin());
            arrows(o, ends, &[(tip, heading)], stroke, ink, 1);
        } else {
            arrows(o, ends, &[(tip, angle)], stroke, ink, 1);
        }
    }
    let decimals = n(v, "decimals", 1.)?.trunc().clamp(0., 4.) as usize;
    let auto = format!("{:.*}°", decimals, sweep.to_degrees().abs());
    let custom = s(v, "text", "")?;
    let label = if custom.is_empty() {
        auto
    } else {
        custom.replace("<>", &auto)
    };
    let size = n(v, "text_mm", 2.8)?;
    let middle = a0 + sweep / 2.;
    let d = r + n(v, "offset_mm", 0.8)? + size * 0.75;
    let (lx, ly) = (d * middle.cos(), d * middle.sin());
    let tcol = if s(v, "text_color", "")?.is_empty() {
        ink
    } else {
        color(s(v, "text_color", "")?)?
    };
    let bg = s(v, "text_bg", "")?;
    if !bg.is_empty() && !label.is_empty() {
        let w = label.chars().count() as f64 * size * 0.62 + 2.;
        o.rectangle(
            [lx - w / 2., ly - size * 0.75, w, size * 1.5],
            opacity(color(bg)?, n(v, "text_bg_opacity", 1.)?),
        );
    }
    text(
        o,
        p,
        &label,
        [lx - 20., ly - size * 0.65, 40., size * 1.3],
        size,
        false,
        false,
        false,
        Alignment::Center,
        1,
        tcol,
        [0.; 2],
        0.,
    )
}
fn radial(o: &mut Output, v: &Value, p: &mut Painter, pdf: bool) -> Result<(), String> {
    let radius = n(v, "radius_mm", 15.)?;
    let angle = n(v, "angle_deg", -30.)?.to_radians();
    let (u, w) = (angle.cos(), angle.sin());
    let diameter = s(v, "kind", "radius")? == "diameter";
    let a = if diameter {
        [-u * radius, -w * radius]
    } else {
        [0., 0.]
    };
    let b = [u * radius, w * radius];
    let ink = color(s(v, "color", "#1e242c")?)?;
    let stroke = n(v, "stroke_mm", 0.25)?;
    let size = n(v, "text_mm", 2.8)?;
    let label = length(
        v,
        if diameter {
            2. * radius.abs()
        } else {
            radius.abs()
        },
        Some(if diameter { "Ø" } else { "R" }),
    )?;
    let width = label.chars().count() as f64 * size * 0.62 + 2.;
    let placement = s(v, "placement", "auto")?;
    let out = match placement {
        "in" => false,
        "out" => true,
        _ => {
            width + (stroke * 6.).max(1.8) * if diameter { 2. } else { 1. } + 1.
                > (b[0] - a[0]).hypot(b[1] - a[1])
        }
    };
    o.line(a, b, ink, stroke, 1);
    if out {
        let reach = radius + width + 2.5;
        o.line(b, [u * reach, w * reach], ink, stroke, 1);
    }
    if self::b(v, "centre_mark", true)? {
        let m = (size * 0.45).max(1.);
        o.line([-m, 0.], [m, 0.], ink, stroke, 1);
        o.line([0., -m], [0., m], ink, stroke, 1);
    }
    let ends = s(v, "ends", "arrow")?;
    let mut tips = vec![(b, if out { [-u, -w] } else { [u, w] })];
    if diameter {
        tips.push((a, if out { [u, w] } else { [-u, -w] }));
    }
    for (tip, heading) in tips {
        let a = if ends == "arrow" {
            heading[1].atan2(heading[0]) + PI
        } else {
            w.atan2(u)
        };
        arrows(o, ends, &[(tip, a)], stroke, ink, 1);
    }
    let deg = readable(u, w);
    let anchor = if out {
        let reach = radius + width / 2. + 2.;
        [u * reach, w * reach]
    } else if diameter {
        let d = deg.to_radians();
        [d.cos() * (radius / 2.), d.sin() * (radius / 2.)]
    } else {
        [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2.]
    };
    let r = baseline(p, size, -n(v, "offset_mm", 0.5)?)?;
    let tcol = if s(v, "text_color", "")?.is_empty() {
        ink
    } else {
        color(s(v, "text_color", "")?)?
    };
    let bg = s(v, "text_bg", "")?;
    if !bg.is_empty() && !label.is_empty() {
        rotated_rectangle(
            o,
            [-width / 2., r[1] - 0.4, width, r[3] + 0.8],
            anchor,
            deg,
            opacity(color(bg)?, n(v, "text_bg_opacity", 1.)?),
            pdf,
        );
    }
    text(
        o,
        p,
        &label,
        r,
        size,
        false,
        false,
        false,
        Alignment::Center,
        0,
        tcol,
        anchor,
        deg,
    )
}

/// Source canvas item sizes used by _SheetItem.itemChange snapping. They differ
/// from the painted/control hull and from the legacy label dataclass height.
pub(crate) fn authoring_size(
    sheet: &Sheet,
    key: &str,
    v: &Value,
    p: &mut Painter,
    context: &Context<'_>,
) -> Result<Option<[f64; 2]>, String> {
    if key == "etiquetas" {
        let content = expanded(sheet, s(v, "text", "Texto")?, context, "")?;
        let sz = n(v, "size_pt", 11.)? * 25.4 / 72.;
        let width = n(v, "w_mm", 50.)?;
        let bold = b(v, "bold", false)?;
        let style = style(
            &content,
            [0., 0., width.max(0.5), 1_000_000.],
            sz,
            bold,
            false,
            false,
            Alignment::Left,
            [0., 0., 0., 1.],
        );
        let [asc, desc, leading, _] = p.font_metrics_mm(&style)?;
        let line = asc + desc;
        let count = if content.is_empty() {
            0.
        } else {
            (p.measure_box(&style)?[1] / line.max(1e-6))
                .round_ties_even()
                .max(1.)
        };
        let laid = if count == 0. {
            0.
        } else {
            line + (count - 1.) * (line + leading)
        };
        return Ok(Some([width, (sz * 1.15).max(laid)]));
    }
    if key == "niveles" {
        let r = authoring_level_bounds(v, p)?;
        return Ok(Some([r[2], r[3]]));
    }
    Ok(None)
}
pub(crate) fn authoring_level_bounds(v: &Value, p: &mut Painter) -> Result<[f64; 4], String> {
    let sz = n(v, "size_mm", 2.5)?;
    let symbol = (sz * 1.2).max(1.5);
    let content = symbols::level_label(v)?;
    let advance = p.measure_advance(&style(
        &content,
        [0., 0., 80., sz],
        sz,
        false,
        false,
        false,
        Alignment::Left,
        [0., 0., 0., 1.],
    ))?;
    let line = n(v, "line_mm", 14.)?.max(symbol / 2. + 0.6 + advance + 1.);
    let sign = if b(v, "mirror", false)? { -1. } else { 1. };
    let ax = n(v, "ax_mm", 0.)?;
    let ay = n(v, "ay_mm", 0.)?;
    let x0 = (-symbol / 2.).min(sign * (line + 0.5)).min(ax).min(0.);
    let x1 = (symbol / 2.).max(sign * (line + 0.5)).max(ax).max(0.);
    let circle = s(v, "symbol", "triangle")? == "circle";
    let y0 = (if circle {
        -symbol / 2. - sz * 1.4
    } else {
        -symbol - sz * 1.4
    })
    .min(ay);
    let y1 = (if circle { symbol / 2. } else { 0.5 }).max(ay);
    Ok([x0 - 0.5, y0 - 0.5, x1 - x0 + 1., y1 - y0 + 1.])
}

/// Cold controller measurement; shares physical label formatting and source metrics.
pub(crate) fn authoring_linear_label(v:&Value)->Result<String,String>{let dx=n(v,"dx_mm",40.)?;let dy=n(v,"dy_mm",0.)?;length(v,match s(v,"axis","")?{"h"=>dx.abs(),"v"=>dy.abs(),_=>dx.hypot(dy)},None)}
pub(crate) fn authoring_label_ink_width(sheet:&Sheet,v:&Value,p:&mut Painter,context:&Context<'_>)->Result<f64,String>{
 let content=expanded(sheet,s(v,"text","Texto")?,context,"")?;let size=n(v,"size_pt",11.)?*25.4/72.;let mut widest=0_f64;
 for line in content.split('\n'){widest=widest.max(p.measure_advance(&style(line,[0.,0.,1_000_000.,1_000_000.],size,b(v,"bold",false)?,false,false,Alignment::Left,[0.,0.,0.,1.]))?);}
 Ok(2_f64.max(n(v,"w_mm",50.)?.min(widest+0.2)))
}

pub(crate) fn authoring_radial_label(v:&Value)->Result<String,String>{let r=n(v,"radius_mm",15.)?.abs();let diameter=s(v,"kind","radius")?=="diameter";length(v,if diameter{2.*r}else{r},Some(if diameter{"Ø"}else{"R"}))}
