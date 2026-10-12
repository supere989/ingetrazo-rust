// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, thread-confined paper fonts and immutable direct-PDF resources.
use crate::{
    paper_font_subset as subset,
    paper_text::{FtPolicy, Glyph, PaperFont},
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
};
use uuid::Uuid;
pub const MAX_CACHED_FONTS: usize = 16;
pub const MAX_CACHED_FONT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CACHED_SUBSETS: usize = 8;
pub const MAX_CACHED_SUBSET_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontKey {
    pub identity: Uuid,
    pub face_index: u32,
    pub policy: FtPolicy,
}
#[derive(Debug, Clone)]
pub(crate) struct Metadata {
    pub name: Vec<u8>,
    pub bbox: [f32; 4],
    pub ascent: f32,
    pub descent: f32,
    pub stem: f32,
    pub italic: f32,
}
/// Identity cannot be forged by callers. Glyph maps/programs are validated before publication.
pub struct PdfFont {
    id: Uuid,
    pub(crate) program: Arc<[u8]>,
    pub(crate) cids: BTreeMap<u32, u16>,
    pub(crate) widths: Vec<f32>,
    pub(crate) metadata: Metadata,
    pub(crate) unicode: Vec<Option<u16>>,
    pub(crate) bounds_px: Vec<[f64; 4]>,
}
impl std::fmt::Debug for PdfFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfFont")
            .field("id", &self.id)
            .field("bytes", &self.program.len())
            .field("glyphs", &self.cids.len())
            .finish()
    }
}
impl PdfFont {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn cid(&self, glyph: u32) -> Option<u16> {
        self.cids.get(&glyph).copied()
    }
    pub fn program_bytes(&self) -> usize {
        self.program.len()
    }
    pub fn glyph_count(&self) -> usize {
        self.widths.len()
    }
    pub fn unicode_for_cid(&self, cid: u16) -> Option<u16> {
        self.unicode.get(cid as usize).copied().flatten()
    }
}
struct SubsetCache {
    entries: HashMap<Vec<u32>, Arc<PdfFont>>,
    order: VecDeque<Vec<u32>>,
    bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontStats {
    pub glyphs: usize,
    pub points: usize,
    pub subset_entries: usize,
    pub subset_bytes: usize,
    pub builds: u64,
}
pub struct CachedFont {
    key: FontKey,
    shaper: PaperFont,
    #[cfg(target_os="macos")]native:Option<Rc<crate::paper_coretext_glyph::NativeFont>>,
    geometry: RefCell<subset::GlyphStore>,
    subsets: RefCell<SubsetCache>,
    reverse: Vec<Option<u16>>,
    metadata: Metadata,
    italic: bool,
    bold: bool,
    cap_height_26_6: i32,
    builds: Cell<u64>,
}
impl std::fmt::Debug for CachedFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedFont")
            .field("key", &self.key)
            .field("bytes", &self.shared_bytes().len())
            .finish()
    }
}
impl CachedFont {
    #[cfg(target_os="macos")]
    pub(crate) fn native_font(&self)->Option<&Rc<crate::paper_coretext_glyph::NativeFont>>{self.native.as_ref()}
    pub fn key(&self) -> FontKey {
        self.key
    }
    fn new(key: FontKey, bytes: Arc<[u8]>) -> Result<Self, String> {
        Self::new_backend(key,bytes,#[cfg(target_os="macos")]None)
    }
    fn new_backend(key:FontKey,bytes:Arc<[u8]>,#[cfg(target_os="macos")]native:Option<Rc<crate::paper_coretext_glyph::NativeFont>>)->Result<Self,String>{
        if bytes.is_empty() || bytes.len() > crate::paper_text::MAX_FONT_BYTES {
            return Err("Paper font byte budget exceeded".into());
        }
        let face =
            ttf_parser::Face::parse(&bytes, key.face_index).map_err(|_| "Invalid paper font")?;
        if !face.is_subsetting_allowed()
            || !face.is_outline_embedding_allowed()
            || matches!(
                face.permissions(),
                Some(ttf_parser::Permissions::Restricted)
            )
        {
            return Err("Paper font requires an unavailable embedding policy".into());
        }
        let em = f32::from(face.units_per_em());
        let scale = 1000. / em;
        let bbox = face.global_bounding_box();
        let bbox = [
            bbox.x_min as f32 * scale,
            bbox.y_min as f32 * scale,
            bbox.x_max as f32 * scale,
            bbox.y_max as f32 * scale,
        ];
        let name = face
            .names()
            .into_iter()
            .find_map(crate::paper_font_resolver::sfnt_postscript_name)
            .unwrap_or_else(|| "NativePaperFont".into());
        if name.len() > 256 || name.is_empty() {
            return Err("Invalid paper font resource name".into());
        }
        let metadata = Metadata {
            name: name.into_bytes(),
            bbox,
            ascent: face.ascender() as f32 * scale,
            descent: face.descender() as f32 * scale,
            stem: face
                .underline_metrics()
                .map_or(44., |v| v.thickness as f32 * scale),
            italic: face.italic_angle(),
        };
        if metadata
            .bbox
            .iter()
            .chain([
                &metadata.ascent,
                &metadata.descent,
                &metadata.stem,
                &metadata.italic,
            ])
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        {
            return Err("Invalid paper font resource metrics".into());
        }
        let italic = face.is_italic();
        let bold = face.is_bold();
        #[cfg(target_os="macos")]let (italic,bold)=native.as_ref().map_or((italic,bold),|n|(n.traits&1!=0,n.traits&2!=0));
        let count = face.number_of_glyphs() as usize;
        let mut reverse = Vec::new();
        reverse
            .try_reserve_exact(count)
            .map_err(|_| "Paper unicode map allocation failed")?;
        reverse.resize(count, None);
        // Literal source reverseMap policy: lowest nonzero BMP cmap character per glyph.
        for u in 1..=u16::MAX {
            if let Some(ch) = char::from_u32(u32::from(u)) {
                if let Some(g) = face.glyph_index(ch) {
                    if let Some(slot) = reverse.get_mut(g.0 as usize) {
                        if slot.is_none() {
                            *slot = Some(u);
                        }
                    }
                }
            }
        }
        #[cfg(not(target_os="macos"))]
        let shaper=PaperFont::new_with_outline_policy(bytes.clone(),key.face_index,Some(key.policy))?;
        #[cfg(target_os="macos")]
        let shaper=if let Some(n)=&native{PaperFont::new_coretext(n)?}else{PaperFont::new_with_outline_policy(bytes.clone(),key.face_index,Some(key.policy))?};
        let cap_height_26_6=shaper.cap_height_26_6(face.capital_height());
        #[cfg(not(target_os="macos"))]let cap_height_26_6=cap_height_26_6?;
        #[cfg(target_os="macos")]let cap_height_26_6=if let Some(n)=&native{n.metrics()[3]}else{cap_height_26_6?};
        #[cfg(not(target_os="macos"))]let geometry=subset::GlyphStore::new(bytes,key.face_index)?;
        #[cfg(target_os="macos")]let geometry=if let Some(n)=&native{subset::GlyphStore::new_coretext(n.clone())?}else{subset::GlyphStore::new(bytes,key.face_index)?};
        Ok(Self {
            key,
            shaper,
            #[cfg(target_os="macos")]native,
            geometry: RefCell::new(geometry),
            subsets: RefCell::new(SubsetCache {
                entries: HashMap::new(),
                order: VecDeque::new(),
                bytes: 0,
            }),
            reverse,
            metadata,
            italic,
            bold,
            cap_height_26_6,
            builds: Cell::new(0),
        })
    }
    pub fn shape_run(&self, text: &str, direction: Option<bool>) -> Result<Vec<Glyph>, String> {
        self.shaper.shape_run(text, direction)
    }
    pub fn covers_character(&self, ch: char) -> bool {
        self.shaper.covers_character(ch)
    }
    pub fn cap_height_26_6(&self) -> i32 {
        self.cap_height_26_6
    }
    pub fn ascent_26_6(&self) -> i32 {
        self.shaper.ascent_26_6
    }
    pub fn descent_26_6(&self) -> i32 {
        self.shaper.descent_26_6
    }
    pub fn leading_26_6(&self) -> i32 {
        self.shaper.leading_26_6
    }
    /// Source QPainter antialiased underline: ceil FT offset plus half thickness, capped at descent.
    pub fn underline_metrics_px(&self) -> Result<[f64; 2], String> {
        let [thickness, offset] = self.decoration_metrics()?;
        let thickness = f64::from(thickness) / 64.;
        let offset = f64::from(offset) / 64.;
        let descent = f64::from(self.descent_26_6()) / 64.;
        let mut adjusted = offset.ceil() + thickness / 2.;
        if offset <= descent {
            adjusted = adjusted.min(descent - thickness / 2.);
        }
        Ok([thickness, adjusted])
    }
    /// Immutable HarfBuzz shaping context; zero means unspecified.
    pub fn shaping_point_size(&self) -> f32 { self.shaper.shaping_point_size() }
    /// Source QPdf synthetic-bold second pass: shift left by half native line thickness.
    pub fn synthetic_bold_offset_px(&self, requested: bool) -> Result<f64, String> {
        if requested && !self.bold {
            Ok(f64::from(self.decoration_metrics()?[0]) / 128.)
        } else {
            Ok(0.)
        }
    }
    pub fn synthetic_italic_shear(&self, requested: bool) -> f64 {
        if requested && !self.italic {
            0.3
        } else {
            0.
        }
    }
    fn decoration_metrics(&self)->Result<[i32;2],String>{
        #[cfg(target_os="macos")]if let Some(n)=&self.native{return Ok(n.decoration())}
        self.shaper.decoration_metrics_26_6()
    }
    pub fn supports_synthetic_bold_rasterization(&self) -> bool {
        #[cfg(target_os="macos")] { self.native.is_some() }
        #[cfg(not(target_os="macos"))] { false }
    }
    /// Native engine phase policy is established by bounded cold raster probes.
    /// Zero means an empty outline and leaves engine acquisition pending.
    pub fn raster_phase_count(&self,glyph:u32,scale:f64,bold:bool,italic:bool)->Result<u8,String>{
        #[cfg(target_os="macos")]if let Some(n)=&self.native {
            let shear=if italic&&!self.italic{crate::paper_coretext_glyph::synthetic_italic_shear()}else{0.};
            return n.raster_phase_count(glyph,scale,shear,self.synthetic_bold_offset_px(bold)?)
        }
        let _=(glyph,scale,bold,italic);Ok(4)
    }
    pub fn rasterize_glyph(&self,glyph:u32,scale:f64,phase:[u8;2],italic:bool)->Result<crate::paper_text::RasterGlyph,String>{
        self.rasterize_styled_glyph(glyph,scale,phase,false,italic)
    }
    /// Native Canvas style is distinct from PDF stroke/synthesis policy.
    pub fn rasterize_styled_glyph(&self,glyph:u32,scale:f64,phase:[u8;2],bold:bool,italic:bool)->Result<crate::paper_text::RasterGlyph,String>{
        let bold_offset=self.synthetic_bold_offset_px(bold)?;
        #[cfg(target_os="macos")]if let Some(n)=&self.native {
            let shear=if italic && !self.italic {crate::paper_coretext_glyph::synthetic_italic_shear()}else{0.};
            return n.raster(glyph,scale,phase,shear,bold_offset)
        }
        if bold_offset>0. {return Err("Canvas synthetic-bold glyph raster remains unsupported".into());}
        self.shaper.rasterize_glyph(glyph,scale,phase,self.synthetic_italic_shear(italic))
    }
    pub fn right_bearing_26_6(&self,glyph:u32)->Result<i32,String>{
        #[cfg(target_os="macos")]if let Some(n)=&self.native{return n.right_bearing_26_6(glyph);}
        self.shaper.right_bearing_26_6(glyph)
    }
    pub fn rasterize_rotated_glyph(&self,glyph:u32,scale:f64,phase:[u8;2],bold:bool,italic:bool,angle:f64)->Result<crate::paper_text::RasterGlyph,String>{
        if angle==0.{return self.rasterize_styled_glyph(glyph,scale,phase,bold,italic);}
        let bold_offset=self.synthetic_bold_offset_px(bold)?;
        #[cfg(target_os="macos")]if let Some(n)=&self.native{return n.raster_rotation(glyph,scale,phase,if italic&&!self.italic{crate::paper_coretext_glyph::synthetic_italic_shear()}else{0.},bold_offset,angle);}
        if bold_offset>0.{return Err("Canvas synthetic-bold glyph raster unsupported".into());}
        self.shaper.rasterize_rotated_glyph(glyph,scale,phase,self.synthetic_italic_shear(italic),angle)
    }
    pub fn shared_bytes(&self) -> &Arc<[u8]> {
        self.shaper.shared_bytes()
    }
    pub fn stats(&self) -> FontStats {
        let (glyphs, points) = self.geometry.borrow().stats();
        let cache = self.subsets.borrow();
        FontStats {
            glyphs,
            points,
            subset_entries: cache.entries.len(),
            subset_bytes: cache.bytes,
            builds: self.builds.get(),
        }
    }
    pub fn subset(&self, glyphs: &[u32]) -> Result<Arc<PdfFont>, String> {
        let ids = subset::ordered_ids(glyphs)?;
        if let Some(font) = self.subsets.borrow().entries.get(&ids) {
            return Ok(font.clone());
        }
        let built = subset::build(
            &mut self.geometry.borrow_mut(),
            self.shared_bytes(),
            self.key.face_index,
            ids.clone(),
        )?;
        let mut unicode = Vec::new();
        unicode
            .try_reserve_exact(built.ids.len())
            .map_err(|_| "Paper unicode map allocation failed")?;
        for (g, cid) in built.ids.iter().zip(0u16..) {
            unicode.push(if cid == 0 {
                Some(0)
            } else {
                self.reverse.get(*g as usize).copied().flatten()
            });
        }
        let font = Arc::new(PdfFont {
            id: Uuid::new_v4(),
            program: built.bytes,
            cids: built.cids,
            widths: built.widths,
            metadata: self.metadata.clone(),
            unicode,
            bounds_px: built.bounds_px,
        });
        // A valid very large subset can be emitted within document admission but is not cached.
        let mut cache = self.subsets.borrow_mut();
        if font.program.len() <= MAX_CACHED_SUBSET_BYTES {
            while cache.entries.len() >= MAX_CACHED_SUBSETS
                || cache.bytes + font.program.len() > MAX_CACHED_SUBSET_BYTES
            {
                if let Some(old) = cache.order.pop_front() {
                    if let Some(old) = cache.entries.remove(&old) {
                        cache.bytes -= old.program.len();
                    }
                } else {
                    break;
                }
            }
            cache
                .entries
                .try_reserve(1)
                .map_err(|_| "Paper subset cache allocation failed")?;
            cache
                .order
                .try_reserve(1)
                .map_err(|_| "Paper subset cache allocation failed")?;
            cache.order.push_back(ids.clone());
            cache.bytes += font.program.len();
            cache.entries.insert(ids, font.clone());
        }
        self.builds.set(self.builds.get().saturating_add(1));
        Ok(font)
    }
}
/// Caller-controlled resource identity must change when font bytes change; collisions refuse.
/// No process/IPC, file reread, or font-byte duplication occurs on a warm cache hit.
#[derive(Default)]
pub struct FontCache {
    fonts: HashMap<FontKey, Rc<CachedFont>>,
    order: VecDeque<FontKey>,
    bytes: usize,
    loads: u64,
}
impl FontCache {
    pub fn load(&mut self, key: FontKey, bytes: Arc<[u8]>) -> Result<Rc<CachedFont>, String> {
        if let Some(font) = self.fonts.get(&key) {
            if !Arc::ptr_eq(font.shared_bytes(), &bytes)
                && font.shared_bytes().as_ref() != bytes.as_ref()
            {
                return Err("Paper font resource identity collision".into());
            }
            return Ok(font.clone());
        }
        let font = Rc::new(CachedFont::new(key, bytes)?);
        self.order
            .try_reserve(1)
            .map_err(|_| "Paper font cache allocation failed")?;
        while self.fonts.len() >= MAX_CACHED_FONTS
            || self.bytes + font.shared_bytes().len() > MAX_CACHED_FONT_BYTES
        {
            if let Some(old) = self.order.pop_front() {
                if let Some(old) = self.fonts.remove(&old) {
                    self.bytes -= old.shared_bytes().len();
                }
            } else {
                return Err("Paper font cache budget exceeded".into());
            }
        }
        self.fonts
            .try_reserve(1)
            .map_err(|_| "Paper font cache allocation failed")?;
        self.order.push_back(key);
        self.bytes += font.shared_bytes().len();
        self.fonts.insert(key, font.clone());
        self.loads = self.loads.saturating_add(1);
        Ok(font)
    }
    #[cfg(target_os="macos")]
    pub(crate) fn load_coretext(&mut self,key:FontKey,native:Rc<crate::paper_coretext_glyph::NativeFont>)->Result<Rc<CachedFont>,String>{
        if let Some(font)=self.get(key){return Ok(font)}
        let bytes=native.bytes.clone();let font=Rc::new(CachedFont::new_backend(key,bytes,Some(native))?);
        self.order.try_reserve(1).map_err(|_|"Native font queue allocation failed")?;
        self.fonts.try_reserve(1).map_err(|_|"Native font map allocation failed")?;
        while self.fonts.len()>=MAX_CACHED_FONTS||self.bytes+font.shared_bytes().len()>MAX_CACHED_FONT_BYTES{
            let old=self.order.pop_front().ok_or("Native font cache budget exceeded")?;if let Some(old)=self.fonts.remove(&old){self.bytes-=old.shared_bytes().len();}
        }
        self.bytes+=font.shared_bytes().len();self.order.push_back(key);self.fonts.insert(key,font.clone());self.loads=self.loads.saturating_add(1);Ok(font)
    }
    pub fn get(&self, key: FontKey) -> Option<Rc<CachedFont>> {
        self.fonts.get(&key).cloned()
    }
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.order.clear();
        self.bytes = 0;
    }
    pub fn stats(&self) -> (usize, usize, u64) {
        (self.fonts.len(), self.bytes, self.loads)
    }
}

