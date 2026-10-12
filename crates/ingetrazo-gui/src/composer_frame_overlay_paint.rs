//! Source `_paint_annots_mm`, using the shared native paper painter.
use super::*;
use crate::composer::frame_annotations::{Plan, Primitive};
const INK: [f32; 4] = [45. / 255., 55. / 255., 75. / 255., 1.];
const HALO: [f32; 4] = [1., 1., 1., 230. / 255.];
const PATH_INK: [f32; 4] = [0., 150. / 255., 170. / 255., 1.];
pub(crate) fn draws(
    plan: &Plan,
    width: f64,
    height: f64,
    fonts: &mut Painter,
) -> Result<Draws, String> {
    let mut o = Output::new([0.; 2]);
    for primitive in &plan.primitives {
        match primitive {
            Primitive::Line([x, y, a, b]) => o.line([*x, *y], [*a, *b], HALO, 0.7, 2),
            Primitive::Section {
                line: [x, y, a, b], ..
            } => o.line([*x, *y], [*a, *b], HALO, 1., 2),
            Primitive::Poly(points) => poly(&mut o, points, HALO, 0.9),
            _ => {}
        }
    }
    for primitive in &plan.primitives {
        match primitive {
            Primitive::Line([x, y, a, b]) => o.line([*x, *y], [*a, *b], INK, 0.25, 2),
            Primitive::Poly(points) => {
                poly(&mut o, points, PATH_INK, 0.4);
                for &[x, y] in points {
                    o.path(
                        ellipse(x, y, 0.45, 0.45),
                        Some(PATH_INK),
                        Some((HALO, 0.15)),
                        2,
                        2,
                        &[],
                    );
                }
            }
            Primitive::Arrow([x, y, dx, dy]) => {
                let angle = dy.atan2(*dx);
                let base = 12_f64.to_radians();
                o.polygon(
                    &[
                        [*x, *y],
                        [
                            x + 1.8 * (angle + base).cos(),
                            y + 1.8 * (angle + base).sin(),
                        ],
                        [
                            x + 1.8 * (angle - base).cos(),
                            y + 1.8 * (angle - base).sin(),
                        ],
                    ],
                    Some(INK),
                    None,
                    2,
                );
            }
            Primitive::Text {
                at,
                angle,
                text: t,
                size,
            } => {
                let d = 0.12_f64.max(size * 0.06);
                for [dx, dy] in [
                    [-d, 0.],
                    [d, 0.],
                    [0., -d],
                    [0., d],
                    [-d, -d],
                    [d, d],
                    [-d, d],
                    [d, -d],
                    [0., 0.],
                ] {
                    text(
                        &mut o,
                        fonts,
                        t,
                        [-60. + dx, -size * 1.3 + dy, 120., size * 1.3],
                        *size,
                        true,
                        false,
                        false,
                        Alignment::Center,
                        2,
                        if dx == 0. && dy == 0. { INK } else { HALO },
                        *at,
                        *angle,
                    )?;
                }
            }
            Primitive::Section {
                line,
                direction,
                label,
                size,
            } => section(&mut o, fonts, *line, *direction, label, *size)?,
        }
    }
    // Text's own rotated rectangle clip remains first; frame clip is a second
    // intersection expressed in each draw's local origin, without rotating it.
    for draw in &mut o.draws {
        let clip = rect([-draw.origin_mm[0], -draw.origin_mm[1], width, height]);
        if draw.clip.is_some() {
            if draw.clip_intersection.is_some() {
                return Err("Frame overlay exceeds clip intersection depth".into());
            }
            draw.clip_intersection = Some(clip);
        } else {
            draw.clip = Some(clip);
        }
    }
    o.finish()
}
fn poly(o: &mut Output, points: &[[f64; 2]], ink: [f32; 4], width: f64) {
    if let Some((first, rest)) = points.split_first() {
        let mut path = Vec::with_capacity(points.len());
        path.push(PathOp::Move(*first));
        path.extend(rest.iter().copied().map(PathOp::Line));
        o.path(path, None, Some((ink, width)), 1, 1, &[]);
    }
}
fn section(
    o: &mut Output,
    p: &mut Painter,
    [x0, y0, x1, y1]: [f64; 4],
    [ax, ay]: [f64; 2],
    label: &str,
    size: f64,
) -> Result<(), String> {
    o.path(
        vec![PathOp::Move([x0, y0]), PathOp::Line([x1, y1])],
        None,
        Some((INK, 0.35)),
        0,
        2,
        &[2.8, 0.7, 0.35, 0.7],
    );
    let len = (x1 - x0).hypot(y1 - y0);
    let len = if len == 0. { 1. } else { len };
    let (ux, uy) = ((x1 - x0) / len, (y1 - y0) / len);
    let stub = 3_f64.max(size * 1.2);
    let arrow = 2.5_f64.max(size);
    let r = size * 0.95;
    for (ex, ey, sx, sy) in [(x0, y0, ux, uy), (x1, y1, -ux, -uy)] {
        o.line([ex, ey], [ex + sx * stub, ey + sy * stub], INK, 0.7, 0);
        let (bx, by) = (ex + sx * stub * 0.5, ey + sy * stub * 0.5);
        let wing = arrow * 0.45;
        o.polygon(
            &[
                [bx + ax * arrow, by + ay * arrow],
                [bx + ux * wing, by + uy * wing],
                [bx - ux * wing, by - uy * wing],
            ],
            Some(INK),
            None,
            2,
        );
        let (cx, cy) = (bx + ax * (arrow + 0.8 + r), by + ay * (arrow + 0.8 + r));
        o.path(
            ellipse(cx, cy, r, r),
            Some([1.; 4]),
            Some((INK, 0.35)),
            2,
            2,
            &[],
        );
        text(
            o,
            p,
            label,
            [cx - r, cy - r, 2. * r, 2. * r],
            size * 0.9,
            true,
            false,
            false,
            Alignment::Center,
            1,
            INK,
            [0.; 2],
            0.,
        )?;
    }
    Ok(())
}
