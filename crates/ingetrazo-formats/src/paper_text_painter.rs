// SPDX-License-Identifier: GPL-3.0-or-later
//! Physical source paper text painter consuming bounded, reusable native paragraph resources.
use crate::{
    paper_font_resolver::{Request, Resolver},
    paper_text_layout::{Alignment, ParagraphCache},
    sheet_pdf::{Draw, PathOp, TextDecoration, TextDraw, TextGlyph},
};
pub struct TextStyle<'a> {
    pub text: &'a str,
    pub origin_mm: [f64; 2],
    pub width_mm: f64,
    pub size_pt: f64,
    pub family: &'a str,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub alignment: Alignment,
    pub color: [f32; 4],
}
#[derive(Default)]
pub struct Painter {
    resolver: Option<Resolver>,
    cache: ParagraphCache,
}
impl Painter {
    /// Refresh is explicit; existing pages retain their immutable font resources.
    pub fn refresh_fonts(&mut self) -> Result<(), String> {
        if let Some(resolver) = &mut self.resolver {
            resolver.refresh()?;
        }
        self.cache.clear();
        Ok(())
    }
    pub fn stats(
        &self,
    ) -> (
        (usize, usize, usize, usize, u64),
        Option<(usize, u64, (usize, usize, u64))>,
    ) {
        (
            self.cache.stats(),
            self.resolver.as_ref().map(Resolver::stats),
        )
    }
    /// Source background metrics intentionally ignore italic/underline and use primary font spacing.
    pub fn draw_with_background(
        &mut self,
        style: &TextStyle<'_>,
        color: [f32; 4],
    ) -> Result<Vec<Draw>, String> {
        if color
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid paper text background".into());
        }
        let mut draws = self.draw(style)?;
        let size_mm = style.size_pt * 25.4 / 72.;
        let scale = size_mm / 100. * 0.75;
        let (y0, y1) = if style.text.is_empty() {
            (0., size_mm)
        } else {
            let request = Request {
                family: style.family.into(),
                bold: style.bold,
                italic: false,
                required_chars: vec![],
            };
            let resolver = self.resolver.as_mut().unwrap();
            let font = resolver.resolve(request.clone())?.font;
            let paragraph = self.cache.layout(
                resolver,
                &request,
                style.text,
                style.width_mm.max(0.5) / scale,
                Alignment::Left,
            )?;
            let ascent = font.ascent_26_6() as f64 / 64.;
            let descent = font.descent_26_6() as f64 / 64.;
            let height = ascent + descent;
            let spacing = height + font.leading_26_6() as f64 / 64.;
            let n = (paragraph.height_26_6 as f64 / 64. / height.max(1e-6))
                .round_ties_even()
                .max(1.);
            (
                (ascent - font.cap_height_26_6() as f64 / 64.) * scale - 0.25 * size_mm,
                (height + (n - 1.) * spacing) * scale + 0.05 * size_mm,
            )
        };
        let h = (y1 - y0).max(1.);
        let x0 = -0.5;
        let x1 = style.width_mm + 0.5;
        draws
            .try_reserve(1)
            .map_err(|_| "Paper background draw allocation failed")?;
        draws.insert(
            0,
            Draw {
                origin_mm: style.origin_mm,
                path: vec![
                    PathOp::Move([x0, y0]),
                    PathOp::Line([x1, y0]),
                    PathOp::Line([x1, y0 + h]),
                    PathOp::Line([x0, y0 + h]),
                    PathOp::Close,
                ],
                fill: Some(color),
                stroke: None,
                stroke_options: None,
                clip: None,
                clip_intersection: None,
                text: None,
                flatten_transform: false,
                even_odd: false,
            },
        );
        Ok(draws)
    }
    pub fn draw(&mut self, style: &TextStyle<'_>) -> Result<Vec<Draw>, String> {
        if style.family.len() > 1024
            || style.family.as_bytes().contains(&0)
            || style.text.len() > crate::paper_text_layout::MAX_LINE_BYTES
            || !style.size_pt.is_finite()
            || style.size_pt <= 0.
            || style.size_pt > 10000.
            || !style.width_mm.is_finite()
            || style.width_mm <= 0.
            || style.width_mm > 1_000_000.
            || style
                .origin_mm
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || style
                .color
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("Invalid physical paper text style".into());
        }
        if style.text.is_empty() {
            return Ok(vec![]);
        }
        if self.resolver.is_none() {
            self.resolver = Some(Resolver::new()?);
        }
        let request = Request {
            family: style.family.into(),
            bold: style.bold,
            italic: style.italic,
            required_chars: vec![],
        };
        let size_mm = style.size_pt * 25.4 / 72.;
        let scale = size_mm / 100. * 0.75;
        let height_mm =
            size_mm * 1.35 * (style.text.bytes().filter(|b| *b == b'\n').count() + 3) as f64;
        let paragraph = self.cache.layout(
            self.resolver.as_mut().unwrap(),
            &request,
            style.text,
            style.width_mm / scale,
            style.alignment,
        )?;
        let clip =
            if paragraph.height_26_6 as f64 / 64. * scale > height_mm || paragraph.exceeds_width {
                Some(vec![
                    PathOp::Move([0., 0.]),
                    PathOp::Line([style.width_mm, 0.]),
                    PathOp::Line([style.width_mm, height_mm]),
                    PathOp::Line([0., height_mm]),
                    PathOp::Close,
                ])
            } else {
                None
            };
        let mut draws = vec![];
        let mut count = 0usize;
        for line in &paragraph.lines {
            for run in &line.line.runs {
                count = count
                    .checked_add(run.glyphs.len())
                    .ok_or("Paper text glyph budget overflow")?;
                if count > crate::paper_text::MAX_GLYPHS
                    || draws.len() >= crate::sheet_pdf::MAX_DRAWS
                {
                    return Err("Paper text draw/glyph budget exceeded".into());
                }
                let mut x = (line.x_26_6 + run.x_26_6) as f64 / 64.;
                let start = x;
                let baseline = line.baseline_26_6 as f64 / 64.;
                let mut glyphs = vec![];
                glyphs
                    .try_reserve_exact(run.glyphs.len())
                    .map_err(|_| "Paper text glyph allocation failed")?;
                for glyph in &run.glyphs {
                    glyphs.push(TextGlyph {
                        glyph: glyph.id,
                        position_px: [
                            x + glyph.offset[0] as f64 / 64.,
                            baseline + glyph.offset[1] as f64 / 64.,
                        ],
                    });
                    x += glyph.advance[0] as f64 / 64.;
                }
                // QPdf emits RTL glyphs in logical order with negative relative advances.
                if run.rtl {
                    glyphs.reverse();
                }
                let mut decorations = vec![];
                if style.underline {
                    let [t, offset] = line.underline_metrics_px;
                    decorations.push(TextDecoration {
                        line_px: [
                            start.floor(),
                            baseline + offset,
                            x.floor(),
                            baseline + offset,
                        ],
                        width_px: t,
                    });
                }
                draws
                    .try_reserve(1)
                    .map_err(|_| "Paper text draw allocation failed")?;
                draws.push(Draw {
                    origin_mm: style.origin_mm,
                    text: Some(TextDraw {
                        font: run.font.clone(),
                        scale_mm: scale,
                        italic: style.italic,
                        bold: style.bold,
                        glyphs,
                        decorations,
                    }),
                    path: vec![],
                    stroke: None,
                    stroke_options: None,
                    fill: Some(style.color),
                    clip: clip.clone(),
                    clip_intersection: None,
                    even_odd: true,
                    flatten_transform: false,
                });
            }
        }
        Ok(draws)
    }
}

impl std::fmt::Debug for Painter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaperTextPainter")
            .field("resource_stats", &self.stats())
            .finish()
    }
}
