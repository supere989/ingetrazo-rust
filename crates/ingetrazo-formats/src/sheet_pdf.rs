// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct physical-page vector output. Caller supplies validated item painters;
//! no rendering process, raster screenshot, or intermediate image is required.
use pdf_writer::{
    types::{CidFontType, FontFlags, SystemInfo},
    Name, Pdf, Rect, Ref, Str,
};
use std::{io::Write, path::Path};
#[derive(Clone, Debug)]
pub enum PathOp {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([f64; 6]),
    Close,
}
#[derive(Clone, Debug)]
pub struct StrokeOptions {
    /// PDF cap: 0 flat, 1 round, 2 square.
    pub cap: u8,
    /// PDF join: 0 miter, 1 round, 2 bevel.
    pub join: u8,
    /// Physical paper millimetres, independently of stroke width.
    pub dash: Vec<f64>,
}
#[derive(Clone, Debug)]
pub struct TextGlyph {
    pub glyph: u32,
    pub position_px: [f64; 2],
}
#[derive(Clone, Debug)]
pub struct TextDecoration {
    pub line_px: [f64; 4],
    pub width_px: f64,
}
#[derive(Clone, Debug)]
pub struct TextDraw {
    pub rotation_deg: f64,
    pub font: std::rc::Rc<crate::paper_font::CachedFont>,
    /// Millimetres per source pixel100 layout unit.
    pub scale_mm: f64,
    pub italic: bool,
    pub bold: bool,
    pub glyphs: Vec<TextGlyph>,
    pub decorations: Vec<TextDecoration>,
}
#[derive(Clone, Debug)]
pub struct Draw {
    pub picture: Option<ingetrazo_core::paper_picture::Picture>,
    pub rotation_deg: f64,
    pub origin_mm: [f64; 2],
    pub text: Option<TextDraw>,
    pub even_odd: bool,
    pub path: Vec<PathOp>,
    pub stroke: Option<([f32; 4], f64)>,
    pub stroke_options: Option<StrokeOptions>,
    pub fill: Option<[f32; 4]>,
    pub clip: Option<Vec<PathOp>>,
    /// Second clip intersects the first in physical paper space (e.g. hatch ring and frame).
    pub clip_intersection: Option<Vec<PathOp>>,
    /// Source QPdfEngine nonopaque pen outlines use already mapped device coordinates.
    pub flatten_transform: bool,
}
pub struct Page {
    /// Source logical-device frame; paths remain physical mm. No rasterization.
    pub device_dpi: Option<f64>,
    pub size_mm: [f64; 2],
    pub media_box_points: Option<[f64; 2]>,
    pub origin_mm: [f64; 2],
    pub draws: Vec<Draw>,
}
// Emit finite physical coordinates without f32 narrowing. Fixed decimal PDF
// numbers avoid exponential notation and preserve authored double precision.
pub const MAX_DRAWS: usize = 100_000;
pub const MAX_COMMANDS: usize = 500_000;
struct Number(f64, bool);
impl std::fmt::Display for Number {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.1 {
            // Qt QPdfEngine qt_real_to_string (qtbase qpdf.cpp): truncate the
            // fractional part according to the integer digit count. Scoped to
            // original paper serialization; authored geometry remains f64.
            let negative = self.0 < 0.;
            let value = self.0.abs();
            let mut integer = value.trunc() as u32;
            let mut fraction = ((value - value.trunc()) * 1_000_000_000.) as u32;
            if fraction == 1_000_000_000 {
                integer += 1;
                fraction = 0;
            }
            let mut digits = 0;
            let mut count = integer;
            while count > 0 {
                digits += 1;
                count /= 10;
            }
            let mut factor = 100_000_000u32;
            for _ in 0..digits {
                factor /= 10;
                fraction /= 10;
            }
            if negative {
                out.write_str("-")?;
            }
            write!(out, "{integer}")?;
            if fraction != 0 {
                out.write_str(".")?;
                while factor > 0 {
                    write!(out, "{}", (fraction / factor) % 10)?;
                    factor /= 10;
                }
            }
            return Ok(());
        }
        // Every operand has passed bounded finite-domain admission. The largest
        // derived transform is <4e8; 64 bytes comfortably holds fixed .12 text.
        struct Buffer {
            bytes: [u8; 64],
            len: usize,
        }
        impl std::fmt::Write for Buffer {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                let end = self.len.checked_add(text.len()).ok_or(std::fmt::Error)?;
                if end > self.bytes.len() {
                    return Err(std::fmt::Error);
                }
                self.bytes[self.len..end].copy_from_slice(text.as_bytes());
                self.len = end;
                Ok(())
            }
        }
        let mut buffer = Buffer {
            bytes: [0; 64],
            len: 0,
        };
        std::fmt::write(&mut buffer, format_args!("{:.12}", self.0))?;
        while buffer.len > 0 && buffer.bytes[buffer.len - 1] == b'0' {
            buffer.len -= 1;
        }
        if buffer.len > 0 && buffer.bytes[buffer.len - 1] == b'.' {
            buffer.len -= 1;
        }
        let text = std::str::from_utf8(&buffer.bytes[..buffer.len]).map_err(|_| std::fmt::Error)?;
        out.write_str(if text == "-0" { "0" } else { text })
    }
}
fn path(
    content: &mut Vec<u8>,
    ops: &[PathOp],
    original_qt: bool,
    transform: Option<([f64; 2], f64)>,
) {
    let number = |v| Number(v, original_qt);
    let x = |v| number(transform.map_or(v, |(o, k)| v * k + o[0] * k));
    let y = |v| number(transform.map_or(v, |(o, k)| v * k + o[1] * k));
    for op in ops {
        match *op {
            PathOp::Move([xv, yv]) => writeln!(content, "{} {} m", x(xv), y(yv)).unwrap(),
            PathOp::Line([xv, yv]) => writeln!(content, "{} {} l", x(xv), y(yv)).unwrap(),
            PathOp::Cubic(p) => writeln!(
                content,
                "{} {} {} {} {} {} c",
                x(p[0]),
                y(p[1]),
                x(p[2]),
                y(p[3]),
                x(p[4]),
                y(p[5])
            )
            .unwrap(),
            PathOp::Close => content.extend_from_slice(b"h\n"),
        }
    }
}
fn validate_path(ops: &[PathOp]) -> Result<(), String> {
    if ops.len() > 1_000_000 {
        return Err("PDF path command budget exceeded".into());
    }
    let mut open = false;
    for op in ops {
        let points: &[f64] = match op {
            PathOp::Move(p) => {
                open = true;
                p
            }
            PathOp::Line(p) => {
                if !open {
                    return Err("PDF path starts without Move".into());
                }
                p
            }
            PathOp::Cubic(p) => {
                if !open {
                    return Err("PDF curve starts without Move".into());
                }
                p
            }
            PathOp::Close => {
                if !open {
                    return Err("PDF close starts without Move".into());
                }
                &[]
            }
        };
        if points
            .iter()
            .any(|p| !p.is_finite() || p.abs() > 1_000_000.)
        {
            return Err("PDF path coordinate exceeds domain".into());
        }
    }
    Ok(())
}
/// Validates every page before authoring. Millimetres use top-left coordinates.
/// Origin is explicit because original PDF and image export have different margins.
pub fn encode(pages: &[Page]) -> Result<Vec<u8>, String> {
    encode_policy(pages, false)
}
/// Original Qt paper numeral policy. Generic vector exports retain fixed12.
/// This affects serialization only, never Scene coordinates or general HLR.
pub fn encode_original_qt_paper(pages: &[Page]) -> Result<Vec<u8>, String> {
    encode_policy(pages, true)
}
fn encode_policy(pages: &[Page], original_qt: bool) -> Result<Vec<u8>, String> {
    let number = |v| Number(v, original_qt);
    if pages.is_empty() || pages.len() > 10_000 {
        return Err("PDF requires 1..10000 pages".into());
    }
    let mut total = 0usize;
    let mut path_commands = 0usize;
    let mut font_inputs: Vec<(std::rc::Rc<crate::paper_font::CachedFont>, Vec<u32>)> = Vec::new();
    let mut font_indices: std::collections::HashMap<crate::paper_font::FontKey, usize> =
        std::collections::HashMap::new();
    for page in pages {
        if page
            .device_dpi
            .is_some_and(|v| !v.is_finite() || v < 1. || v > 9600.)
        {
            return Err("Invalid PDF logical device DPI".into());
        }
        if page
            .size_mm
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || *v > 10_000.)
            || page
                .origin_mm
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 10_000.)
        {
            return Err("Invalid physical PDF page extent/origin".into());
        }
        if page
            .media_box_points
            .is_some_and(|p| p.iter().any(|v| !v.is_finite() || *v <= 0. || *v > 30_000.))
        {
            return Err("Invalid PDF media box".into());
        }
        total = total
            .checked_add(page.draws.len())
            .ok_or("PDF draw count overflow")?;
        if total > MAX_DRAWS {
            return Err("PDF draw budget exceeded".into());
        }
        for draw in &page.draws {
            if !draw.rotation_deg.is_finite()||draw.rotation_deg.abs()>1_000_000.||draw.rotation_deg!=0.&&(draw.flatten_transform||draw.text.is_some()){return Err("Invalid physical path rotation".into());}
            if let Some(text) = &draw.text {
                if !draw.path.is_empty()
                    || draw.stroke.is_some()
                    || draw.fill.is_none()
                    || draw.flatten_transform
                {
                    return Err("PDF text requires a separate fill-only text draw".into());
                }
                if !text.rotation_deg.is_finite() || text.rotation_deg.abs()>1_000_000. || !text.scale_mm.is_finite()
                    || text.scale_mm <= 0.
                    || text.scale_mm > 10_000.
                    || text.glyphs.len() > crate::paper_text::MAX_GLYPHS
                {
                    return Err("Invalid physical PDF text scale/glyph count".into());
                }
                for glyph in &text.glyphs {
                    if glyph.position_px.iter().enumerate().any(|(axis, v)| {
                        !v.is_finite()
                            || v.abs() > 1_000_000.
                            || (*v * text.scale_mm + draw.origin_mm[axis]).abs() > 1_000_000.
                    }) {
                        return Err("Invalid physical PDF text position".into());
                    }
                }
                path_commands = path_commands
                    .checked_add(
                        text.glyphs
                            .len()
                            .checked_mul(if text.bold { 4 } else { 2 })
                            .ok_or("PDF glyph count overflow")?,
                    )
                    .ok_or("PDF glyph count overflow")?;
                if path_commands > MAX_COMMANDS {
                    return Err("PDF aggregate command budget exceeded".into());
                }
                path_commands = path_commands
                    .checked_add(
                        text.decorations
                            .len()
                            .checked_mul(2)
                            .ok_or("PDF decoration count overflow")?,
                    )
                    .ok_or("PDF decoration count overflow")?;
                if path_commands > MAX_COMMANDS {
                    return Err("PDF aggregate command budget exceeded".into());
                }
                for d in &text.decorations {
                    if !d.width_px.is_finite() || d.width_px <= 0. || d.width_px > 10_000. {
                        return Err("Invalid physical PDF decoration width".into());
                    }
                    for (i, v) in d.line_px.iter().enumerate() {
                        if !v.is_finite()
                            || v.abs() > 1_000_000.
                            || (*v * text.scale_mm + draw.origin_mm[i % 2]).abs()
                                + d.width_px * text.scale_mm / 2.
                                > 1_000_000.
                        {
                            return Err("Invalid physical PDF decoration position".into());
                        }
                    }
                }
                let key = text.font.key();
                let index = if let Some(index) = font_indices.get(&key).copied() {
                    let old: &std::rc::Rc<crate::paper_font::CachedFont> = &font_inputs[index].0;
                    if !std::rc::Rc::ptr_eq(old, &text.font)
                        && old.shared_bytes().as_ref() != text.font.shared_bytes().as_ref()
                    {
                        return Err("PDF font resource identity collision".into());
                    }
                    index
                } else {
                    if font_inputs.len() >= 64 {
                        return Err("PDF font resource count exceeded".into());
                    }
                    font_indices
                        .try_reserve(1)
                        .map_err(|_| "PDF font resource allocation failed")?;
                    font_inputs
                        .try_reserve(1)
                        .map_err(|_| "PDF font resource allocation failed")?;
                    let index = font_inputs.len();
                    font_inputs.push((text.font.clone(), Vec::new()));
                    font_indices.insert(key, index);
                    index
                };
                let ids = &mut font_inputs[index].1;
                ids.try_reserve(text.glyphs.len())
                    .map_err(|_| "PDF glyph identity allocation failed")?;
                ids.extend(text.glyphs.iter().map(|g| g.glyph));
            }
            if draw.flatten_transform
                && (!original_qt || page.device_dpi.is_none() || draw.stroke.is_some())
            {
                return Err(
                    "Device-space PDF outlines require source paper policy and fill-only paths"
                        .into(),
                );
            }
            path_commands = path_commands
                .checked_add(draw.path.len())
                .and_then(|n| n.checked_add(draw.clip.as_ref().map_or(0, Vec::len)))
                .and_then(|n| n.checked_add(draw.clip_intersection.as_ref().map_or(0, Vec::len)))
                .ok_or("PDF command count overflow")?;
            if path_commands > MAX_COMMANDS {
                return Err("PDF aggregate command budget exceeded".into());
            }
            if draw
                .origin_mm
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            {
                return Err("Invalid PDF draw origin".into());
            }
            validate_path(&draw.path)?;
            if let Some(clip) = &draw.clip {
                validate_path(clip)?;
            }
            if let Some(clip) = &draw.clip_intersection {
                validate_path(clip)?;
            }
            for color in draw.fill.iter().chain(draw.stroke.iter().map(|(c, _)| c)) {
                if color
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
                {
                    return Err("Invalid PDF encoded color".into());
                }
            }
            if let Some(options) = &draw.stroke_options {
                if draw.stroke.is_none()
                    || options.cap > 2
                    || options.join > 2
                    || options.dash.len() > 32
                    || options
                        .dash
                        .iter()
                        .any(|v| !v.is_finite() || *v <= 0. || *v > 1_000_000.)
                {
                    return Err("Invalid physical PDF stroke options".into());
                }
            }
            if let Some((_, width)) = draw.stroke {
                if !width.is_finite() || width <= 0. || width > 10_000. {
                    return Err("Invalid physical PDF stroke width".into());
                }
            }
        }
    }
    // All page geometry is admitted before any cold font program work or destination I/O.
    let mut fonts = Vec::new();
    fonts
        .try_reserve_exact(font_inputs.len())
        .map_err(|_| "PDF font resource allocation failed")?;
    let mut font_bytes = 0usize;
    for (font, ids) in &font_inputs {
        let resource = font.subset(ids)?;
        font_bytes = font_bytes
            .checked_add(resource.program_bytes())
            .ok_or("PDF font byte count overflow")?;
        if font_bytes > 64 * 1024 * 1024 {
            return Err("PDF aggregate font byte budget exceeded".into());
        }
        fonts.push(resource);
    }
    for page in pages {
        for draw in &page.draws {
            if !draw.rotation_deg.is_finite()||draw.rotation_deg.abs()>1_000_000.||draw.rotation_deg!=0.&&(draw.flatten_transform||draw.text.is_some()){return Err("Invalid physical path rotation".into());}
            if let Some(text) = &draw.text {
                let resource = &fonts[font_indices[&text.font.key()]];
                let shear = text.font.synthetic_italic_shear(text.italic);
                for glyph in &text.glyphs {
                    let cid = resource
                        .cid(glyph.glyph)
                        .ok_or("PDF glyph absent from prepared font resource")?;
                    let b = resource.bounds_px[cid as usize];
                    let bold_offset = text.font.synthetic_bold_offset_px(text.bold)?;
                    for x in [b[0] - bold_offset, b[2]] {
                        for y in [b[1], b[3]] {
                            let px = draw.origin_mm[0]
                                + (glyph.position_px[0] + x + shear * y) * text.scale_mm;
                            let py = draw.origin_mm[1] + (glyph.position_px[1] - y) * text.scale_mm;
                            if !px.is_finite()
                                || !py.is_finite()
                                || px.abs() > 1_000_000.
                                || py.abs() > 1_000_000.
                            {
                                return Err(
                                    "PDF transformed glyph extent exceeds physical domain".into()
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let mut next = 3i32;
    let mut font_refs = Vec::new();
    for font in &fonts {
        let font_ref = Ref::new(next);
        write_font(&mut pdf, font_ref, font)?;
        font_refs.push(font_ref);
        next += 5;
    }
    let mut ids = Vec::new();
    pdf.catalog(catalog).pages(tree);
    for page in pages {
        let id = Ref::new(next);
        let stream = Ref::new(next + 1);
        next += 2;
        ids.push(id);
        let alpha_ids = page
            .draws
            .iter()
            .map(|_| {
                let id = Ref::new(next);
                next += 1;
                id
            })
            .collect::<Vec<_>>();
        let names = (0..page.draws.len())
            .map(|i| format!("G{i}"))
            .collect::<Vec<_>>();
        let mut content = Vec::new();
        let physical_unit = 72. / 25.4;
        let unit = page.device_dpi.map_or(physical_unit, |dpi| 72. / dpi);
        let local_scale = page.device_dpi.map_or(1., |dpi| dpi / 25.4);
        let box_size = page
            .media_box_points
            .unwrap_or(page.size_mm.map(|v| v * physical_unit));
        writeln!(
            content,
            "{} 0 0 {} {} {} cm",
            number(unit),
            number(-unit),
            number(page.origin_mm[0] * physical_unit),
            number(box_size[1] - page.origin_mm[1] * physical_unit)
        )
        .unwrap();
        for (i, draw) in page.draws.iter().enumerate() {
            if let Some(text) = &draw.text {
                let index = font_indices[&text.font.key()];
                let resource = &fonts[index];
                writeln!(content, "q\n/{} gs", names[i]).unwrap();
                // Clip in source logical device coordinates before the composed pixel100 text transform.
                for clip in [&draw.clip, &draw.clip_intersection].into_iter().flatten() {
                    path(
                        &mut content,
                        clip,
                        original_qt,
                        Some((draw.origin_mm, local_scale)),
                    );
                    content.extend_from_slice(if draw.even_odd { b"W* n\n" } else { b"W n\n" });
                }
                let scale = local_scale * text.scale_mm;
                if text.rotation_deg==0. {
                    writeln!(content,"{} 0 0 {} {} {} cm",number(scale),number(scale),number(draw.origin_mm[0]*local_scale),number(draw.origin_mm[1]*local_scale)).unwrap();
                }else{
                    let(sin,cos)=text.rotation_deg.to_radians().sin_cos();
                    writeln!(content,"{} {} {} {} {} {} cm",number(scale*cos),number(scale*sin),number(-scale*sin),number(scale*cos),number(draw.origin_mm[0]*local_scale),number(draw.origin_mm[1]*local_scale)).unwrap();
                }
                let c = draw.fill.unwrap();
                writeln!(
                    content,
                    "{} {} {} rg\nBT\n/F{} 100 Tf 1 0 {} -1 0 0 Tm",
                    number(c[0] as f64),
                    number(c[1] as f64),
                    number(c[2] as f64),
                    index,
                    number(text.font.synthetic_italic_shear(text.italic))
                )
                .unwrap();
                let shear = text.font.synthetic_italic_shear(text.italic);
                let mut previous = [0., 0.];
                for glyph in &text.glyphs {
                    let desired = [
                        glyph.position_px[0] + shear * glyph.position_px[1],
                        -glyph.position_px[1],
                    ];
                    let cid = resource
                        .cid(glyph.glyph)
                        .ok_or("PDF glyph absent from prepared font resource")?;
                    writeln!(
                        content,
                        "{} {} Td <{:04X}> Tj",
                        number(desired[0] - previous[0]),
                        number(desired[1] - previous[1]),
                        cid
                    )
                    .unwrap();
                    previous = desired;
                }
                let bold_offset = text.font.synthetic_bold_offset_px(text.bold)?;
                if bold_offset > 0. && !text.glyphs.is_empty() {
                    writeln!(content, "1 0 {} -1 0 0 Tm\n/Span << /ActualText <> >> BDC", number(shear)).unwrap();
                    let mut previous = [bold_offset, 0.];
                    for glyph in &text.glyphs {
                        let desired = [glyph.position_px[0] + shear * glyph.position_px[1], -glyph.position_px[1]];
                        let cid = resource.cid(glyph.glyph).ok_or("PDF glyph absent from prepared font resource")?;
                        writeln!(content, "{} {} Td <{:04X}> Tj", number(desired[0]-previous[0]), number(desired[1]-previous[1]), cid).unwrap();
                        previous = desired;
                    }
                    content.extend_from_slice(b"EMC\n");
                }
                content.extend_from_slice(b"ET\n");
                for d in &text.decorations {
                    writeln!(
                        content,
                        "{} {} {} RG\n{} w 0 J 2 j [] 0 d\n{} {} m\n{} {} l\nS",
                        number(c[0] as f64),
                        number(c[1] as f64),
                        number(c[2] as f64),
                        number(d.width_px),
                        number(d.line_px[0]),
                        number(d.line_px[1]),
                        number(d.line_px[2]),
                        number(d.line_px[3])
                    )
                    .unwrap();
                }
                content.extend_from_slice(b"Q\n");
                continue;
            }
            let flatten = draw.flatten_transform && original_qt;
            let transform = flatten.then_some((draw.origin_mm, local_scale));
            if draw.rotation_deg==0.{
            writeln!(
                content,
                "q\n/{} gs\n{} 0 0 {} {} {} cm",
                names[i],
                number(if flatten { 1. } else { local_scale }),
                number(if flatten { 1. } else { local_scale }),
                number(if flatten {
                    0.
                } else {
                    draw.origin_mm[0] * local_scale
                }),
                number(if flatten {
                    0.
                } else {
                    draw.origin_mm[1] * local_scale
                })
            )
            .unwrap();
            }else{
                let(sin,cos)=draw.rotation_deg.to_radians().sin_cos();
                writeln!(content,"q\n/{} gs\n{} {} {} {} {} {} cm",names[i],number(local_scale*cos),number(local_scale*sin),number(-local_scale*sin),number(local_scale*cos),number(draw.origin_mm[0]*local_scale),number(draw.origin_mm[1]*local_scale)).unwrap();
            }
            if let Some(clip) = &draw.clip {
                path(&mut content, clip, original_qt, transform);
                content.extend_from_slice(if draw.even_odd { b"W* n\n" } else { b"W n\n" });
            }
            if let Some(clip) = &draw.clip_intersection {
                path(&mut content, clip, original_qt, transform);
                content.extend_from_slice(if draw.even_odd { b"W* n\n" } else { b"W n\n" });
            }
            if let Some(c) = draw.fill {
                writeln!(
                    content,
                    "{} {} {} rg",
                    number(c[0] as f64),
                    number(c[1] as f64),
                    number(c[2] as f64)
                )
                .unwrap();
            }
            if let Some((c, width)) = draw.stroke {
                writeln!(
                    content,
                    "{} {} {} RG\n{} w",
                    number(c[0] as f64),
                    number(c[1] as f64),
                    number(c[2] as f64),
                    number(width)
                )
                .unwrap();
            }
            if draw.stroke.is_some() {
                if let Some(options) = &draw.stroke_options {
                    writeln!(content, "{} J\n{} j", options.cap, options.join).unwrap();
                    if !options.dash.is_empty() {
                        content.extend_from_slice(b"[");
                        for (i, dash) in options.dash.iter().enumerate() {
                            if i > 0 {
                                content.push(b' ');
                            }
                            write!(content, "{}", number(*dash)).unwrap();
                        }
                        content.extend_from_slice(b"] 0 d\n");
                    }
                } else {
                    content.extend_from_slice(b"1 J\n2 j\n");
                }
            }
            path(&mut content, &draw.path, original_qt, transform);
            content.extend_from_slice(
                match (draw.fill.is_some(), draw.stroke.is_some(), draw.even_odd) {
                    (true, true, true) => b"B*\n",
                    (true, false, true) => b"f*\n",
                    (true, true, false) => b"B\n",
                    (true, false, false) => b"f\n",
                    (false, true, _) => b"S\n",
                    _ => b"n\n",
                },
            );
            content.extend_from_slice(b"Q\n");
        }
        pdf.stream(stream, &content);
        for (i, draw) in page.draws.iter().enumerate() {
            pdf.ext_graphics(alpha_ids[i])
                .stroking_alpha(draw.stroke.map_or(1., |(c, _)| c[3]))
                .non_stroking_alpha(draw.fill.map_or(1., |c| c[3]));
        }
        let mut writer = pdf.page(id);
        writer
            .parent(tree)
            .media_box(Rect::new(0., 0., box_size[0] as f32, box_size[1] as f32))
            .contents(stream);
        let mut resources = writer.resources();
        let mut states = resources.ext_g_states();
        for (i, id) in alpha_ids.iter().enumerate() {
            states.pair(Name(names[i].as_bytes()), *id);
        }
        drop(states);
        if page.draws.iter().any(|d| d.text.is_some()) {
            let mut font_resources = resources.fonts();
            for (index, font_ref) in font_refs.iter().enumerate() {
                font_resources.pair(Name(format!("F{index}").as_bytes()), *font_ref);
            }
        }
    }
    pdf.pages(tree).kids(ids).count(pages.len() as i32);
    Ok(pdf.finish())
}
fn write_font(pdf: &mut Pdf, id: Ref, font: &crate::paper_font::PdfFont) -> Result<(), String> {
    let raw = id.get();
    let descendant = Ref::new(raw + 1);
    let descriptor = Ref::new(raw + 2);
    let program = Ref::new(raw + 3);
    let unicode = Ref::new(raw + 4);
    let name = Name(&font.metadata.name);
    pdf.type0_font(id)
        .base_font(name)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(descendant)
        .to_unicode(unicode);
    {
        let mut writer = pdf.cid_font(descendant);
        writer
            .subtype(CidFontType::Type2)
            .base_font(name)
            .system_info(SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            })
            .font_descriptor(descriptor)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        writer.widths().consecutive(0, font.widths.iter().copied());
    }
    let m = &font.metadata;
    pdf.font_descriptor(descriptor)
        .name(name)
        .flags(FontFlags::SYMBOLIC)
        .bbox(Rect::new(m.bbox[0], m.bbox[1], m.bbox[2], m.bbox[3]))
        .italic_angle(m.italic)
        .ascent(m.ascent)
        .descent(m.descent)
        .cap_height(m.ascent)
        .stem_v(m.stem)
        .font_file2(program);
    pdf.stream(program, &font.program)
        .insert(Name(b"Length1"))
        .primitive(font.program.len() as i32);
    // Literal source reverse cmap policy, including source glyph0 -> Unicode0.
    let mut cmap = Vec::new();
    cmap.extend_from_slice(b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    let mappings = font
        .unicode
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.map(|c| (i, c)))
        .collect::<Vec<_>>();
    for group in mappings.chunks(100) {
        writeln!(cmap, "{} beginbfchar", group.len()).unwrap();
        for (cid, scalar) in group {
            writeln!(cmap, "<{:04X}> <{:04X}>", cid, scalar).unwrap();
        }
        cmap.extend_from_slice(b"endbfchar\n");
    }
    cmap.extend_from_slice(b"endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    pdf.stream(unicode, &cmap);
    Ok(())
}
/// Same-directory atomic replacement; invalid input never touches destination.
pub fn save(path: &Path, pages: &[Page]) -> Result<(), String> {
    let bytes = encode(pages)?;
    save_bytes(path, &bytes)
}
pub fn save_original_qt_paper(path: &Path, pages: &[Page]) -> Result<(), String> {
    let bytes = encode_original_qt_paper(pages)?;
    save_bytes(path, &bytes)
}
fn save_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = parent.join(format!(".ingetrazo-sheet-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
