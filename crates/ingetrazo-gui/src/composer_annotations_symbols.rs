// SPDX-License-Identifier: GPL-3.0-or-later
//! Labels, elevation marks and callouts from the original millimetre painters.
use super::*;

pub(super) fn label(
    out: &mut Output,
    sheet: &Sheet,
    value: &Value,
    painter: &mut Painter,
    context: &Context<'_>,
) -> Result<(), String> {
    let content = expanded(sheet, s(value, "text", "Texto")?, context, "")?;
    let size = n(value, "size_pt", 11.)? * 25.4 / 72.;
    let width = n(value, "w_mm", 50.)?;
    let bold = b(value, "bold", false)?;
    let ink = color(s(value, "color", "#1e242c")?)?;
    let stroke = n(value, "stroke_mm", 0.25)?;
    let metric_style = style(
        &content,
        [0., 0., width.max(0.5), 1_000_000.],
        size,
        bold,
        false,
        false,
        Alignment::Left,
        ink,
    );
    let [ascent, descent, leading, cap_height] = painter.font_metrics_mm(&metric_style)?;
    let line_height = ascent + descent;
    let count = if content.is_empty() {
        0.
    } else {
        (painter.measure_box(&metric_style)?[1] / line_height.max(1e-6))
            .round_ties_even()
            .max(1.)
    };
    let laid_out = if count == 0. {
        0.
    } else {
        line_height + (count - 1.) * (line_height + leading)
    };
    let height = (size * 1.15).max(laid_out);
    let mut widest = 0_f64;
    for line in content.split('\n') {
        widest = widest.max(painter.measure_advance(&style(
            line,
            [0., 0., width, height],
            size,
            bold,
            false,
            false,
            Alignment::Left,
            ink,
        ))?);
    }
    let ink_width = width.min(widest + 0.2).max(2.);
    let extras = match value.get("leaders") {
        None | Some(Value::Null) => &[][..],
        Some(v) => v
            .as_array()
            .map(Vec::as_slice)
            .ok_or("Invalid label leaders")?,
    };
    // Admit before allocating any leader geometry. Each leader emits at most three draws.
    if extras.len() > 10_000 {
        return Err("Label leader budget exceeded".into());
    }
    let first = [n(value, "ax_mm", -20.)?, n(value, "ay_mm", 15.)?];
    let dot_radius = if b(value, "dot", true)? {
        0.45_f64.max(stroke * 1.8)
    } else {
        0.
    };
    let arrow = b(value, "arrow", true)?;
    for index in 0..=extras.len() {
        let [ax, ay] = if index == 0 {
            first
        } else {
            let item = &extras[index - 1];
            if !item.is_object() {
                return Err("Invalid label leader".into());
            }
            [n(item, "ax_mm", 0.)?, n(item, "ay_mm", 0.)?]
        };
        let [sx, sy] = if ax < 0. {
            [-0.8, height / 2.]
        } else if ax > ink_width {
            [ink_width + 0.8, height / 2.]
        } else if ay < 0. {
            [ink_width / 2., -0.8]
        } else {
            [ink_width / 2., height + 0.8]
        };
        out.line([sx, sy], [ax, ay], ink, stroke, 1);
        if dot_radius != 0. {
            out.path(
                ellipse(sx, sy, dot_radius, dot_radius),
                Some(ink),
                None,
                2,
                2,
                &[],
            );
        }
        if arrow {
            let angle = (sy - ay).atan2(sx - ax);
            let length = 1.8_f64.max(stroke * 7.);
            let base = 14_f64.to_radians();
            out.polygon(
                &[
                    [ax, ay],
                    [
                        ax + length * (angle + base).cos(),
                        ay + length * (angle + base).sin(),
                    ],
                    [
                        ax + length * (angle - base).cos(),
                        ay + length * (angle - base).sin(),
                    ],
                ],
                Some(ink),
                None,
                2,
            );
        }
    }
    let background = s(value, "bg_color", "")?;
    if !background.is_empty() {
        let (top, bottom) = if count == 0. {
            (0., size)
        } else {
            (ascent - cap_height - 0.25 * size, laid_out + 0.05 * size)
        };
        out.rectangle(
            [-0.5, top, ink_width + 1., (bottom - top).max(1.)],
            opacity(color(background)?, n(value, "bg_opacity", 1.)?, pdf),
        );
    }
    text(
        out,
        painter,
        &content,
        [0., 0., width, height + size],
        size,
        bold,
        b(value, "italic", false)?,
        b(value, "underline", false)?,
        Alignment::Left,
        0,
        ink,
        [0.; 2],
        0.,
    )
}

