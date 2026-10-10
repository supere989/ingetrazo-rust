// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared source shape commands to direct PDF paths. Other item painters stay
//! explicitly refused until implemented, rather than silently omitted.
use crate::sheet_pdf::{Draw, Page, PathOp};
use ingetrazo_core::{
    composition::{Sheet, SheetItem},
    sheet_color::parse_sheet_color,
    sheet_shape::{ShapePaint, ShapePath},
};
fn polygon(points: &[glam::DVec2], offset: glam::DVec2) -> Vec<PathOp> {
    let mut result = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let p = (*p + offset).to_array();
        result.push(if i == 0 {
            PathOp::Move(p)
        } else {
            PathOp::Line(p)
        });
    }
    result.push(PathOp::Close);
    result
}
fn path(shape: &ShapePath, offset: glam::DVec2) -> Vec<PathOp> {
    let xy = |x: f64, y: f64| [x + offset.x, y + offset.y];
    let cubic = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        let a = xy(a[0], a[1]);
        let b = xy(b[0], b[1]);
        let c = xy(c[0], c[1]);
        PathOp::Cubic([a[0], a[1], b[0], b[1], c[0], c[1]])
    };
    let k = 0.5522847498307934;
    match shape {
        ShapePath::Line(a, b) => vec![
            PathOp::Move((*a + offset).to_array()),
            PathOp::Line((*b + offset).to_array()),
        ],
        ShapePath::Polygon(p) => polygon(p, offset),
        ShapePath::Ellipse { size } => {
            let (rx, ry) = (size.x / 2., size.y / 2.);
            vec![
                PathOp::Move(xy(size.x, ry)),
                cubic([size.x, ry + k * ry], [rx + k * rx, size.y], [rx, size.y]),
                cubic([rx - k * rx, size.y], [0., ry + k * ry], [0., ry]),
                cubic([0., ry - k * ry], [rx - k * rx, 0.], [rx, 0.]),
                cubic([rx + k * rx, 0.], [size.x, ry - k * ry], [size.x, ry]),
                PathOp::Close,
            ]
        }
        ShapePath::RoundedRect { size, radius } => {
            let (w, h, r) = (size.x, size.y, *radius);
            if r <= 0. {
                return polygon(
                    &[
                        glam::DVec2::ZERO,
                        glam::DVec2::new(w, 0.),
                        *size,
                        glam::DVec2::new(0., h),
                    ],
                    offset,
                );
            }
            vec![
                PathOp::Move(xy(r, 0.)),
                PathOp::Line(xy(w - r, 0.)),
                cubic([w - r + k * r, 0.], [w, r - k * r], [w, r]),
                PathOp::Line(xy(w, h - r)),
                cubic([w, h - r + k * r], [w - r + k * r, h], [w - r, h]),
                PathOp::Line(xy(r, h)),
                cubic([r - k * r, h], [0., h - r + k * r], [0., h - r]),
                PathOp::Line(xy(0., r)),
                cubic([0., r - k * r], [r - k * r, 0.], [r, 0.]),
                PathOp::Close,
            ]
        }
    }
}
fn draw(command: &ShapePaint, offset: glam::DVec2) -> Result<Draw, String> {
    let origin_mm = offset.to_array();
    let offset = glam::DVec2::ZERO;
    let color = |value: &str| parse_sheet_color(value).map(|c| c.map(|v| v as f32 / 255.));
    let stroke = command
        .stroke
        .as_ref()
        .map(|(c, w)| Ok::<_, String>((color(c)?, *w)))
        .transpose()?;
    let fill = command
        .fill
        .as_ref()
        .map(|(c, alpha)| {
            let mut c = color(c)?;
            if *alpha != 1. {
                c[3] = *alpha as f32;
            }
            Ok::<_, String>(c)
        })
        .transpose()?;
    Ok(Draw {
        origin_mm,
        even_odd: true,
        path: path(&command.path, offset),
        stroke,
        fill,
        stroke_options: None,
        clip: command.clip.as_ref().map(|p| polygon(p, offset)),
        clip_intersection: None,
        text: None,
        flatten_transform: false,
    })
}
/// Partial painter integration: all visible unsupported content fails closed.
/// Does not mutate the scene, use the UI raster cache, or start a worker.
pub fn prepare(sheet: &Sheet) -> Result<Page, String> {
    prepare_with_views(sheet, &mut |_| {
        Err("PDF painter for this sheet item remains unavailable".into())
    })
}
/// Scene-aware caller supplies actual cached view ink, preserving source item order.
pub fn prepare_with_views<F>(sheet: &Sheet, views: &mut F) -> Result<Page, String>
where
    F: FnMut(&ingetrazo_core::composition::ViewFrame) -> Result<Vec<Draw>, String>,
{
    prepare_with_painters(sheet,views,&mut |_,_|Err("PDF painter for this sheet item remains unavailable".into()))
}
pub fn prepare_with_painters<F,T>(sheet:&Sheet,views:&mut F,texts:&mut T)->Result<Page,String>
where F:FnMut(&ingetrazo_core::composition::ViewFrame)->Result<Vec<Draw>,String>,
      T:FnMut(&Sheet,&ingetrazo_core::composition::TextItem)->Result<Vec<Draw>,String>
{
    for key in [
        "images",
        "leyendas",
        "cotas",
        "cotas_ang",
        "cotas_rad",
        "etiquetas",
        "perfiles",
        "niveles",
        "llamadas",
    ] {
        if sheet
            .native_document
            .get(key)
            .and_then(|v| v.as_array())
            .is_some_and(|a| {
                a.iter()
                    .any(|v| v.get("hidden").and_then(|v| v.as_bool()) != Some(true))
            })
        {
            return Err(format!("PDF painter for {key} remains unavailable"));
        }
    }
    if sheet
        .native_document
        .get("border")
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        return Err("PDF sheet border painter remains unavailable".into());
    }
    let mut draws = Vec::new();
    let mut path_commands = 0usize;
    for index in sheet.visible_item_indices() {
        match &sheet.items[index] {
            SheetItem::Shape(item) => {
                let offset = glam::DVec2::new(item.shape.x_mm, item.shape.y_mm);
                if draws.len() == crate::sheet_pdf::MAX_DRAWS {
                    return Err("PDF draw budget exceeded".into());
                }
                // Preserve the source per-shape conservative estimator. Using
                // the remaining aggregate capacity as its budget can reject a
                // valid last shape whose estimate exceeds its actual output.
                // Only this bounded one-shape painter is materialized at once.
                for command in item.shape.paint_commands(crate::sheet_pdf::MAX_DRAWS)? {
                    let next = draw(&command, offset)?;
                    if draws.len() == crate::sheet_pdf::MAX_DRAWS {
                        return Err("PDF draw budget exceeded".into());
                    }
                    path_commands = path_commands
                        .checked_add(next.path.len())
                        .and_then(|n| n.checked_add(next.clip.as_ref().map_or(0, Vec::len)))
                        .and_then(|n| n.checked_add(next.clip_intersection.as_ref().map_or(0, Vec::len)))
                        .ok_or("PDF command count overflow")?;
                    if path_commands > crate::sheet_pdf::MAX_COMMANDS {
                        return Err("PDF aggregate command budget exceeded".into());
                    }
                    draws.push(next);
                }
            }
            SheetItem::ViewFrame(frame) => {
                for next in views(frame)? {
                    if draws.len() == crate::sheet_pdf::MAX_DRAWS {
                        return Err("PDF draw budget exceeded".into());
                    }
                    path_commands = path_commands
                        .checked_add(next.path.len())
                        .and_then(|n| n.checked_add(next.clip.as_ref().map_or(0, Vec::len)))
                        .and_then(|n| n.checked_add(next.clip_intersection.as_ref().map_or(0, Vec::len)))
                        .ok_or("PDF command count overflow")?;
                    if path_commands > crate::sheet_pdf::MAX_COMMANDS {
                        return Err("PDF aggregate command budget exceeded".into());
                    }
                    draws.push(next);
                }
            }
            SheetItem::Text(item)=>{
                for next in texts(sheet,item)? {
                    if draws.len()==crate::sheet_pdf::MAX_DRAWS {return Err("PDF draw budget exceeded".into());}
                    path_commands=path_commands.checked_add(draw_command_count(&next)?).ok_or("PDF command count overflow")?;
                    if path_commands>crate::sheet_pdf::MAX_COMMANDS {return Err("PDF aggregate command budget exceeded".into());}
                    draws.push(next);
                }
            }
            _ => return Err("PDF painter for this sheet item remains unavailable".into()),
        }
    }
    let (w, h) = sheet.dimensions_mm();
    let size = [w, h];
    // Original standard QPageSize uses integer point dimensions and ten-point
    // default QPdfWriter margins. At source RENDER_DPI=300, Qt rounds
    // ten points to42device pixels (10.08points), observed in content matrices.
    Ok(Page {
        device_dpi: Some(300.),
        size_mm: size,
        media_box_points: Some(size.map(|v| (v * 72. / 25.4).round())),
        origin_mm: [42. * 25.4 / 300.; 2],
        draws,
    })
}

