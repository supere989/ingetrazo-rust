// SPDX-License-Identifier: GPL-3.0-or-later
//! Physical source ViewFrame labels. Model clipping never clips outside captions.
use ingetrazo_core::composition::{Sheet, TextItem, ViewFrame};
use ingetrazo_formats::{
    paper_text_layout::Alignment,
    paper_text_painter::{Painter, TextStyle},
    sheet_pdf::{Draw, PathOp, StrokeOptions},
};
const INK: [f32; 4] = [30. / 255., 36. / 255., 44. / 255., 1.];
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<PathOp> {
    vec![
        PathOp::Move([x, y]),
        PathOp::Line([x + w, y]),
        PathOp::Line([x + w, y + h]),
        PathOp::Line([x, y + h]),
        PathOp::Close,
    ]
}
fn path(
    origin: [f64; 2],
    ops: Vec<PathOp>,
    fill: Option<[f32; 4]>,
    stroke: Option<([f32; 4], f64)>,
) -> Draw {
    Draw {
        origin_mm: origin,
        path: ops,
        fill,
        stroke,
        stroke_options: stroke.map(|_| StrokeOptions {
            cap: 2,
            join: 2,
            dash: vec![],
        }),
        clip: None,
        clip_intersection: None,
        text: None,
        flatten_transform: false,
        even_odd: false,
        rotation_deg: 0.,
    }
}
fn string<'a>(
    raw: Option<&'a serde_json::Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, String> {
    match raw.and_then(|v| v.get(key)) {
        None => Ok(default),
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("Invalid view caption {key}")),
    }
}
fn option<'a>(
    raw: Option<&'a serde_json::Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, String> {
    let value = string(raw, key, default)?;
    Ok(if value.is_empty() { default } else { value })
}
fn number(raw: Option<&serde_json::Value>, key: &str, default: f64) -> Result<f64, String> {
    let v = match raw.and_then(|v| v.get(key)) {
        None => default,
        Some(v) => v
            .as_f64()
            .ok_or_else(|| format!("Invalid view caption {key}"))?,
    };
    if !v.is_finite() || v.abs() > 1_000_000. {
        return Err(format!("Invalid view caption {key}"));
    }
    Ok(if v == 0. { default } else { v })
}
fn boolean(raw: Option<&serde_json::Value>, key: &str, default: bool) -> Result<bool, String> {
    match raw.and_then(|v| v.get(key)) {
        None => Ok(default),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("Invalid view caption {key}")),
    }
}
fn style<'a>(
    text: &'a str,
    r: [f64; 4],
    size: f64,
    bold: bool,
    align: Alignment,
    ink: [f32; 4],
) -> TextStyle<'a> {
    TextStyle {
        text,
        origin_mm: [r[0], r[1]],
        width_mm: r[2],
        size_pt: size * 72. / 25.4,
        family: "Sans Serif",
        bold,
        italic: false,
        underline: false,
        alignment: align,
        color: ink,
    }
}
fn text(
    p: &mut Painter,
    s: &str,
    origin: [f64; 2],
    r: [f64; 4],
    size: f64,
    bold: bool,
    align: Alignment,
    vertical: u8,
    ink: [f32; 4],
    angle: f64,
) -> Result<Vec<Draw>, String> {
    if s.is_empty() {
        return Ok(vec![]);
    }
    let mut out = p.draw_box(&style(s, r, size, bold, align, ink), r[3], vertical)?;
    let (sin, cos) = angle.to_radians().sin_cos();
    for d in &mut out {
        d.origin_mm = origin;
        if let Some(t) = &mut d.text {
            t.rotation_deg = angle;
        }
        if angle != 0. {
            for clip in [&mut d.clip, &mut d.clip_intersection]
                .into_iter()
                .flatten()
            {
                for op in clip {
                    match op {
                        PathOp::Move(q) | PathOp::Line(q) => {
                            *q = [q[0] * cos - q[1] * sin, q[0] * sin + q[1] * cos]
                        }
                        PathOp::Cubic(v) => {
                            for i in [0, 2, 4] {
                                let (x, y) = (v[i], v[i + 1]);
                                v[i] = x * cos - y * sin;
                                v[i + 1] = x * sin + y * cos;
                            }
                        }
                        PathOp::Close => {}
                    }
                }
            }
        }
    }
    Ok(out)
}
fn bubble(
    p: &mut Painter,
    out: &mut Vec<Draw>,
    lines: &mut Vec<usize>,
    origin: [f64; 2],
    cx: f64,
    cy: f64,
    d: f64,
    number: &str,
    sheet: &str,
) -> Result<(), String> {
    let r = d / 2.;
    let k = r * 0.5522847498;
    out.push(path(
        origin,
        vec![
            PathOp::Move([cx + r, cy]),
            PathOp::Cubic([cx + r, cy + k, cx + k, cy + r, cx, cy + r]),
            PathOp::Cubic([cx - k, cy + r, cx - r, cy + k, cx - r, cy]),
            PathOp::Cubic([cx - r, cy - k, cx - k, cy - r, cx, cy - r]),
            PathOp::Cubic([cx + k, cy - r, cx + r, cy - k, cx + r, cy]),
            PathOp::Close,
        ],
        Some([1.; 4]),
        Some((INK, 0.35)),
    ));
    if !sheet.is_empty() {
        lines.push(out.len());
        out.push(path(
            origin,
            vec![PathOp::Move([cx - r, cy]), PathOp::Line([cx + r, cy])],
            None,
            Some((INK, 0.35)),
        ));
        out.extend(text(
            p,
            number,
            origin,
            [cx - r, cy - r, d, r],
            d * 0.34,
            true,
            Alignment::Center,
            1,
            INK,
            0.,
        )?);
        out.extend(text(
            p,
            sheet,
            origin,
            [cx - r, cy, d, r],
            d * 0.26,
            false,
            Alignment::Center,
            1,
            INK,
            0.,
        )?);
    } else {
        out.extend(text(
            p,
            number,
            origin,
            [cx - r, cy - r, d, d],
            d * 0.42,
            true,
            Alignment::Center,
            1,
            INK,
            0.,
        )?);
    }
    Ok(())
}
fn view_name(frame: &ViewFrame) -> String {
    if let Some(name) = frame.view_key.strip_prefix("scene:") {
        name.into()
    } else if let Some(key) = frame.view_key.strip_prefix("std:") {
        ingetrazo_core::i18n::tr(match key {
            "top" => "Top (plan)",
            "front" => "Front",
            "back" => "Back",
            "left" => "Left",
            "right" => "Right",
            "iso" => "Isometric",
            other => other,
        })
    } else {
        ingetrazo_core::i18n::tr("View")
    }
}
fn expand(
    sheet: &Sheet,
    frame: &ViewFrame,
    s: &str,
    ctx: &crate::composer_text_fields::Context<'_>,
) -> Result<String, String> {
    if s.len() > ingetrazo_formats::paper_text_layout::MAX_LINE_BYTES {
        return Err("View caption input exceeds byte budget".into());
    }
    let mut authored = String::new();
    authored
        .try_reserve_exact(s.len())
        .map_err(|_| "View caption allocation refused")?;
    authored.push_str(s);
    crate::composer_text_fields::expand_bound(
        sheet,
        &TextItem {
            id: frame.id,
            pos: (0., 0.),
            text: authored,
            font_size_pt: 10.,
            bold: false,
        },
        ctx,
        Some(frame),
    )
}
pub(crate) fn placeholder(frame: &ViewFrame, p: &mut Painter) -> Result<Vec<Draw>, String> {
    text(
        p,
        &ingetrazo_core::i18n::tr("Update the view to render"),
        [frame.rect.x, frame.rect.y],
        [2., 2., frame.rect.w - 4., frame.rect.h - 4.],
        3.5,
        false,
        Alignment::Left,
        0,
        [140. / 255., 150. / 255., 160. / 255., 1.],
        0.,
    )
}
pub(crate) struct CaptionDraws {
    pub(crate) draws: Vec<Draw>,
    pub(crate) source_lines: Vec<usize>,
}
pub(crate) fn draws(
    sheet: &Sheet,
    frame: &ViewFrame,
    p: &mut Painter,
    ctx: &crate::composer_text_fields::Context<'_>,
) -> Result<CaptionDraws, String> {
    let raw = sheet.native_items.get(&frame.id);
    let origin = [frame.rect.x, frame.rect.y];
    let (w, h) = (frame.rect.w, frame.rect.h);
    let mut out = vec![];
    let mut lines = Vec::new();
    if frame.show_title {
        let size = number(raw, "title_mm", 4.)?.max(1.5);
        let layout = option(raw, "title_style", "layout")?;
        let align = option(raw, "title_align", "left")?;
        let pos = option(raw, "title_pos", "below")?;
        if !matches!(layout, "layout" | "simple" | "bar")
            || !matches!(align, "left" | "center" | "right")
            || !matches!(pos, "above" | "below")
        {
            return Err("Unsupported view caption style/alignment/position".into());
        }
        let title = expand(
            sheet,
            frame,
            string(raw, "title_text", &frame.title_text)?,
            ctx,
        )?;
        let title = if title.is_empty() {
            view_name(frame)
        } else {
            title
        };
        let sub = expand(sheet, frame, string(raw, "title_subtitle", "")?, ctx)?;
        let n = expand(sheet, frame, string(raw, "title_number", "")?, ctx)?;
        let s = expand(sheet, frame, string(raw, "title_sheet", "")?, ctx)?;
        let scale = if boolean(raw, "title_scale", true)? {
            format!(
                "ESC. {}",
                crate::composer_text_fields::source_scale(frame.scale_n)
            )
        } else {
            String::new()
        };
        if layout == "bar" {
            let sw = 2.
                + size * 1.5
                + if sub.is_empty() { 0. } else { size * 1.1 }
                + if scale.is_empty() { 0. } else { size * 1.1 };
            let left = sw + 1.;
            out.push(path(
                origin,
                rect(-left, 0., sw, h),
                None,
                Some((INK, 0.25)),
            ));
            let mut x0 = 0.;
            if !n.is_empty() {
                let d = sw.min(size * 2.4) * 0.9;
                bubble(
                    p,
                    &mut out,
                    &mut lines,
                    origin,
                    -left + sw / 2.,
                    h - (d / 2. + 1.),
                    d,
                    &n,
                    &s,
                )?;
                x0 = d + 2.;
            }
            let rotated_origin = [origin[0] - left, origin[1] + h];
            let mut y = 1.;
            for (t, sz, bold, ch) in [
                (&title, size, true, size * 1.5),
                (&sub, size * 0.75, false, size * 1.1),
                (&scale, size * 0.75, false, size * 1.1),
            ] {
                if t.is_empty() {
                    continue;
                }
                out.extend(text(
                    p,
                    t,
                    rotated_origin,
                    [x0 + 1., y, h - x0 - 2., ch],
                    sz,
                    bold,
                    Alignment::Left,
                    1,
                    INK,
                    -90.,
                )?);
                y += ch;
            }
        } else {
            let d = if n.is_empty() { 0. } else { size * 2.4 };
            let block = if layout == "simple" {
                1.2 + size * 1.6 + if sub.is_empty() { 0. } else { size }
            } else {
                1.2 + d.max(size * 1.5) + 0.6 + if sub.is_empty() { 0. } else { size + 0.6 }
            };
            let by = if pos == "above" { -block } else { h + 1.2 };
            if layout == "simple" {
                let t = if scale.is_empty() {
                    title.clone()
                } else {
                    format!("{} — {}", title, &scale[5..])
                };
                let a = match align {
                    "left" => Alignment::Left,
                    "right" => Alignment::Right,
                    _ => Alignment::Center,
                };
                out.extend(text(
                    p,
                    &t,
                    origin,
                    [0., by, w, size * 1.6],
                    size,
                    true,
                    a,
                    0,
                    INK,
                    0.,
                )?);
                out.extend(text(
                    p,
                    &sub,
                    origin,
                    [0., by + size * 1.6, w, size],
                    size * 0.7,
                    false,
                    a,
                    0,
                    INK,
                    0.,
                )?);
            } else {
                let gap = size * 0.5;
                let tw = p.measure_advance(&style(
                    &title,
                    [0., 0., w, size * 1.5],
                    size,
                    true,
                    Alignment::Left,
                    INK,
                ))?;
                let sw = p.measure_advance(&style(
                    &scale,
                    [0., 0., w, size * 1.5],
                    size * 0.8,
                    false,
                    Alignment::Left,
                    INK,
                ))?;
                let total = if d > 0. { d + gap } else { 0. }
                    + tw
                    + if scale.is_empty() { 0. } else { gap + sw };
                let xb = match align {
                    "right" => (w - total).max(0.),
                    "center" => ((w - total) / 2.).max(0.),
                    _ => (0.),
                };
                let xt = xb + if d > 0. { d + gap } else { 0. };
                let ry = by + d.max(size * 1.5);
                if d > 0. {
                    bubble(
                        p,
                        &mut out,
                        &mut lines,
                        origin,
                        xb + d / 2.,
                        ry - d / 2.,
                        d,
                        &n,
                        &s,
                    )?;
                }
                out.extend(text(
                    p,
                    &title,
                    origin,
                    [xt, ry - size * 1.5, w, size * 1.5],
                    size,
                    true,
                    Alignment::Left,
                    2,
                    INK,
                    0.,
                )?);
                out.extend(text(
                    p,
                    &scale,
                    origin,
                    [xt + tw + gap, ry - size * 1.5, w, size * 1.5 - size * 0.08],
                    size * 0.8,
                    false,
                    Alignment::Left,
                    2,
                    INK,
                    0.,
                )?);
                lines.push(out.len());
                out.push(path(
                    origin,
                    vec![
                        PathOp::Move([
                            if align == "left" && d > 0. {
                                xb + d
                            } else {
                                0.
                            },
                            ry + 0.2,
                        ]),
                        PathOp::Line([w, ry + 0.2]),
                    ],
                    None,
                    Some((INK, 0.35)),
                ));
                out.extend(text(
                    p,
                    &sub,
                    origin,
                    [xt, ry + 0.6, w, size],
                    size * 0.7,
                    false,
                    Alignment::Left,
                    0,
                    INK,
                    0.,
                )?);
            }
        }
    }
    if boolean(raw, "show_scale", false)? {
        let size = number(raw, "scale_mm", 3.)?.max(1.5);
        let template = string(raw, "scale_text", "ESC. 1:{n}")?;
        let template = if template.is_empty() {
            "ESC. 1:{n}"
        } else {
            template
        };
        let shown = crate::composer_text_fields::source_scale(frame.scale_n);
        let label = if template.contains("1:{n}") {
            template.replace("1:{n}", &shown)
        } else {
            template.replace(
                "{n}",
                &crate::composer_text_fields::source_general(frame.scale_n),
            )
        };
        let pos = option(raw, "scale_pos", "under-right")?;
        let height = size * 1.4;
        let ink = [40. / 255., 46. / 255., 54. / 255., 1.];
        if pos.starts_with("under") {
            if !matches!(pos, "under-left" | "under-right") {
                return Err("Unsupported view scale position".into());
            }
            out.extend(text(
                p,
                &label,
                origin,
                [0., h + 1., w, height],
                size,
                true,
                if pos == "under-left" {
                    Alignment::Left
                } else {
                    Alignment::Right
                },
                0,
                ink,
                0.,
            )?);
        } else {
            if !matches!(pos, "inside-bl" | "inside-br") {
                return Err("Unsupported view scale position".into());
            }
            let width = label.chars().count() as f64 * size * 0.62 + 2.;
            let x = if pos == "inside-bl" {
                1.
            } else {
                w - width - 1.
            };
            let r = [x, h - height - 1., width, height];
            let mut halo = path(
                origin,
                rect(r[0], r[1], r[2], r[3]),
                Some([1., 1., 1., 220. / 255.]),
                None,
            );
            halo.clip = Some(rect(0., 0., w, h));
            out.push(halo);
            let mut labels = text(
                p,
                &label,
                origin,
                r,
                size,
                true,
                Alignment::Center,
                1,
                ink,
                0.,
            )?;
            for d in &mut labels {
                if d.clip.is_some() {
                    d.clip_intersection = Some(rect(0., 0., w, h));
                } else {
                    d.clip = Some(rect(0., 0., w, h));
                }
            }
            out.extend(labels);
        }
    }
    if out.len() > ingetrazo_formats::sheet_pdf::MAX_DRAWS {
        return Err("View caption draw budget exceeded".into());
    }
    Ok(CaptionDraws {
        draws: out,
        source_lines: lines,
    })
}