pub(super) fn level_label(value: &Value) -> Result<String, String> {
    let anchor = s(value, "anchor_uid", "")?;
    let world = value.get("a_world").and_then(Value::as_array);
    let z = if !anchor.is_empty() && world.is_some_and(|v| v.len() > 2) {
        world.unwrap()[2]
            .as_f64()
            .filter(|z| z.is_finite())
            .ok_or("Invalid stored elevation anchor")?
    } else {
        n(value, "z_m", 0.)?
    };
    let elevation = z - n(value, "datum_m", 0.)?;
    if !elevation.is_finite() {
        return Err("Invalid elevation value".into());
    }
    let decimals = n(value, "decimals", 2.)?.trunc().max(0.) as usize;
    if decimals > 256 {
        return Err("Elevation precision exceeds text budget".into());
    }
    let signed = format!("{elevation:+.*}", decimals);
    let formatted = if signed.chars().all(|c| matches!(c, '+' | '-' | '0' | '.')) {
        format!("±{:.*}", decimals, 0.)
    } else {
        signed
    };
    let label = option(value, "text", "N.P.T. {z}")?;
    // An explicit empty prefix means value-only in the original model.
    let label = if value.get("text").and_then(Value::as_str) == Some("") {
        "{z}"
    } else {
        label
    };
    Ok(if label.contains("{z}") {
        label.replace("{z}", &formatted)
    } else {
        format!("{} {formatted}", label.trim_end())
    })
}

pub(super) fn level(out: &mut Output, value: &Value, painter: &mut Painter) -> Result<(), String> {
    let ink = color(s(value, "color", "#1e242c")?)?;
    let stroke = n(value, "stroke_mm", 0.25)?;
    let size = n(value, "size_mm", 2.5)?;
    let symbol_size = 1.5_f64.max(size * 1.2);
    let half = symbol_size / 2.;
    let mirror = b(value, "mirror", false)?;
    let sign = if mirror { -1. } else { 1. };
    let anchor = [n(value, "ax_mm", 0.)?, n(value, "ay_mm", 0.)?];
    if anchor[0].hypot(anchor[1]) > 0.3 {
        out.line(anchor, [0.; 2], ink, (stroke * 0.5).max(0.1), 2);
    }
    let line_y = if s(value, "symbol", "triangle")? == "circle" {
        out.path(
            ellipse(0., 0., half, half),
            None,
            Some((ink, stroke)),
            1,
            1,
            &[],
        );
        let k = half * 0.5522847498;
        out.path(
            vec![
                PathOp::Move([0., 0.]),
                PathOp::Line([half, 0.]),
                PathOp::Cubic([half, -k, k, -half, 0., -half]),
                PathOp::Close,
                PathOp::Move([0., 0.]),
                PathOp::Line([-half, 0.]),
                PathOp::Cubic([-half, k, -k, half, 0., half]),
                PathOp::Close,
            ],
            Some(ink),
            None,
            2,
            2,
            &[],
        );
        -half
    } else {
        let height = symbol_size * 0.866;
        out.polygon(
            &[[0., 0.], [-half, -height], [half, -height]],
            None,
            Some((ink, stroke)),
            1,
        );
        -height
    };
    let content = level_label(value)?;
    let start = half + 0.6;
    let advance = painter.measure_advance(&style(
        &content,
        [0., 0., 80., size],
        size,
        false,
        false,
        false,
        Alignment::Left,
        ink,
    ))?;
    let line = n(value, "line_mm", 14.)?.max(start + advance + 1.);
    out.line([0., line_y], [sign * line, line_y], ink, stroke, 1);
    let height = size * 1.4;
    let bounds = [
        if mirror { -line } else { start },
        line_y - height,
        line - start,
        height,
    ];
    text(
        out,
        painter,
        &content,
        bounds,
        size,
        false,
        false,
        false,
        if mirror {
            Alignment::Right
        } else {
            Alignment::Left
        },
        2,
        ink,
        [0.; 2],
        0.,
    )
}

fn rounded_callout(width: f64, height: f64) -> Vec<PathOp> {
    let rx = 1.5_f64.min(width / 2.);
    let ry = 1.5_f64.min(height / 2.);
    let k = 0.5522847498;
    // Qt addRoundedRect starts on the left edge; dash phase depends on this order.
    vec![
        PathOp::Move([0., ry]),
        PathOp::Cubic([0., ry - ry * k, rx - rx * k, 0., rx, 0.]),
        PathOp::Line([width - rx, 0.]),
        PathOp::Cubic([width - rx + rx * k, 0., width, ry - ry * k, width, ry]),
        PathOp::Line([width, height - ry]),
        PathOp::Cubic([
            width,
            height - ry + ry * k,
            width - rx + rx * k,
            height,
            width - rx,
            height,
        ]),
        PathOp::Line([rx, height]),
        PathOp::Cubic([
            rx - rx * k,
            height,
            0.,
            height - ry + ry * k,
            0.,
            height - ry,
        ]),
        PathOp::Line([0., ry]),
        PathOp::Close,
    ]
}

