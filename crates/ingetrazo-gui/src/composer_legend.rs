// SPDX-License-Identifier: GPL-3.0-or-later
//! Source legend snapshot painter shared by physical PDF and direct native PNG.
use ingetrazo_formats::{
    paper_text_layout::Alignment,
    paper_text_painter::{Painter, TextStyle},
    sheet_pdf::{Draw, PathOp, StrokeOptions},
};
const INK: [f32; 4] = [30. / 255., 36. / 255., 44. / 255., 1.];
pub(crate) struct LegendDraws {
    pub(crate) draws: Vec<Draw>,
    pub(crate) rectangles: Vec<usize>,
}
fn rectangle(origin: [f64; 2], r: [f64; 4], fill: Option<[f32; 4]>, stroke: Option<f64>) -> Draw {
    let [x, y, w, h] = r;
    Draw {
        origin_mm: origin,
        path: vec![
            PathOp::Move([x, y]),
            PathOp::Line([x + w, y]),
            PathOp::Line([x + w, y + h]),
            PathOp::Line([x, y + h]),
            PathOp::Close,
        ],
        fill,
        stroke: stroke.map(|w| (INK, w)),
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
fn number(v: &serde_json::Value, key: &str, default: f64) -> Result<f64, String> {
    let x = match v.get(key) {
        None => default,
        Some(v) => v.as_f64().ok_or_else(|| format!("Invalid legend {key}"))?,
    };
    if !x.is_finite() || x.abs() > 1_000_000. {
        return Err(format!("Invalid legend {key}"));
    }
    Ok(x)
}
fn text(
    p: &mut Painter,
    s: &str,
    origin: [f64; 2],
    r: [f64; 4],
    size: f64,
    bold: bool,
) -> Result<Vec<Draw>, String> {
    if s.is_empty() {
        return Ok(vec![]);
    }
    let mut draws = p.draw_box(
        &TextStyle {
            text: s,
            origin_mm: [r[0], r[1]],
            width_mm: r[2],
            size_pt: size * 72. / 25.4,
            family: "Sans Serif",
            bold,
            italic: false,
            underline: false,
            alignment: Alignment::Left,
            color: INK,
        },
        r[3],
        0,
    )?;
    for d in &mut draws {
        d.origin_mm = origin;
    }
    Ok(draws)
}
pub(crate) fn draws(value: &serde_json::Value, p: &mut Painter) -> Result<LegendDraws, String> {
    if !value.is_object() {
        return Err("Invalid legend record".into());
    }
    let origin = [number(value, "x_mm", 20.)?, number(value, "y_mm", 20.)?];
    let width = number(value, "w_mm", 55.)?;
    if width <= 10. {
        return Err("Legend text box has nonpositive supported width".into());
    }
    let title = match value.get("title") {
        None => "LEYENDA",
        Some(v) => v.as_str().ok_or("Invalid legend title")?,
    };
    let rows = match value.get("rows") {
        None => &[][..],
        Some(v) => v
            .as_array()
            .map(Vec::as_slice)
            .ok_or("Invalid legend snapshot rows")?,
    };
    if rows.len() > 10_000 {
        return Err("Legend snapshot row budget exceeded".into());
    }
    // Validate all inputs before admitting any output resources. Source rows remain snapshots.
    for row in rows {
        if row.as_str().is_none() {
            return Err("Invalid legend snapshot label".into());
        }
    }
    let height = 7.5 + 5.5 * rows.len().max(1) as f64;
    let mut out = vec![];
    let mut rectangles = vec![];
    rectangles.push(out.len());
    out.push(rectangle(
        origin,
        [0., 0., width, height],
        Some([1.; 4]),
        None,
    ));
    out.push(rectangle(origin, [0., 0., width, height], None, Some(0.3)));
    out.extend(text(
        p,
        title,
        origin,
        [2., 1.4, width - 4., 5.],
        3.2,
        true,
    )?);
    let empty = ingetrazo_core::i18n::tr("(no layers)");
    let mut y = 7.5;
    for index in 0..rows.len().max(1) {
        let label = if rows.is_empty() {
            empty.as_str()
        } else {
            rows[index].as_str().unwrap()
        };
        rectangles.push(out.len());
        out.push(rectangle(
            origin,
            [2.2, y + 0.8, 4., 3.2],
            Some([226. / 255., 232. / 255., 238. / 255., 1.]),
            Some(0.2),
        ));
        out.extend(text(
            p,
            label,
            origin,
            [8., y + 0.7, width - 10., 5.],
            2.8,
            false,
        )?);
        y += 5.5;
        if out.len() > ingetrazo_formats::sheet_pdf::MAX_DRAWS {
            return Err("Legend draw budget exceeded".into());
        }
    }
    Ok(LegendDraws {
        draws: out,
        rectangles,
    })
}