/// Admit an atlas while retaining at most the global writer budgets.
pub fn prepare_all(sheets: &[Sheet]) -> Result<Vec<Page>, String> {
    prepare_all_with_views(sheets, &mut |_| {
        Err("PDF painter for this sheet item remains unavailable".into())
    })
}
pub fn prepare_all_with_views<F>(sheets: &[Sheet], views: &mut F) -> Result<Vec<Page>, String>
where
    F: FnMut(&ingetrazo_core::composition::ViewFrame) -> Result<Vec<Draw>, String>,
{
    prepare_all_with_painters(sheets,views,&mut |_,_|Err("PDF painter for this sheet item remains unavailable".into()))
}
pub fn prepare_all_with_painters<F,T>(sheets:&[Sheet],views:&mut F,texts:&mut T)->Result<Vec<Page>,String>
where F:FnMut(&ingetrazo_core::composition::ViewFrame)->Result<Vec<Draw>,String>,
      T:FnMut(&Sheet,&ingetrazo_core::composition::TextItem)->Result<Vec<Draw>,String>
{
    if sheets.is_empty() || sheets.len() > 10_000 {
        return Err("PDF requires 1..10000 pages".into());
    }
    let mut pages = Vec::new();
    let mut draws = 0usize;
    let mut commands = 0usize;
    for sheet in sheets {
        let page = prepare_with_painters(sheet, views, texts)?;
        draws = draws
            .checked_add(page.draws.len())
            .ok_or("PDF draw count overflow")?;
        if draws > crate::sheet_pdf::MAX_DRAWS {
            return Err("PDF atlas draw budget exceeded".into());
        }
        for draw in &page.draws {
            commands = commands.checked_add(draw_command_count(draw)?).ok_or("PDF command count overflow")?;
        }
        if commands > crate::sheet_pdf::MAX_COMMANDS {
            return Err("PDF atlas command budget exceeded".into());
        }
        pages.push(page);
    }
    Ok(pages)
}

fn draw_command_count(draw:&Draw)->Result<usize,String>{
    let mut count=draw.path.len().checked_add(draw.clip.as_ref().map_or(0,Vec::len))
        .and_then(|n|n.checked_add(draw.clip_intersection.as_ref().map_or(0,Vec::len))).ok_or("PDF command count overflow")?;
    if let Some(text)=&draw.text {
        count=count.checked_add(text.glyphs.len().checked_mul(if text.bold {4}else{2}).ok_or("PDF glyph count overflow")?)
            .and_then(|n|n.checked_add(text.decorations.len().checked_mul(2)?)).ok_or("PDF command count overflow")?;
    }
    Ok(count)
}