// QPaintEngineEx::drawRoundedRect starts at the top edge; the nonextended
// PDF engine consumes QPainterPath::addRoundedRect, starting at the left edge.
// Dash phase makes these different source operations observable.
fn raster_rounded_callout(width: f64, height: f64) -> Vec<PathOp> {
    let rx = 1.5_f64.min(width / 2.);
    let ry = 1.5_f64.min(height / 2.);
    let k = 0.5522847498;
    vec![
        PathOp::Move([rx, 0.]),
        PathOp::Line([width - rx, 0.]),
        PathOp::Cubic([width - (1. - k) * rx, 0., width, (1. - k) * ry, width, ry]),
        PathOp::Line([width, height - ry]),
        PathOp::Cubic([
            width,
            height - (1. - k) * ry,
            width - (1. - k) * rx,
            height,
            width - rx,
            height,
        ]),
        PathOp::Line([rx, height]),
        PathOp::Cubic([
            (1. - k) * rx,
            height,
            0.,
            height - (1. - k) * ry,
            0.,
            height - ry,
        ]),
        PathOp::Line([0., ry]),
        PathOp::Cubic([0., (1. - k) * ry, (1. - k) * rx, 0., rx, 0.]),
        PathOp::Close,
    ]
}

pub(super) fn callout(
    out: &mut Output,
    sheet: &Sheet,
    value: &Value,
    painter: &mut Painter,
    context: &Context<'_>,
    pdf: bool,
) -> Result<(), String> {
    let ink = color(s(value, "color", "#1e242c")?)?;
    let stroke = n(value, "stroke_mm", 0.3)?;
    let width = n(value, "w_mm", 30.)?;
    let height = n(value, "h_mm", 20.)?;
    if width <= 0. || height <= 0. {
        return Err("Nonpositive callout bounds".into());
    }
    let cx = n(value, "bx_mm", 36.)?;
    let cy = n(value, "by_mm", -6.)?;
    let diameter = (n(value, "size_mm", 3.)? * 2.4).max(3.);
    let radius = diameter / 2.;
    let circular = s(value, "shape", "rect")? == "circle";
    out.path(
        if circular {
            ellipse(width / 2., height / 2., width / 2., height / 2.)
        } else if pdf {
            rounded_callout(width, height)
        } else {
            raster_rounded_callout(width, height)
        },
        None,
        Some((ink, stroke)),
        0,
        2,
        &[6. * stroke, 3. * stroke],
    );
    let start = if circular {
        let (rx, ry) = (width / 2., height / 2.);
        let (dx, dy) = (cx - rx, cy - ry);
        if rx > 1e-9 && ry > 1e-9 && (dx / rx).powi(2) + (dy / ry).powi(2) <= 1. {
            None
        } else {
            let angle = (dy / ry.max(1e-9)).atan2(dx / rx.max(1e-9));
            Some([rx + rx * angle.cos(), ry + ry * angle.sin()])
        }
    } else if 0. <= cx && cx <= width && 0. <= cy && cy <= height {
        None
    } else {
        Some([cx.max(0.).min(width), cy.max(0.).min(height)])
    };
    if let Some([px, py]) = start {
        let (dx, dy) = (cx - px, cy - py);
        let length = dx.hypot(dy);
        if length > radius + 0.3 {
            out.line(
                [px, py],
                [cx - dx / length * radius, cy - dy / length * radius],
                ink,
                stroke,
                2,
            );
        }
    }
    let uid = s(value, "frame_uid", "")?;
    let number = expanded(sheet, s(value, "number", "1")?, context, uid)?;
    let reference = expanded(sheet, s(value, "sheet", "{lamina}")?, context, uid)?;
    out.path(
        ellipse(cx, cy, radius, radius),
        Some([1.; 4]),
        Some((ink, 0.35)),
        2,
        2,
        &[],
    );
    if !reference.is_empty() {
        out.line([cx - radius, cy], [cx + radius, cy], ink, 0.35, 2);
        text(
            out,
            painter,
            &number,
            [cx - radius, cy - radius, diameter, radius],
            diameter * 0.34,
            true,
            false,
            false,
            Alignment::Center,
            1,
            ink,
            [0.; 2],
            0.,
        )?;
        text(
            out,
            painter,
            &reference,
            [cx - radius, cy, diameter, radius],
            diameter * 0.26,
            false,
            false,
            false,
            Alignment::Center,
            1,
            ink,
            [0.; 2],
            0.,
        )?;
    } else {
        text(
            out,
            painter,
            &number,
            [cx - radius, cy - radius, diameter, diameter],
            diameter * 0.42,
            true,
            false,
            false,
            Alignment::Center,
            1,
            ink,
            [0.; 2],
            0.,
        )?;
    }
    Ok(())
}
