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
const MAX_LAYOUT_FONTS: usize = 64;
const MAX_LAYOUT_FONT_BYTES: usize = 64 * 1024 * 1024;
fn admit_font(
    owners: &mut Vec<(crate::paper_font::FontKey, usize)>,
    font: &Rc<CachedFont>,
) -> Result<(), String> {
    if owners.iter().any(|(key, _)| *key == font.key()) {
        return Ok(());
    }
    let total = owners
        .iter()
        .try_fold(font.shared_bytes().len(), |n, (_, bytes)| {
            n.checked_add(*bytes)
        })
        .ok_or("Paper layout font bytes overflow")?;
    if owners.len() >= MAX_LAYOUT_FONTS || total > MAX_LAYOUT_FONT_BYTES {
        return Err("Paper layout aggregate font budget exceeded".into());
    }
    owners
        .try_reserve(1)
        .map_err(|_| "Paper layout font ownership allocation failed")?;
    owners.push((font.key(), font.shared_bytes().len()));
    Ok(())
}
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
    shape_line_with_level(resolver, request, text, None)
}
fn shape_line_with_level(
    resolver: &mut Resolver,
    request: &Request,
    text: &str,
    level: Option<unicode_bidi::Level>,
) -> Result<VisualLine, String> {
    if text.len() > MAX_LINE_BYTES || text.contains('\n') || text.contains('\r') {
        return Err("Paper visual line domain exceeded".into());
    }
    // QPainter drawText without TextExpandTabs replaces tabs by ordinary spaces.
    // Tabs are one UTF-8 byte, so logical byte offsets remain stable.
    let normalized;
    let text = if text.contains('\t') {
        normalized = text.replace('\t', " ");
        normalized.as_str()
    } else {
        text
    };
    let mut base_request = request.clone();
    base_request.required_chars.clear();
    let primary = resolver.resolve(base_request)?.font;
    let mut owners = vec![];
    admit_font(&mut owners, &primary)?;
    let mut result = VisualLine {
        runs: vec![],
        advance_26_6: 0,
        ascent_26_6: primary.ascent_26_6(),
        descent_26_6: primary.descent_26_6(),
    };
    let bidi = unicode_bidi::BidiInfo::new(text, level);
    for paragraph in &bidi.paragraphs {
        let (levels, visual) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for range in visual {
            let rtl = levels[range.start].is_rtl();
            let mut spans: Vec<(Range<usize>, Rc<CachedFont>, Option<u32>)> = vec![];
            for (offset, ch) in text[range.clone()].char_indices() {
                let start = range.start + offset;
                let font = if ch.is_control() || primary.covers_character(ch) {
                    primary.clone()
                } else {
                    let mut fallback = request.clone();
                    fallback.required_chars = vec![ch];
                    resolver.resolve(fallback)?.font
                };
                admit_font(&mut owners, &font)?;
                let script = unsafe {
                    harfbuzz_sys::hb_unicode_script(
                        harfbuzz_sys::hb_unicode_funcs_get_default(),
                        ch as u32,
                    )
                };
                let script = if script == harfbuzz_sys::HB_SCRIPT_COMMON
                    || script == harfbuzz_sys::HB_SCRIPT_INHERITED
                {
                    None
                } else {
                    Some(script)
                };
                if let Some((previous, owner, previous_script)) = spans.last_mut() {
                    if Rc::ptr_eq(owner, &font)
                        && (script.is_none()
                            || previous_script.is_none()
                            || *previous_script == script)
                    {
                        if previous_script.is_none() {
                            *previous_script = script;
                        }
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
                spans.push((start..start + ch.len_utf8(), font, script));
            }
            if rtl {
                spans.reverse();
            }
            result
                .runs
                .try_reserve(spans.len())
                .map_err(|_| "Paper visual run allocation failed")?;
            for (range, font, _) in spans {
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

/// Source fixed-pixel100 paragraph geometry. Positions remain fixed-point until PDF publication.
#[derive(Debug, Clone)]
pub struct ParagraphLine {
    pub line: VisualLine,
    pub x_26_6: i64,
    pub baseline_26_6: i64,
    /// Shared source decoration metrics across all font items in the line.
    pub underline_metrics_px: [f64; 2],
}
#[derive(Debug, Clone)]
pub struct Paragraph {
    pub lines: Vec<ParagraphLine>,
    pub height_26_6: i64,
    pub exceeds_width: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
/// Source WordWrap preserves initial whitespace and paragraph direction across wrapped lines.
/// Explicit shaping-work admission bounds repeated prefix measurement on adversarial text.
pub fn layout_paragraph(
    resolver: &mut Resolver,
    request: &Request,
    text: &str,
    width_px: f64,
    alignment: Alignment,
) -> Result<Paragraph, String> {
    if text.len() > MAX_LINE_BYTES
        || !width_px.is_finite()
        || width_px <= 0.
        || width_px > 1_000_000.
    {
        return Err("Paper paragraph domain exceeded".into());
    }
    let width = (width_px * 64.).floor() as i64;
    let mut result = Paragraph {
        lines: vec![],
        height_26_6: 0,
        exceeds_width: false,
    };
    let mut measured_bytes = 0usize;
    let mut owners = vec![];
    for paragraph in text.split('\n') {
        let paragraph = paragraph.strip_suffix('\r').unwrap_or(paragraph);
        let bidi = unicode_bidi::BidiInfo::new(paragraph, None);
        let level = bidi.paragraphs.first().map(|p| p.level);
        let mut start = 0;
        let mut end = 0;
        let mut ranges = vec![];
        let mut words = paragraph.char_indices().peekable();
        while let Some((word_start, ch)) = words.next() {
            if ch == ' ' || ch == '\t' {
                continue;
            }
            let mut word_end = word_start + ch.len_utf8();
            while let Some(&(offset, ch)) = words.peek() {
                if ch == ' ' || ch == '\t' {
                    break;
                }
                words.next();
                word_end = offset + ch.len_utf8();
            }
            let candidate = &paragraph[start..word_end];
            measured_bytes = measured_bytes
                .checked_add(candidate.len())
                .ok_or("Paper paragraph work overflow")?;
            if measured_bytes > 4 * 1024 * 1024 {
                return Err("Paper paragraph shaping work budget exceeded".into());
            }
            let advance = shape_line_with_level(resolver, request, candidate, level)?.advance_26_6;
            if end > start && advance > width {
                ranges.push(start..end);
                start = word_start;
            }
            end = word_end;
        }
        ranges.push(start..paragraph.len());
        for range in ranges {
            if result.lines.len() >= 4096 {
                return Err("Paper paragraph line budget exceeded".into());
            }
            let line = shape_line_with_level(
                resolver,
                request,
                paragraph[range].trim_end_matches([' ', '\t']),
                level,
            )?;
            for run in &line.runs {
                admit_font(&mut owners, &run.font)?;
            }
            let x = match alignment {
                Alignment::Left => 0,
                Alignment::Center => (width - line.advance_26_6).div_euclid(2),
                Alignment::Right => width - line.advance_26_6,
            };
            let baseline = result
                .height_26_6
                .checked_add(i64::from(line.ascent_26_6))
                .ok_or("Paper paragraph height overflow")?;
            let height =
                (i64::from(line.ascent_26_6) + i64::from(line.descent_26_6) + 63) / 64 * 64;
            result.height_26_6 = result
                .height_26_6
                .checked_add(height)
                .ok_or("Paper paragraph height overflow")?;
            result.exceeds_width |= line.advance_26_6 > width;
            result
                .lines
                .try_reserve(1)
                .map_err(|_| "Paper paragraph allocation failed")?;
            let mut underline_metrics_px = [0f64, 0f64];
            for run in &line.runs {
                let metrics = run.font.underline_metrics_px()?;
                underline_metrics_px[0] = underline_metrics_px[0].max(metrics[0]);
                underline_metrics_px[1] = underline_metrics_px[1].max(metrics[1]);
            }
            result.lines.push(ParagraphLine {
                line,
                x_26_6: x,
                baseline_26_6: baseline,
                underline_metrics_px,
            });
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ParagraphKey {
    primary: crate::paper_font::FontKey,
    request: Request,
    text: String,
    width_26_6: i64,
    alignment: Alignment,
}
struct ParagraphEntry {
    paragraph: Rc<Paragraph>,
    glyphs: usize,
    font_bytes: usize,
    fonts: usize,
}
/// Thread-confined bounded paragraph resources. Refresh changes the primary font identity,
/// so stale installed-font epochs cannot match; existing held paragraphs remain valid.
#[derive(Default)]
pub struct ParagraphCache {
    entries: std::collections::HashMap<ParagraphKey, ParagraphEntry>,
    order: std::collections::VecDeque<ParagraphKey>,
    glyphs: usize,
    font_bytes: usize,
    fonts: usize,
    builds: u64,
}
impl ParagraphCache {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.glyphs = 0;
        self.font_bytes = 0;
        self.fonts = 0;
    }
    pub fn stats(&self) -> (usize, usize, usize, usize, u64) {
        (
            self.entries.len(),
            self.order.len(),
            self.glyphs,
            self.font_bytes,
            self.builds,
        )
    }
    pub fn layout(
        &mut self,
        resolver: &mut Resolver,
        request: &Request,
        text: &str,
        width_px: f64,
        alignment: Alignment,
    ) -> Result<Rc<Paragraph>, String> {
        if text.len() > MAX_LINE_BYTES
            || !width_px.is_finite()
            || width_px <= 0.
            || width_px > 1_000_000.
        {
            return Err("Paper paragraph domain exceeded".into());
        }
        let mut base = request.clone();
        base.required_chars.clear();
        let primary = resolver.resolve(base.clone())?.font;
        let key = ParagraphKey {
            primary: primary.key(),
            request: base,
            text: text.into(),
            width_26_6: (width_px * 64.).floor() as i64,
            alignment,
        };
        if let Some(entry) = self.entries.get(&key) {
            return Ok(entry.paragraph.clone());
        }
        let paragraph = Rc::new(layout_paragraph(
            resolver, request, text, width_px, alignment,
        )?);
        let mut glyphs = 0usize;
        let mut owners = std::collections::HashMap::new();
        for line in &paragraph.lines {
            for run in &line.line.runs {
                glyphs = glyphs
                    .checked_add(run.glyphs.len())
                    .ok_or("Paper paragraph glyph count overflow")?;
                owners.insert(run.font.key(), run.font.shared_bytes().len());
            }
        }
        let font_bytes = owners
            .values()
            .try_fold(0usize, |n, v| n.checked_add(*v))
            .ok_or("Paper paragraph font count overflow")?;
        let fonts = owners.len();
        // A valid large layout is returned uncached; ownership always remains explicit.
        if glyphs <= 128 * 1024 && font_bytes <= 32 * 1024 * 1024 && fonts <= 16 {
            self.entries
                .try_reserve(1)
                .map_err(|_| "Paper paragraph cache allocation failed")?;
            self.order
                .try_reserve(1)
                .map_err(|_| "Paper paragraph cache allocation failed")?;
            while self.entries.len() >= 8
                || self.glyphs + glyphs > 128 * 1024
                || self.font_bytes + font_bytes > 32 * 1024 * 1024
                || self.fonts + fonts > 16
            {
                let old = self
                    .order
                    .pop_front()
                    .ok_or("Paper paragraph cache ownership inconsistent")?;
                let retired = self
                    .entries
                    .remove(&old)
                    .ok_or("Paper paragraph cache ownership inconsistent")?;
                self.glyphs -= retired.glyphs;
                self.font_bytes -= retired.font_bytes;
                self.fonts -= retired.fonts;
            }
            self.order.push_back(key.clone());
            self.entries.insert(
                key,
                ParagraphEntry {
                    paragraph: paragraph.clone(),
                    glyphs,
                    font_bytes,
                    fonts,
                },
            );
            self.glyphs += glyphs;
            self.font_bytes += font_bytes;
            self.fonts += fonts;
        }
        self.builds = self.builds.saturating_add(1);
        Ok(paragraph)
    }
}
#[cfg(all(test, target_os = "linux"))]
mod paragraph_tests {
    use super::*;
    #[test]
    fn warm_paragraph_resource_and_refresh_identity_are_bounded() {
        let mut resolver = Resolver::new().unwrap();
        let mut cache = ParagraphCache::default();
        let request = Request {
            family: "Sans Serif".into(),
            bold: false,
            italic: false,
            required_chars: vec![],
        };
        let held = cache
            .layout(
                &mut resolver,
                &request,
                "Office 平面図 several words العربية end",
                863.890625,
                Alignment::Left,
            )
            .unwrap();
        let stats = cache.stats();
        let fonts = resolver.stats();
        let warm = cache
            .layout(
                &mut resolver,
                &request,
                "Office 平面図 several words العربية end",
                863.890625,
                Alignment::Left,
            )
            .unwrap();
        assert!(Rc::ptr_eq(&held, &warm));
        assert_eq!(stats, cache.stats());
        assert_eq!(fonts, resolver.stats());
        assert!(cache
            .layout(
                &mut resolver,
                &request,
                "invalid",
                f64::NAN,
                Alignment::Left
            )
            .is_err());
        assert_eq!(stats, cache.stats());
        assert_eq!(fonts, resolver.stats());
        for i in 0..40 {
            cache
                .layout(
                    &mut resolver,
                    &request,
                    &format!("different paragraph {i}"),
                    863.890625,
                    Alignment::Left,
                )
                .unwrap();
        }
        let current = cache.stats();
        assert_eq!(current.0, current.1);
        assert!(current.0 <= 8);
        assert!(current.2 <= 128 * 1024);
        assert!(current.3 <= 32 * 1024 * 1024);
        let owner = held.lines[0].line.runs[0].font.clone();
        resolver.refresh().unwrap();
        let fresh = cache
            .layout(
                &mut resolver,
                &request,
                "Office 平面図 several words العربية end",
                863.890625,
                Alignment::Left,
            )
            .unwrap();
        assert!(!Rc::ptr_eq(&held, &fresh));
        assert_ne!(owner.key(), fresh.lines[0].line.runs[0].font.key());
        assert!(owner.shape_run("retained resource", None).is_ok());
        cache.clear();
        assert_eq!(cache.stats().0, 0);
        assert!(!held.lines.is_empty());
    }
}
