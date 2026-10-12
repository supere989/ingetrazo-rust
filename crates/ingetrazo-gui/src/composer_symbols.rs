// SPDX-License-Identifier: GPL-3.0-or-later
//! Literal physical titleblock, north and scale commands shared by PDF and GPU export.
use ingetrazo_core::composition::{Sheet, SheetItem, TextItem};
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
fn draw(
    origin: [f64; 2],
    path: Vec<PathOp>,
    fill: Option<[f32; 4]>,
    stroke: Option<([f32; 4], f64)>,
) -> Draw {
    Draw {picture:None, rotation_deg:0.,
        origin_mm: origin,
        path,
        fill,
        stroke,
        stroke_options: stroke.map(|_|StrokeOptions {
            cap: 2,
            join: 2,
            dash: vec![],
        }),
        clip: None,
        clip_intersection: None,
        text: None,
        flatten_transform: false,
        even_odd: false,
    }
}
fn color(v: &str) -> Result<[f32; 4], String> {
    Ok(ingetrazo_core::sheet_color::parse_sheet_color(v)?.map(|c| c as f32 / 255.))
}
fn num(raw: Option<&serde_json::Value>, key: &str, default: f64) -> Result<f64, String> {
    let v = match raw.and_then(|r| r.get(key)) {
        None => default,
        Some(v) => v.as_f64().ok_or_else(|| format!("Invalid symbol {key}"))?,
    };
    if !v.is_finite() || v.abs() > 1_000_000. {
        return Err(format!("Invalid symbol {key}"));
    }
    Ok(v)
}
fn string<'a>(
    raw: Option<&'a serde_json::Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, String> {
    match raw.and_then(|r| r.get(key)) {
        None => Ok(default),
        Some(v) => v.as_str().ok_or_else(|| format!("Invalid symbol {key}")),
    }
}
fn outline(w: f64, h: f64, corner: &str, r: f64) -> Result<Vec<PathOp>, String> {
    let r = r.max(0.).min(w / 2.).min(h / 2.);
    let k = 0.5522847498;
    if corner == "chamfer" && r > 0. {
        let p = [
            [r, 0.],
            [w - r, 0.],
            [w, r],
            [w, h - r],
            [w - r, h],
            [r, h],
            [0., h - r],
            [0., r],
        ];
        let mut out = vec![PathOp::Move(p[0])];
        out.extend(p[1..].iter().copied().map(PathOp::Line));
        out.push(PathOp::Close);
        return Ok(out);
    }
    if corner == "rounded" && r > 0. {
        return Ok(vec![
            PathOp::Move([r, 0.]),
            PathOp::Line([w - r, 0.]),
            PathOp::Cubic([w - r + k * r, 0., w, r - k * r, w, r]),
            PathOp::Line([w, h - r]),
            PathOp::Cubic([w, h - r + k * r, w - r + k * r, h, w - r, h]),
            PathOp::Line([r, h]),
            PathOp::Cubic([r - k * r, h, 0., h - r + k * r, 0., h - r]),
            PathOp::Line([0., r]),
            PathOp::Cubic([0., r - k * r, r - k * r, 0., r, 0.]),
            PathOp::Close,
        ]);
    }
    if !matches!(corner, "square" | "rounded" | "chamfer") {
        return Err("Unsupported title corner".into());
    }
    Ok(rect(0., 0., w, h))
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
fn fit(p: &mut Painter, text: &str, r: [f64; 4], mut size: f64, bold: bool) -> Result<f64, String> {
    while size > 1. {
        let need = p.measure_box(&style(text, r, size, bold, Alignment::Left, INK))?;
        if need[0] <= r[2] && need[1] <= r[3] {
            return Ok(size);
        }
        size *= 0.88;
    }
    Ok(1.)
}
fn text(
    p: &mut Painter,
    text: &str,
    origin: [f64; 2],
    r: [f64; 4],
    size: f64,
    bold: bool,
    align: Alignment,
    center: bool,
    ink: [f32; 4],
    mask: Option<&[PathOp]>,
) -> Result<Vec<Draw>, String> {
    let mut out = p.draw_box(
        &style(text, r, size, bold, align, ink),
        r[3],
        u8::from(center),
    )?;
    for d in &mut out {
        let offset = d.origin_mm;
        d.origin_mm = origin;
        for clip in [&mut d.clip, &mut d.clip_intersection]
            .into_iter()
            .flatten()
        {
            for op in clip {
                map_op(op, |q| [q[0] + offset[0], q[1] + offset[1]]);
            }
        }
        if let Some(t) = &mut d.text {
            for glyph in &mut t.glyphs {
                glyph.position_px[0] += offset[0] / t.scale_mm;
                glyph.position_px[1] += offset[1] / t.scale_mm;
            }
        }
        if let Some(mask) = mask {
            d.clip_intersection = Some(mask.to_vec());
        }
    }
    Ok(out)
}
fn map_op(op: &mut PathOp, map: impl Fn([f64; 2]) -> [f64; 2]) {
    match op {
        PathOp::Move(p) | PathOp::Line(p) => *p = map(*p),
        PathOp::Cubic(p) => {
            for i in [0, 2, 4] {
                let q = map([p[i], p[i + 1]]);
                p[i] = q[0];
                p[i + 1] = q[1];
            }
        }
        PathOp::Close => {}
    }
}
fn field(
    sheet: &Sheet,
    id: uuid::Uuid,
    value: &str,
    ctx: Option<&crate::composer_text_fields::Context<'_>>,
) -> Result<String, String> {
    if value.contains('{') {
        let ctx = ctx.ok_or("Dynamic title field requires document context")?;
        crate::composer_text_fields::expand(
            sheet,
            &TextItem {
                id,
                pos: (0., 0.),
                text: value.into(),
                font_size_pt: 10.,
                bold: false,
            },
            ctx,
        )
    } else {
        Ok(value.into())
    }
}
pub(crate) fn draws(
    sheet: &Sheet,
    item: &SheetItem,
    p: &mut Painter,
    ctx: Option<&crate::composer_text_fields::Context<'_>>,
) -> Result<Vec<Draw>, String> {
    commands(sheet,item,p,ctx,None)
}
pub(crate) fn raster_draws(sheet:&Sheet,item:&SheetItem,p:&mut Painter,ctx:Option<&crate::composer_text_fields::Context<'_>>,density:f64)->Result<Vec<Draw>,String>{commands(sheet,item,p,ctx,Some(density))}
fn commands(sheet:&Sheet,item:&SheetItem,p:&mut Painter,ctx:Option<&crate::composer_text_fields::Context<'_>>,raster:Option<f64>)->Result<Vec<Draw>,String>{
    let raw = sheet.native_items.get(&item.id());
    let mut out = Vec::new();
    match item {
        SheetItem::TitleBlock(c) => {
            let (w, h) = (c.rect.w, c.rect.h);
            let origin = [c.rect.x, c.rect.y];
            if w <= 0.
                || h <= 0.
                || [w, h, origin[0], origin[1]]
                    .iter()
                    .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            {
                return Err("Invalid title rectangle".into());
            }
            let layout = string(raw, "layout", "grid")?;
            if !matches!(layout, "grid" | "banded" | "minimal") {
                return Err("Unsupported title layout".into());
            }
            let corner = string(raw, "corner", "square")?;
            let radius = num(raw, "radius_mm", 3.)?;
            let outer = outline(w, h, corner, radius)?;
            let heavy = num(raw, "border_mm", 0.5)?;
            let light = num(raw, "line_mm", 0.2)?;
            if heavy <= 0. || light <= 0. {
                return Err("Invalid title pen width".into());
            }
            let line = color(string(raw, "line_color", "#1e242c")?)?;
            let label = color(string(raw, "label_color", "#5a626c")?)?;
            let ink = color(string(raw, "text_color", "#1e242c")?)?;
            let fill = string(raw, "fill_color", "")?;
            let fill = if fill.is_empty() {
                None
            } else {
                Some(color(fill)?)
            };
            let mut rows: Vec<[String; 2]> = if let Some(rows) = raw
                .and_then(|r| r.get("campos"))
                .and_then(|v| v.as_array())
                .filter(|r| !r.is_empty())
            {
                if rows.len() > 10000 {
                    return Err("Title row budget exceeded".into());
                }
                rows.iter()
                    .map(|r| {
                        let r = r
                            .as_array()
                            .filter(|r| r.len() == 2)
                            .ok_or("Invalid title field row")?;
                        Ok([
                            r[0].as_str().ok_or("Invalid title field label")?.into(),
                            r[1].as_str().ok_or("Invalid title field value")?.into(),
                        ])
                    })
                    .collect::<Result<_, String>>()?
            } else {
                vec![
                    ["PROYECTO".into(), c.project_name.clone()],
                    ["AUTOR".into(), c.author.clone()],
                    ["FECHA".into(), c.date.clone()],
                    ["ESCALA".into(), c.scale_text.clone()],
                    ["LÁMINA".into(), c.sheet_number.clone()],
                ]
            };
            for row in &mut rows {
                row[1] = field(sheet, c.id, &row[1], ctx)?;
                if row[1].is_empty()
                    && matches!(row[0].trim().to_lowercase().as_str(), "escala" | "scale")
                {
                    row[1] = field(sheet, c.id, "{escala}", ctx)?;
                }
            }
            out.push(draw(origin, outer.clone(), Some([1.; 4]), None));
            let mut band_h = 0.;
            if layout == "banded" && rows.len() > 1 {
                let band = rows.remove(0);
                band_h = h * 1.5 / (rows.len() as f64 + 1.5);
                if let Some(fill) = fill {
                    let mut d = draw(origin, rect(0., 0., w, band_h), Some(fill), None);
                    d.clip = Some(outer.clone());
                    out.push(d);
                }
                let mut d = draw(
                    origin,
                    vec![PathOp::Move([0., band_h]), PathOp::Line([w, band_h])],
                    None,
                    Some((line, light)),
                );
                d.clip = Some(outer.clone());
                out.push(d);
                out.extend(text(
                    p,
                    &band[0],
                    origin,
                    [1.5, 0.4, w - 3., band_h * 0.32],
                    (band_h * 0.2).max(1.4),
                    true,
                    Alignment::Left,
                    false,
                    label,
                    Some(&outer),
                )?);
                let r = [1.5, band_h * 0.3, w - 3., band_h * 0.68];
                let size = fit(p, &band[1], r, band_h * 0.4, true)?;
                out.extend(text(
                    p,
                    &band[1],
                    origin,
                    r,
                    size,
                    true,
                    Alignment::Center,
                    true,
                    ink,
                    Some(&outer),
                )?);
            }
            let columns = num(raw, "columns", 1.)?;
            if columns.fract() != 0. {
                return Err("Invalid title columns".into());
            }
            let cols = (columns as usize).max(1).min(rows.len().max(1));
            let per = rows.len().div_ceil(cols).max(1);
            let body = h - band_h;
            let cw = w / cols as f64;
            let lw = num(raw, "label_mm", 0.)?;
            let lw = if lw > 0. {
                lw.min(cw * 0.6)
            } else {
                28_f64.min(cw * 0.3)
            };
            let base = body / per as f64;
            let minimal = layout == "minimal";
            let mut weights = Vec::new();
            for j in 0..per {
                let mut want = 0_f64;
                for k in 0..cols {
                    if let Some(row) = rows.get(k * per + j) {
                        let value = if row[1].is_empty() { "M" } else { &row[1] };
                        let inner = (if minimal { cw - 3. } else { cw - lw - 3. }).max(1.);
                        let size = base * if minimal { 0.42 } else { 0.52 };
                        want = want.max(
                            p.measure_box(&style(
                                value,
                                [0., 0., inner, body],
                                size,
                                false,
                                Alignment::Left,
                                ink,
                            ))?[1]
                                / if minimal { 0.62 } else { 1. }
                                + 1.,
                        );
                        if !minimal && !row[0].is_empty() {
                            want = want.max(
                                p.measure_box(&style(
                                    &row[0],
                                    [0., 0., (lw - 2.).max(1.), body],
                                    base * 0.38,
                                    true,
                                    Alignment::Left,
                                    label,
                                ))?[1]
                                    + 1.,
                            );
                        }
                    }
                }
                weights.push((want / base).clamp(0.6, 3.));
            }
            let sum: f64 = weights.iter().sum();
            let heights: Vec<_> = weights.iter().map(|v| body * v / sum).collect();
            for k in 0..cols {
                let x = k as f64 * cw;
                let mut y = band_h;
                if !minimal {
                    if let Some(fill) = fill {
                        let mut d = draw(origin, rect(x, band_h, lw, body), Some(fill), None);
                        d.clip = Some(outer.clone());
                        out.push(d);
                    }
                    for xx in [if k > 0 { Some(x) } else { None }, Some(x + lw)]
                        .into_iter()
                        .flatten()
                    {
                        let mut d = draw(
                            origin,
                            vec![PathOp::Move([xx, band_h]), PathOp::Line([xx, h])],
                            None,
                            Some((line, light)),
                        );
                        d.clip = Some(outer.clone());
                        out.push(d);
                    }
                }
                for (j, row) in rows.iter().skip(k * per).take(per).enumerate() {
                    let rh = heights[j];
                    if !minimal && j > 0 {
                        let mut d = draw(
                            origin,
                            vec![PathOp::Move([x, y]), PathOp::Line([x + cw, y])],
                            None,
                            Some((line, light)),
                        );
                        d.clip = Some(outer.clone());
                        out.push(d);
                    }
                    let (lr, vr, ls, vs) = if minimal {
                        (
                            [x + 1.5, y + 0.4, cw - 3., rh * 0.36],
                            [x + 1.5, y + rh * 0.36, cw - 3., rh * 0.62],
                            (base * 0.24).max(1.4),
                            base * 0.42,
                        )
                    } else {
                        (
                            [x + 1.2, y + 0.5, lw - 2., rh - 1.],
                            [x + lw + 1.5, y + 0.5, cw - lw - 3., rh - 1.],
                            base * 0.38,
                            base * 0.52,
                        )
                    };
                    let ls = if minimal {
                        ls
                    } else {
                        fit(p, &row[0], lr, ls, true)?
                    };
                    let vs = fit(p, &row[1], vr, vs, false)?;
                    out.extend(text(
                        p,
                        &row[0],
                        origin,
                        lr,
                        ls,
                        true,
                        Alignment::Left,
                        !minimal,
                        label,
                        Some(&outer),
                    )?);
                    out.extend(text(
                        p,
                        &row[1],
                        origin,
                        vr,
                        vs,
                        false,
                        Alignment::Left,
                        true,
                        ink,
                        Some(&outer),
                    )?);
                    y += rh;
                }
            }
            out.push(draw(origin, outer, None, Some((line, heavy))));
            if raw
                .and_then(|r| r.get("double_border"))
                .and_then(|v| v.as_bool())
                == Some(true)
            {
                let inner = outline(w - 2.4, h - 2.4, corner, radius - 1.2)?;
                out.push(draw(
                    [origin[0] + 1.2, origin[1] + 1.2],
                    inner,
                    None,
                    Some((line, light)),
                ));
            }
        }
        SheetItem::GraphicScale(c) => {
            if !c.scale_n.is_finite() || c.scale_n <= 0. || c.divisions == 0 || c.divisions > 10000
            {
                return Err("Invalid graphic scale".into());
            }
            let raw_m = 100. / c.divisions as f64 * c.scale_n / 1000.;
            let mut m = 0.1;
            for v in [
                0.1, 0.2, 0.25, 0.5, 1., 2., 2.5, 5., 10., 20., 25., 50., 100., 200., 250., 500.,
                1000., 2000., 5000.,
            ] {
                if v <= raw_m {
                    m = v;
                }
            }
            let mm = m * 1000. / c.scale_n;
            let origin = [c.pos.0, c.pos.1];
            let w = mm * c.divisions as f64;
            for i in 0..c.divisions {
                out.push(draw(
                    origin,
                    rect(i as f64 * mm, 0., mm, 2.4),
                    Some(if i % 2 == 0 { INK } else { [1.; 4] }),
                    Some((INK, 0.25)),
                ));
            }
            for i in 0..=c.divisions {
                out.extend(text(
                    p,
                    &crate::composer_text_fields::source_general(i as f64 * m),
                    origin,
                    [i as f64 * mm - 12., 3.2, 24., 4.],
                    2.6,
                    false,
                    Alignment::Center,
                    false,
                    INK,
                    None,
                )?);
            }
            out.extend(text(
                p,
                &ingetrazo_core::i18n::tr("metres — scale 1:{n}").replace(
                    "{n}",
                    &crate::composer_text_fields::source_general(c.scale_n),
                ),
                origin,
                [0., 7., w, 4.],
                2.6,
                false,
                Alignment::Center,
                false,
                INK,
                None,
            )?);
        }
        SheetItem::NorthArrow(c) => {
            if !c.size_mm.is_finite() || c.size_mm <= 0. || !c.rot_deg.is_finite() {
                return Err("Invalid north symbol".into());
            }
            let sz = c.size_mm;
            let rad = sz * 0.3;
            let r = rad * 0.78;
            let origin = [c.pos.0 + sz / 2., c.pos.1 + sz / 2.];
            let a = c.rot_deg.to_radians();
            let (sin, cos) = a.sin_cos();
            let map = |v: [f64; 2]| [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos];
            use ingetrazo_render::qt_gray::{PathContour,Segment};
            let rx=rad*0.92;let k=rx*0.5522847498;
            // qt_curves_for_arc(QRectF(-rx,-rx,2rx,2rx),0,-360):
            // retain its centered operation order rather than translate a positive-box ellipse.
            let circle=PathContour{start:[rx,0.],segments:vec![
                Segment::Cubic{control1:[rx,k],control2:[k,rx],to:[0.,rx]},
                Segment::Cubic{control1:[-k,rx],control2:[-rx,k],to:[-rx,0.]},
                Segment::Cubic{control1:[-rx,-k],control2:[-k,-rx],to:[0.,-rx]},
                Segment::Cubic{control1:[k,-rx],control2:[rx,-k],to:[rx,0.]}
            ]};
            let ops=|path:&ingetrazo_render::qt_gray::PathContour|{
                use ingetrazo_render::qt_gray::Segment;
                let mut out=vec![PathOp::Move(path.start)];for segment in &path.segments{out.push(match segment{Segment::Line(p)=>PathOp::Line(*p),Segment::Cubic{control1,control2,to}=>PathOp::Cubic([control1[0],control1[1],control2[0],control2[1],to[0],to[1]])});}out.push(PathOp::Close);out
            };
            let local=map;
            if let Some(density)=raster{
                let mut path=Vec::new();for contour in crate::composer_cad::source_curve_stroke(&circle,0.175)?{
                    let points=ingetrazo_render::qt_gray::flatten_local_path(&contour,density,100_000)?;
                    path.try_reserve(points.len()+1).map_err(|_|"North contour allocation refused")?;
                    for(i,q)in points.into_iter().enumerate(){path.push(if i==0{PathOp::Move(local(q))}else{PathOp::Line(local(q))});}path.push(PathOp::Close);
                }out.push(draw(origin,path,Some(INK),None));
            }else{let path=ops(&circle);let mut d=draw(origin,path,None,Some((INK,0.35)));d.rotation_deg=c.rot_deg;out.push(d);}
            for(side,fill)in[(1.,INK),(-1.,[1.;4])]{
                let points=[glam::DVec2::new(0.,-r),glam::DVec2::new(side*r*0.28,r*0.35),glam::DVec2::new(0.,r*0.12)];
                let mut path=vec![PathOp::Move(points[0].to_array()),PathOp::Line(points[1].to_array()),PathOp::Line(points[2].to_array()),PathOp::Close];if raster.is_some(){for op in &mut path{map_op(op,map);}}
                let mut d=draw(origin,path,Some(fill),if raster.is_some(){None}else{Some((INK,0.35))});if raster.is_none(){d.rotation_deg=c.rot_deg;}out.push(d);
                if raster.is_some(){let mut path=Vec::new();for contour in crate::composer_cad::convex_bevel_outline(&points,0.175)?{for(i,q)in contour.iter().enumerate(){path.push(if i==0{PathOp::Move(map(q.to_array()))}else{PathOp::Line(map(q.to_array()))});}path.push(PathOp::Close);}out.push(draw(origin,path,Some(INK),None));}
            }
            let h = sz * 0.18;
            let label = text(
                p,
                "N",
                [0.; 2],
                [-sz / 2., -sz * 0.4 - h / 2., sz, h],
                h,
                true,
                Alignment::Center,
                true,
                INK,
                None,
            )?;
            for mut d in label {d.origin_mm=origin;for path in [&mut d.clip,&mut d.clip_intersection].into_iter().flatten(){for op in path{map_op(op,map);}}if let Some(t)=&mut d.text{t.rotation_deg=c.rot_deg;}out.push(d);}

        }
        _ => return Err("Unsupported physical symbol".into()),
    }
    if out.len() > ingetrazo_formats::sheet_pdf::MAX_DRAWS {
        return Err("Symbol draw budget exceeded".into());
    }
    Ok(out)
}
