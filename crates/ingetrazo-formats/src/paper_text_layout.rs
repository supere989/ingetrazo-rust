// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed bounded visual font runs. Paragraph wrapping and composer item policy remain separate.
use crate::{
    paper_font::CachedFont,
    paper_font_resolver::{Request, Resolver},
    paper_text::Glyph,
};
use std::{ops::Range, rc::Rc};
pub const MAX_LINE_BYTES: usize = 64 * 1024;
pub const MAX_LINE_RUNS: usize = 4096;
#[derive(Debug, Clone)]
pub struct FontRun {
    pub font: Rc<CachedFont>,
    pub logical_bytes: Range<usize>,
    pub glyphs: Vec<Glyph>,
    pub x_26_6: i64,
}
#[derive(Debug, Clone)]
pub struct VisualLine {
    pub runs: Vec<FontRun>,
    pub advance_26_6: i64,
    pub ascent_26_6: i32,
    pub descent_26_6: i32,
}
/// Resolve only missing character spans; primary-font Latin/space glyphs retain their own face.
/// Resources are shared Rc owners from the bounded resolver, never full-font copies.
pub fn shape_line(
    resolver: &mut Resolver,
    request: &Request,
    text: &str,
) -> Result<VisualLine, String> {
    if text.len() > MAX_LINE_BYTES || text.contains('\n') || text.contains('\r') {
        return Err("Paper visual line domain exceeded".into());
    }
    let mut base_request = request.clone();
    base_request.required_chars.clear();
    let primary = resolver.resolve(base_request)?.font;
    let mut result = VisualLine {
        runs: vec![],
        advance_26_6: 0,
        ascent_26_6: primary.ascent_26_6(),
        descent_26_6: primary.descent_26_6(),
    };
    let bidi = unicode_bidi::BidiInfo::new(text, None);
    for paragraph in &bidi.paragraphs {
        let (levels, visual) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for range in visual {
            let rtl = levels[range.start].is_rtl();
            let mut spans: Vec<(Range<usize>, Rc<CachedFont>)> = vec![];
            for (offset, ch) in text[range.clone()].char_indices() {
                let start = range.start + offset;
                let font = if ch.is_control() || primary.covers_character(ch) {
                    primary.clone()
                } else {
                    let mut fallback = request.clone();
                    fallback.required_chars = vec![ch];
                    resolver.resolve(fallback)?.font
                };
                if let Some((previous, owner)) = spans.last_mut() {
                    if Rc::ptr_eq(owner, &font) {
                        previous.end = start + ch.len_utf8();
                        continue;
                    }
                }
                if spans.len() + result.runs.len() >= MAX_LINE_RUNS {
                    return Err("Paper visual run budget exceeded".into());
                }
                spans
                    .try_reserve(1)
                    .map_err(|_| "Paper visual run allocation failed")?;
                spans.push((start..start + ch.len_utf8(), font));
            }
            if rtl {
                spans.reverse();
            }
            result
                .runs
                .try_reserve(spans.len())
                .map_err(|_| "Paper visual run allocation failed")?;
            for (range, font) in spans {
                let glyphs = font.shape_run(&text[range.clone()], Some(rtl))?;
                let advance = glyphs
                    .iter()
                    .try_fold(0i64, |n, g| n.checked_add(i64::from(g.advance[0])))
                    .ok_or("Paper line advance overflow")?;
                result.ascent_26_6 = result.ascent_26_6.max(font.ascent_26_6());
                result.descent_26_6 = result.descent_26_6.max(font.descent_26_6());
                result.runs.push(FontRun {
                    font,
                    logical_bytes: range,
                    glyphs,
                    x_26_6: result.advance_26_6,
                });
                result.advance_26_6 = result
                    .advance_26_6
                    .checked_add(advance)
                    .ok_or("Paper line advance overflow")?;
            }
        }
    }
    Ok(result)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn mixed_fallback_keeps_primary_face_and_warm_ownership() {
        let mut resolver = Resolver::new().unwrap();
        let request = Request {
            family: "Sans Serif".into(),
            bold: false,
            italic: false,
            required_chars: vec![],
        };
        let primary = resolver.resolve(request.clone()).unwrap().font;
        let line = shape_line(&mut resolver, &request, "Office 平面図 AVATAR").unwrap();
        assert_eq!(line.runs.len(), 3);
        assert!(Rc::ptr_eq(&line.runs[0].font, &primary));
        assert!(Rc::ptr_eq(&line.runs[2].font, &primary));
        assert!(!Rc::ptr_eq(&line.runs[1].font, &primary));
        assert_eq!(line.ascent_26_6, 6675); // actual source 104.296875 px.
        assert_eq!(line.advance_26_6, 66002); // actual source 1031.28125 px.
        let before = resolver.stats();
        let warm = shape_line(&mut resolver, &request, "Office 平面図 AVATAR").unwrap();
        assert_eq!(before, resolver.stats());
        assert!(Rc::ptr_eq(&line.runs[1].font, &warm.runs[1].font));
        assert!(shape_line(&mut resolver, &request, "bad\nline").is_err());
        assert_eq!(before, resolver.stats());
    }
}