#[cfg(all(test,target_os="linux"))]
mod raster_admission_tests {
    use super::*;
    #[test]fn device_glyph_refusal_restores_shared_font_geometry(){
        let mut resolver=crate::paper_font_resolver::Resolver::new().unwrap();
        let font=resolver.resolve(crate::paper_font_resolver::Request{family:"Sans Serif".into(),bold:false,italic:false,required_chars:vec![]}).unwrap().font;
        let before=font.shape_run("A",None).unwrap();let glyph=before[0].id;let cap=font.cap_height_26_6();let metrics=font.underline_metrics_px().unwrap();
        let first=font.rasterize_glyph(glyph,0.1,[0,0],false).unwrap();assert!(first.coverage.iter().any(|&v|v>0));assert_eq!(first.coverage.len(),first.width*first.height);
        assert!(font.rasterize_glyph(glyph,40.,[0,0],false).is_err());assert!(font.rasterize_glyph(glyph,0.1,[64,0],false).is_err());assert!(font.rasterize_glyph(u32::MAX,0.1,[0,0],false).is_err());assert!(font.rasterize_glyph(glyph,f64::NAN,[0,0],false).is_err());
        assert_eq!(font.shape_run("A",None).unwrap(),before);assert_eq!(font.cap_height_26_6(),cap);assert_eq!(font.underline_metrics_px().unwrap(),metrics);
        let again=font.rasterize_glyph(glyph,0.1,[0,0],false).unwrap();assert_eq!(again.coverage,first.coverage);assert_eq!((again.left,again.top),(first.left,first.top));
    }
}
