// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct physical-page vector output. Caller supplies validated item painters;
//! no rendering process, raster screenshot, or intermediate image is required.
use pdf_writer::{Name, Pdf, Rect, Ref};
use std::{io::Write, path::Path};
#[derive(Clone, Debug)]
pub enum PathOp {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([f64; 6]),
    Close,
}
#[derive(Clone, Debug)]
pub struct Draw {
    pub origin_mm: [f64; 2],
    pub even_odd: bool,
    pub path: Vec<PathOp>,
    pub stroke: Option<([f32; 4], f64)>,
    pub fill: Option<[f32; 4]>,
    pub clip: Option<Vec<PathOp>>,
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
pub(crate) const MAX_DRAWS: usize = 100_000;
pub(crate) const MAX_COMMANDS: usize = 500_000;
struct Number(f64);
fn number(value: f64) -> Number {
    Number(value)
}
impl std::fmt::Display for Number {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
fn path(content: &mut Vec<u8>, ops: &[PathOp]) {
    for op in ops {
        match *op {
            PathOp::Move([x, y]) => writeln!(content, "{} {} m", number(x), number(y)).unwrap(),
            PathOp::Line([x, y]) => writeln!(content, "{} {} l", number(x), number(y)).unwrap(),
            PathOp::Cubic(p) => writeln!(
                content,
                "{} {} {} {} {} {} c",
                number(p[0]),
                number(p[1]),
                number(p[2]),
                number(p[3]),
                number(p[4]),
                number(p[5])
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
    if pages.is_empty() || pages.len() > 10_000 {
        return Err("PDF requires 1..10000 pages".into());
    }
    let mut total = 0usize;
    let mut path_commands = 0usize;
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
            path_commands = path_commands
                .checked_add(draw.path.len())
                .and_then(|n| n.checked_add(draw.clip.as_ref().map_or(0, Vec::len)))
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
            for color in draw.fill.iter().chain(draw.stroke.iter().map(|(c, _)| c)) {
                if color
                    .iter()
                    .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
                {
                    return Err("Invalid PDF encoded color".into());
                }
            }
            if let Some((_, width)) = draw.stroke {
                if !width.is_finite() || width <= 0. || width > 10_000. {
                    return Err("Invalid physical PDF stroke width".into());
                }
            }
        }
    }
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let mut next = 3i32;
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
            writeln!(
                content,
                "q\n/{} gs\n{} 0 0 {} {} {} cm",
                names[i],
                number(local_scale),
                number(local_scale),
                number(draw.origin_mm[0] * local_scale),
                number(draw.origin_mm[1] * local_scale)
            )
            .unwrap();
            if let Some(clip) = &draw.clip {
                path(&mut content, clip);
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
                    "{} {} {} RG\n{} w\n1 J\n2 j",
                    number(c[0] as f64),
                    number(c[1] as f64),
                    number(c[2] as f64),
                    number(width)
                )
                .unwrap();
            }
            path(&mut content, &draw.path);
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
    }
    pdf.pages(tree).kids(ids).count(pages.len() as i32);
    Ok(pdf.finish())
}
/// Same-directory atomic replacement; invalid input never touches destination.
pub fn save(path: &Path, pages: &[Page]) -> Result<(), String> {
    let bytes = encode(pages)?;
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
