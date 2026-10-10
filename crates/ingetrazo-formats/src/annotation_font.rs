//! Resolved-size immutable annotation faces, sharing the source visual-run layout.
//! Cached font bytes are borrowed from the resolver; no per-glyph font copy.
use crate::{
    paper_font::FontKey,
    paper_font_resolver::{Request, Resolver},
    paper_text::{Glyph, PaperFont, RasterGlyph},
    paper_text_layout::{LayoutFont, VisualLine},
};
use std::{collections::HashMap, rc::Rc, sync::Arc};
pub struct Face {
    key: FontKey,
    pixels: u32,
    font: PaperFont,
}
impl Face {
    pub fn pixels(&self) -> u32 {
        self.pixels
    }
    pub fn rasterize(
        &self,
        glyph: u32,
        density: f64,
        phase: [u8; 2],
    ) -> Result<RasterGlyph, String> {
        self.font.rasterize_glyph(glyph, density, phase, 0.)
    }
}
impl LayoutFont for Face {
    fn key(&self) -> FontKey {
        self.key
    }
    fn shared_bytes(&self) -> &Arc<[u8]> {
        self.font.shared_bytes()
    }
    fn covers_character(&self, ch: char) -> bool {
        self.font.covers_character(ch)
    }
    fn shape_run(&self, text: &str, rtl: Option<bool>) -> Result<Vec<Glyph>, String> {
        self.font.shape_run(text, rtl)
    }
    fn ascent_26_6(&self) -> i32 {
        self.font.ascent_26_6
    }
    fn descent_26_6(&self) -> i32 {
        self.font.descent_26_6
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub faces: usize,
    pub retained_font_bytes: usize,
}
pub struct Cache {
    resolver: Resolver,
    faces: HashMap<(FontKey, u32), Rc<Face>>,
    limits: Limits,
    bytes: usize,
    builds: u64,
}
impl Cache {
    pub fn new(limits: Limits) -> Result<Self, String> {
        if limits.faces == 0 || limits.retained_font_bytes == 0 {
            return Err("Annotation font budget must be positive".into());
        }
        Ok(Self {
            resolver: Resolver::new()?,
            faces: HashMap::new(),
            limits,
            bytes: 0,
            builds: 0,
        })
    }
    pub fn stats(&self) -> (usize, usize, u64) {
        (self.faces.len(), self.bytes, self.builds)
    }
    pub fn resolve(&mut self, request: Request, pixels: u32) -> Result<Rc<Face>, String> {
        if pixels == 0 || pixels > 4096 {
            return Err("Annotation font pixel size outside admission".into());
        }
        let matched = self.resolver.resolve(request)?;
        let key = (matched.font.key(), pixels);
        if let Some(face) = self.faces.get(&key) {
            return Ok(face.clone());
        }
        let bytes = matched.font.shared_bytes().len();
        let next = self
            .bytes
            .checked_add(bytes)
            .filter(|&n| n <= self.limits.retained_font_bytes)
            .ok_or("Annotation retained font bytes exceeded")?;
        if self.faces.len() >= self.limits.faces {
            return Err("Annotation face count exceeded".into());
        }
        self.faces
            .try_reserve(1)
            .map_err(|_| "Annotation face cache allocation failed")?;
        let font = PaperFont::new_at_pixel_size(
            matched.font.shared_bytes().clone(),
            matched.index,
            Some(matched.policy),
            pixels,
        )?;
        let face = Rc::new(Face {
            key: key.0,
            pixels,
            font,
        });
        self.faces.insert(key, face.clone());
        self.bytes = next;
        self.builds += 1;
        Ok(face)
    }
    pub fn shape(
        &mut self,
        request: &Request,
        text: &str,
        pixels: u32,
    ) -> Result<VisualLine<Face>, String> {
        crate::paper_text_layout::shape_line_with_provider(request, text, |request| {
            self.resolve(request, pixels)
        })
    }
    /// Retire only cache-only faces. Live plans keep immutable glyph/font owners.
    pub fn retire_unused(&mut self) {
        self.faces.retain(|_, face| {
            if Rc::strong_count(face) == 1 {
                self.bytes -= face.shared_bytes().len();
                false
            } else {
                true
            }
        });
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn actual_annotation_metrics_and_warm_face_ownership() {
        let oracle: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/native-annotation-source/original.json"
        ))
        .unwrap();
        assert_eq!(
            oracle["source_pin"],
            "6be29fe437a7cd992d4e079f71720821b25216b9"
        );
        let mut cache = Cache::new(Limits {
            faces: 16,
            retained_font_bytes: 32 * 1024 * 1024,
        })
        .unwrap();
        let mut advances = 0;
        for row in oracle["cases"].as_array().unwrap() {
            for metric in row["font_metrics"].as_array().unwrap() {
                let info = &metric["font_info"];
                let request = Request {
                    family: info["family"].as_str().unwrap().into(),
                    bold: info["weight"].as_u64().unwrap() >= 700,
                    italic: false,
                    required_chars: vec![],
                };
                let pixels = info["pixel_size"].as_u64().unwrap() as u32;
                for advance in metric["advances"].as_array().unwrap() {
                    let text = advance["text"].as_str().unwrap();
                    let line = cache.shape(&request, text, pixels).unwrap();
                    assert_eq!(
                        (line.advance_26_6 as f64 / 64. + 0.5).floor() as i64,
                        advance["value"].as_i64().unwrap()
                    );
                    assert_eq!(
                        (line.ascent_26_6 + 63) / 64,
                        metric["ascent"].as_i64().unwrap() as i32
                    );
                    assert_eq!(
                        (line.descent_26_6 + 63) / 64,
                        metric["descent"].as_i64().unwrap() as i32
                    );
                    let before = cache.stats();
                    let warm = cache.shape(&request, text, pixels).unwrap();
                    assert_eq!(cache.stats(), before);
                    for (a, b) in line.runs.iter().zip(&warm.runs) {
                        assert!(Rc::ptr_eq(&a.font, &b.font));
                        assert_eq!(a.glyphs, b.glyphs);
                    }
                    advances += 1;
                }
            }
        }
        assert_eq!(advances, 12);
    }
    #[test]
    fn refused_candidate_keeps_live_font_and_unused_retirement_is_bounded() {
        let mut cache = Cache::new(Limits {
            faces: 1,
            retained_font_bytes: 32 * 1024 * 1024,
        })
        .unwrap();
        let request = Request {
            family: "DejaVu Sans".into(),
            bold: true,
            italic: false,
            required_chars: vec![],
        };
        let retained = cache.resolve(request.clone(), 15).unwrap();
        let counts = cache.stats();
        assert!(cache.resolve(request.clone(), 16).is_err());
        assert_eq!(cache.stats(), counts);
        assert!(Rc::ptr_eq(
            &retained,
            &cache.resolve(request.clone(), 15).unwrap()
        ));
        cache.retire_unused();
        assert_eq!(cache.stats(), counts);
        drop(retained);
        cache.retire_unused();
        assert_eq!((cache.stats().0, cache.stats().1), (0, 0));
        assert!(cache.resolve(request, 16).is_ok());
    }
}
