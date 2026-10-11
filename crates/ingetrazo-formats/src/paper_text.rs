// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct paper font shaping foundation. No Qt, worker or runtime subprocess.
//! Not yet a full text painter: wrapping, bidi runs, fallback and embedding are separate.
use freetype_sys as ft;
use harfbuzz_sys as hb;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};
use std::{ffi::c_void, ptr::NonNull, sync::Arc};
pub const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_GLYPHS: usize = 256 * 1024;
struct Handle<T> {
    ptr: NonNull<T>,
    destroy: unsafe extern "C" fn(*mut T),
}
impl<T> Handle<T> {
    fn new(ptr: *mut T, destroy: unsafe extern "C" fn(*mut T)) -> Result<Self, String> {
        Ok(Self {
            ptr: NonNull::new(ptr).ok_or("Paper font allocation failed")?,
            destroy,
        })
    }
    fn raw(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}
impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.raw()) }
    }
}
struct Advances {
    widths: Vec<i32>,
    metrics: [i32; 3],
    geometry: Option<FtGeometry>,
}
unsafe extern "C" fn advance(
    _: *mut hb::hb_font_t,
    _: *mut c_void,
    glyph: u32,
    data: *mut c_void,
) -> i32 {
    // HarfBuzz owns the callback data through its font-function destroy hook.
    // Metrics are immutable; optional contours use a bounded thread-local cache.
    if data.is_null() {
        return 0;
    }
    let data = unsafe { &*data.cast::<Advances>() };
    data.widths.get(glyph as usize).copied().unwrap_or(0)
}
unsafe extern "C" fn horizontal_extents(
    _: *mut hb::hb_font_t,
    _: *mut c_void,
    out: *mut hb::hb_font_extents_t,
    data: *mut c_void,
) -> hb::hb_bool_t {
    if data.is_null() || out.is_null() {
        return 0;
    }
    let data = unsafe { &*data.cast::<Advances>() };
    unsafe {
        (*out).ascender = data.metrics[0];
        (*out).descender = data.metrics[1];
        (*out).line_gap = data.metrics[2];
    }
    1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hinting {
    None,
    Vertical,
    Full,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FtPolicy {
    pub hinting: Hinting,
    pub force_auto_hint: bool,
}
struct FtGeometry {
    face: Handle<ft::FT_FaceRec>, // drops before library and shared input bytes
    _library: Handle<ft::FT_LibraryRec>,
    points: RefCell<HashMap<u32, Arc<[[i32; 2]]>>>,
    total_points: Cell<usize>,
    failed: Cell<bool>,
    flags: i32,
}
unsafe extern "C" fn release_ft_face(p: *mut ft::FT_FaceRec) {
    unsafe {
        ft::FT_Done_Face(p);
    }
}
unsafe extern "C" fn release_ft_library(p: *mut ft::FT_LibraryRec) {
    unsafe {
        ft::FT_Done_FreeType(p);
    }
}
impl FtGeometry {
    unsafe fn new(bytes: &[u8], index: u32, policy: FtPolicy, pixels: u32) -> Result<Self, String> {
        let mut library = std::ptr::null_mut();
        if unsafe { ft::FT_Init_FreeType(&mut library) } != 0 {
            return Err("Paper font outline library failed".into());
        }
        let library = Handle::new(library, release_ft_library)?;
        let mut face = std::ptr::null_mut();
        if unsafe {
            ft::FT_New_Memory_Face(
                library.raw(),
                bytes.as_ptr(),
                bytes.len() as _,
                index as _,
                &mut face,
            )
        } != 0
        {
            return Err("Paper font outline face failed".into());
        }
        let face = Handle::new(face, release_ft_face)?;
        if unsafe { ft::FT_Set_Char_Size(face.raw(), (pixels * 64) as _, (pixels * 64) as _, 0, 0) } != 0 {
            return Err("Paper font outline scale failed".into());
        }
        let flags = match policy.hinting {
            Hinting::None => ft::FT_LOAD_NO_HINTING,
            Hinting::Vertical => ft::FT_LOAD_TARGET_LIGHT,
            Hinting::Full => ft::FT_LOAD_TARGET_NORMAL,
        } | if policy.force_auto_hint {
            ft::FT_LOAD_FORCE_AUTOHINT
        } else {
            0
        };
        Ok(Self {
            face,
            _library: library,
            points: RefCell::new(HashMap::new()),
            total_points: Cell::new(0),
            failed: Cell::new(false),
            flags,
        })
    }
    fn point(&self, glyph: u32, index: u32) -> Option<[i32; 2]> {
        if let Some(p) = self.points.borrow().get(&glyph) {
            let v = p.get(index as usize).copied();
            if v.is_none() {
                self.failed.set(true);
            }
            return v;
        }
        unsafe {
            if ft::FT_Load_Glyph(self.face.raw(), glyph, self.flags) != 0 {
                self.failed.set(true);
                return None;
            }
            let slot = (*self.face.raw()).glyph;
            if slot.is_null() || (*slot).format != ft::FT_GLYPH_FORMAT_OUTLINE {
                return None;
            }
            let outline = &(*slot).outline;
            if outline.n_points <= 0 {
                return None;
            }
            let count = outline.n_points as usize;
            if outline.points.is_null() || self.total_points.get() + count > 256 * 1024 {
                self.failed.set(true);
                return None;
            }
            let raw = std::slice::from_raw_parts(outline.points, count);
            let mut points = Vec::new();
            if points.try_reserve_exact(count).is_err() {
                self.failed.set(true);
                return None;
            }
            for v in raw {
                let (Ok(x), Ok(y)) = (i32::try_from(v.x), i32::try_from(v.y)) else {
                    self.failed.set(true);
                    return None;
                };
                points.push([x, y]);
            }
            let answer = points.get(index as usize).copied();
            if answer.is_none() {
                self.failed.set(true);
            }
            self.total_points.set(self.total_points.get() + count);
            self.points.borrow_mut().insert(glyph, points.into());
            answer
        }
    }
}
unsafe extern "C" fn contour_point(
    _: *mut hb::hb_font_t,
    _: *mut c_void,
    glyph: u32,
    index: u32,
    x: *mut i32,
    y: *mut i32,
    data: *mut c_void,
) -> hb::hb_bool_t {
    if data.is_null() || x.is_null() || y.is_null() {
        return 0;
    }
    let data = unsafe { &*data.cast::<Advances>() };
    let Some(point) = data.geometry.as_ref().and_then(|g| g.point(glyph, index)) else {
        return 0;
    };
    unsafe {
        *x = point[0];
        *y = point[1];
    }
    1
}
unsafe extern "C" fn release_advances(data: *mut c_void) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(data.cast::<Advances>()) });
    }
}
/// Immutable read-only font bytes remain alive until AFTER the HarfBuzz font drops.
/// Reuse this object for all strings of the same face; buffers are per shape call.
/// Deliberately not Send/Sync: no unsafe cross-thread ownership promise.
/// Device-resolution antialiased coverage, with FreeType baseline bearings.
/// The font owner remains immutable; each operation restores the face transform.
#[derive(Debug,Clone)]
pub struct RasterLcdGlyph {pub left:i32,pub top:i32,pub width:usize,pub height:usize,pub coverage:Vec<u8>}

#[derive(Debug,Clone)]
pub struct RasterGlyph {pub left:i32,pub top:i32,pub width:usize,pub height:usize,pub coverage:Vec<u8>}
struct RestoreFaceTransform(ft::FT_Face);
impl Drop for RestoreFaceTransform {fn drop(&mut self){unsafe{ft::FT_Set_Transform(self.0,std::ptr::null_mut(),std::ptr::null_mut());}}}
pub struct PaperFont {
    font: Handle<hb::hb_font_t>,
    bytes: Arc<[u8]>,
    data: NonNull<Advances>,
    pub units_per_em: u16,
    /// Resolved face-size metrics in QFixed units (1/64px).
    pub ascent_26_6: i32,
    pub descent_26_6: i32,
    pub leading_26_6: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub id: u32,
    pub cluster: u32,
    pub advance: [i32; 2],
    pub offset: [i32; 2],
}
/// Frozen source paper painter uses pixel100 normalized layout. Qt's effective
/// point size is75 at its reference96DPI, including QPdfWriter300DPI; device
/// painter scaling does not redefine the immutable HB shaping context.
#[cfg(target_os="macos")]
pub(crate) const QT_PAPER_SHAPING_POINT_SIZE: f32 = 100. * 72. / 96.;

impl PaperFont {
    pub fn new(bytes: Arc<[u8]>, face_index: u32) -> Result<Self, String> {
        Self::new_with_outline_policy(bytes, face_index, None)
    }
    pub fn new_with_outline_policy(
        bytes: Arc<[u8]>,
        face_index: u32,
        policy: Option<FtPolicy>,
    ) -> Result<Self, String> {
        Self::new_at_pixel_size(bytes, face_index, policy, 100)
    }
    /// Native viewport font at its resolved physical size. Existing paper
    /// callers retain the pixel100 face and transform policy.
    pub fn new_at_pixel_size(
        bytes: Arc<[u8]>,
        face_index: u32,
        policy: Option<FtPolicy>,
        pixels: u32,
    ) -> Result<Self, String> {
        Self::new_with_shaping_context(bytes, face_index, policy, pixels, None)
    }
    fn new_with_shaping_context(
        bytes: Arc<[u8]>, face_index: u32, policy: Option<FtPolicy>,
        pixels: u32, point_size: Option<f32>,
    ) -> Result<Self, String> {
        if pixels == 0 || pixels > 4096 {
            return Err("Native font pixel size outside admission".into());
        }
        if point_size.is_some_and(|p| !p.is_finite() || p <= 0. || p > 4096.) {
            return Err("Paper shaping point context refused".into());
        }
        let fixed_size = i64::from(pixels) * 64;
        if bytes.is_empty() || bytes.len() > MAX_FONT_BYTES {
            return Err("Paper font byte budget exceeded".into());
        }
        let face = ttf_parser::Face::parse(&bytes, face_index).map_err(|_| "Invalid paper font")?;
        if face.is_variable() {
            return Err("Paper variable font instance policy remains unavailable".into());
        }
        let upem = face.units_per_em();
        if upem == 0 || face.number_of_glyphs() == 0 {
            return Err("Invalid paper font metrics".into());
        }
        let fixed = |units: i16| (i64::from(units) * fixed_size / i64::from(upem)) as i32;
        let ascent_26_6 = fixed(face.ascender());
        let descent_26_6 = -fixed(face.descender());
        let leading_26_6 = fixed(face.line_gap()).max(0);
        let mut widths = Vec::new();
        widths
            .try_reserve_exact(face.number_of_glyphs() as usize)
            .map_err(|_| "Paper font metric allocation failed")?;
        for id in 0..face.number_of_glyphs() {
            let width = face
                .glyph_hor_advance(ttf_parser::GlyphId(id))
                .ok_or("Missing paper horizontal advance")?;
            // QFontEngine's pixel100 QFixed horizontal advances truncate at1/64.
            widths.push((u64::from(width) * fixed_size as u64 / u64::from(upem)) as i32);
        }
        // All temporary HB objects are RAII; the font references its parent's
        // face/blob. READONLY never duplicates or modifies the shared font bytes.
        unsafe {
            let blob = Handle::new(
                hb::hb_blob_create(
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    hb::HB_MEMORY_MODE_READONLY,
                    std::ptr::null_mut(),
                    None,
                ),
                hb::hb_blob_destroy,
            )?;
            let hbface = Handle::new(
                hb::hb_face_create(blob.raw(), face_index),
                hb::hb_face_destroy,
            )?;
            if hb::hb_face_get_glyph_count(hbface.raw()) != u32::from(face.number_of_glyphs()) {
                return Err("Paper font face allocation failed".into());
            }
            let parent = Handle::new(hb::hb_font_create(hbface.raw()), hb::hb_font_destroy)?;
            hb::hb_ot_font_set_funcs(parent.raw());
            hb::hb_font_set_scale(parent.raw(), fixed_size as i32, -(fixed_size as i32));
            hb::hb_font_set_ppem(parent.raw(), pixels, pixels);
            if let Some(ptem) = point_size { hb::hb_font_set_ptem(parent.raw(), ptem); }
            let font = Handle::new(
                hb::hb_font_create_sub_font(parent.raw()),
                hb::hb_font_destroy,
            )?;
            // Native CoreText paper fonts use the exact Qt tracking source policy.
            // General FreeType shaping keeps its existing HarfBuzz behavior.
            if point_size.is_some() && hb::ingetrazo_hb_font_set_qt_tracking(font.raw()) == 0 {
                return Err("Paper font tracking policy admission failed".into());
            }
            let funcs = Handle::new(hb::hb_font_funcs_create(), hb::hb_font_funcs_destroy)?;
            if hb::hb_font_is_immutable(font.raw()) != 0
                || hb::hb_font_funcs_is_immutable(funcs.raw()) != 0
            {
                return Err("Paper font callback allocation failed".into());
            }
            let data = Box::into_raw(Box::new(Advances {
                widths,
                metrics: [ascent_26_6, descent_26_6, leading_26_6],
                geometry: policy
                    .map(|p| FtGeometry::new(&bytes, face_index, p, pixels))
                    .transpose()?,
            }))
            .cast();
            hb::hb_font_funcs_set_glyph_h_advance_func(
                funcs.raw(),
                Some(advance),
                data,
                Some(release_advances),
            );
            hb::hb_font_funcs_set_font_h_extents_func(
                funcs.raw(),
                Some(horizontal_extents),
                data,
                None,
            );
            if policy.is_some() {
                hb::hb_font_funcs_set_glyph_contour_point_func(
                    funcs.raw(),
                    Some(contour_point),
                    data,
                    None,
                );
            }
            hb::hb_font_funcs_make_immutable(funcs.raw());
            hb::hb_font_set_funcs(font.raw(), funcs.raw(), std::ptr::null_mut(), None);
            hb::hb_font_make_immutable(font.raw());
            Ok(Self {
                font,
                bytes,
                data: NonNull::new(data.cast::<Advances>()).unwrap(),
                units_per_em: upem,
                ascent_26_6,
                descent_26_6,
                leading_26_6,
            })
        }
    }
    #[cfg(target_os="macos")]
    pub(crate) fn new_coretext(native:&crate::paper_coretext_glyph::NativeFont)->Result<Self,String>{
        Self::new_coretext_sized(native,100,Some(QT_PAPER_SHAPING_POINT_SIZE))
    }
    #[cfg(target_os="macos")]
    pub(crate) fn new_coretext_sized(native:&crate::paper_coretext_glyph::NativeFont,pixels:u32,point_size:Option<f32>)->Result<Self,String>{
        let mut font=Self::new_with_shaping_context(native.bytes.clone(),0,None,pixels,point_size)?;
        let metrics=native.metrics();let data=unsafe{font.data.as_mut()};
        for (id,width)in data.widths.iter_mut().enumerate(){*width=(native.advance(id as u32)?*64.)as i32;}
        data.metrics=[metrics[0],metrics[1],metrics[2]];font.ascent_26_6=metrics[0];font.descent_26_6=metrics[1];font.leading_26_6=metrics[2];Ok(font)
    }
    /// Immutable HarfBuzz point context (zero means unspecified).
    pub fn shaping_point_size(&self) -> f32 {
        unsafe { hb::hb_font_get_ptem(self.font.raw()) }
    }
    /// Render an existing pixel100 glyph at the actual canvas device transform.
    /// Phases are 1/64 device pixel; this does not modify shaping/outline caches.
    pub fn rasterize_glyph(&self,glyph:u32,scale:f64,phase:[u8;2],italic_shear:f64)->Result<RasterGlyph,String>{
        if !scale.is_finite()||scale<=0.||scale>40.||phase.iter().any(|&p|p>=64)||!italic_shear.is_finite()||italic_shear.abs()>1.{return Err("Canvas glyph transform outside domain".into());}
        unsafe{
            let geometry=self.data.as_ref().geometry.as_ref().ok_or("Canvas glyph requires native font geometry")?;
            let face=geometry.face.raw();if glyph>=(*face).num_glyphs as u32{return Err("Canvas glyph index outside font".into());}
            let mut matrix=ft::FT_Matrix{xx:(scale*65536.)as _,xy:(scale*italic_shear*65536.)as _,yx:0,yy:(scale*65536.)as _};
            let mut delta=ft::FT_Vector{x:phase[0]as _,y:-(phase[1]as ft::FT_Pos)};
            ft::FT_Set_Transform(face,&mut matrix,&mut delta);let _restore=RestoreFaceTransform(face);
            // Qt 6.12 loadGlyphFor disables hinting for scaled nonrotation glyphs.
            // Keep the pixel100 layout/face size; only this device raster load is unhinted.
            let flags=geometry.flags|ft::FT_LOAD_NO_BITMAP|if scale!=1. {ft::FT_LOAD_NO_HINTING}else{0};
            if ft::FT_Load_Glyph(face,glyph,flags)!=0||(*face).glyph.is_null(){return Err("Canvas glyph load failed".into());}
            let slot=(*face).glyph;
            if (*slot).format!=ft::FT_GLYPH_FORMAT_OUTLINE{return Err("Canvas glyph outline format unavailable".into());}
            let mut bbox:ft::FT_BBox=std::mem::zeroed();ft::FT_Outline_Get_CBox(&(*slot).outline,&mut bbox);
            let w=bbox.xMax.checked_sub(bbox.xMin).and_then(|v|v.checked_add(127)).map(|v|v/64+2).filter(|&v|(0..=4096).contains(&v)).ok_or("Canvas glyph dimension exceeded")?;
            let h=bbox.yMax.checked_sub(bbox.yMin).and_then(|v|v.checked_add(127)).map(|v|v/64+2).filter(|&v|(0..=4096).contains(&v)).ok_or("Canvas glyph dimension exceeded")?;
            if w.checked_mul(h).is_none_or(|n|n>4*1024*1024){return Err("Canvas glyph coverage budget exceeded".into());}
            if ft::FT_Render_Glyph(slot,ft::FT_RENDER_MODE_NORMAL)!=0{return Err("Canvas glyph raster failed".into());}
            let bitmap=&(*slot).bitmap;let width=bitmap.width as usize;let height=bitmap.rows as usize;
            let bytes=width.checked_mul(height).filter(|&n|n<=4*1024*1024).ok_or("Canvas glyph coverage budget exceeded")?;
            if width>4096||height>4096{return Err("Canvas glyph dimension exceeded".into());}
            let mut coverage=Vec::new();coverage.try_reserve_exact(bytes).map_err(|_|"Canvas glyph coverage allocation failed")?;coverage.resize(bytes,0);
            if bytes>0{
                if bitmap.buffer.is_null()||bitmap.pixel_mode as u32!=ft::FT_PIXEL_MODE_GRAY||bitmap.num_grays!=256||(bitmap.pitch.unsigned_abs()as usize)<width{return Err("Invalid canvas glyph bitmap".into());}
                for y in 0..height{let row=bitmap.buffer.offset((y as isize)*(bitmap.pitch as isize));coverage[y*width..(y+1)*width].copy_from_slice(std::slice::from_raw_parts(row,width));}
            }
            Ok(RasterGlyph{left:(*slot).bitmap_left,top:(*slot).bitmap_top,width,height,coverage})
        }
    }
    /// Source OpenGL RGB-horizontal A32 glyph coverage. This opt-in path
    /// preserves the existing grayscale renderer and admits the full RGB bytes.
    pub fn rasterize_lcd_glyph(&self,glyph:u32,scale:f64,phase:[u8;2],italic_shear:f64)->Result<RasterLcdGlyph,String>{
        if !scale.is_finite()||scale<=0.||scale>40.||phase.iter().any(|&p|p>=64)||!italic_shear.is_finite()||italic_shear.abs()>1.{return Err("Canvas glyph transform outside domain".into());}
        unsafe{
            let geometry=self.data.as_ref().geometry.as_ref().ok_or("Canvas glyph requires native font geometry")?;
            let face=geometry.face.raw();if glyph>=(*face).num_glyphs as u32{return Err("Canvas glyph index outside font".into());}
            let mut matrix=ft::FT_Matrix{xx:(scale*65536.)as _,xy:(scale*italic_shear*65536.)as _,yx:0,yy:(scale*65536.)as _};
            let mut delta=ft::FT_Vector{x:phase[0]as _,y:-(phase[1]as ft::FT_Pos)};
            ft::FT_Set_Transform(face,&mut matrix,&mut delta);let _restore=RestoreFaceTransform(face);
            // Qt 6.12 loadGlyphFor disables hinting for scaled nonrotation glyphs.
            // Keep the pixel100 layout/face size; only this device raster load is unhinted.
            let flags=geometry.flags|ft::FT_LOAD_NO_BITMAP|if scale!=1. {ft::FT_LOAD_NO_HINTING}else{0};
            if ft::FT_Load_Glyph(face,glyph,flags)!=0||(*face).glyph.is_null(){return Err("Canvas glyph load failed".into());}
            let slot=(*face).glyph;
            if (*slot).format!=ft::FT_GLYPH_FORMAT_OUTLINE{return Err("Canvas glyph outline format unavailable".into());}
            let mut bbox:ft::FT_BBox=std::mem::zeroed();ft::FT_Outline_Get_CBox(&(*slot).outline,&mut bbox);
            let w=bbox.xMax.checked_sub(bbox.xMin).and_then(|v|v.checked_add(127)).map(|v|v/64+2).filter(|&v|(0..=4096).contains(&v)).ok_or("Canvas glyph dimension exceeded")?;
            let h=bbox.yMax.checked_sub(bbox.yMin).and_then(|v|v.checked_add(127)).map(|v|v/64+2).filter(|&v|(0..=4096).contains(&v)).ok_or("Canvas glyph dimension exceeded")?;
            // LCD is3 bytes/pixel; admit the filtered bitmap and conservative
            // two-pixel filter padding before FreeType allocates its output.
            if w.checked_add(4).and_then(|w|h.checked_add(2).and_then(|h|w.checked_mul(h))).and_then(|n|n.checked_mul(3)).is_none_or(|n|n>4*1024*1024){return Err("LCD glyph pre-raster RGB budget exceeded".into());}
            // Qt6.12 QFontEngineFT A32 uses the default LCD filter, then resets
            // the library filter. Layout/load flags remain shared with the gray path.
            if ft::FT_Library_SetLcdFilter((*slot).library,ft::FT_LCD_FILTER_DEFAULT)!=0{return Err("LCD glyph filter unavailable".into());}
            let rendered=ft::FT_Render_Glyph(slot,ft::FT_RENDER_MODE_LCD);
            ft::FT_Library_SetLcdFilter((*slot).library,ft::FT_LCD_FILTER_NONE);
            if rendered!=0{return Err("LCD glyph raster failed".into());}
            let bitmap=&(*slot).bitmap;if bitmap.width%3!=0{return Err("Invalid LCD glyph width".into());}let width=bitmap.width as usize/3;let height=bitmap.rows as usize;
            let bytes=width.checked_mul(height).and_then(|n|n.checked_mul(3)).filter(|&n|n<=4*1024*1024).ok_or("Canvas glyph coverage budget exceeded")?;
            if width>4096||height>4096{return Err("Canvas glyph dimension exceeded".into());}
            let mut coverage=Vec::new();coverage.try_reserve_exact(bytes).map_err(|_|"Canvas glyph coverage allocation failed")?;coverage.resize(bytes,0);
            if bytes>0{
                if bitmap.buffer.is_null()||bitmap.pixel_mode as u32!=ft::FT_PIXEL_MODE_LCD||bitmap.num_grays!=256||(bitmap.pitch.unsigned_abs()as usize)<width*3{return Err("Invalid canvas glyph bitmap".into());}
                for y in 0..height{let row=bitmap.buffer.offset((y as isize)*(bitmap.pitch as isize));coverage[y*width*3..(y+1)*width*3].copy_from_slice(std::slice::from_raw_parts(row,width*3));}
            }
            Ok(RasterLcdGlyph{left:(*slot).bitmap_left,top:(*slot).bitmap_top,width,height,coverage})
        }
    }
    /// Cached outline points/glyphs, useful for warm font-publication accounting.
    pub fn outline_cache_stats(&self) -> (usize, usize) {
        unsafe {
            self.data
                .as_ref()
                .geometry
                .as_ref()
                .map_or((0, 0), |g| (g.points.borrow().len(), g.total_points.get()))
        }
    }
    /// Qt FT source uses OS/2 sCapHeight when present; otherwise the hinted H glyph box.
    pub fn cap_height_26_6(&self, cap_units: Option<i16>) -> Result<i32, String> {
        unsafe {
            let geometry = self
                .data
                .as_ref()
                .geometry
                .as_ref()
                .ok_or("Paper cap height requires native outline policy")?;
            let face = geometry.face.raw();
            if (*face).size.is_null() {
                return Err("Invalid paper cap-height face".into());
            }
            if let Some(units) = cap_units {
                return i32::try_from(ft::FT_MulFix(units as _, (*(*face).size).metrics.y_scale))
                    .map_err(|_| "Paper cap-height outside fixed domain".into());
            }
            let glyph = ft::FT_Get_Char_Index(face, 'H' as _);
            if ft::FT_Load_Glyph(face, glyph, geometry.flags) != 0 || (*face).glyph.is_null() {
                return Err("Paper capital glyph load failed".into());
            }
            i32::try_from((*(*face).glyph).metrics.height)
                .map_err(|_| "Paper capital box outside fixed domain".into())
        }
    }
    /// Source FT line thickness / underline offset, both in pixel100 QFixed units.
    pub fn decoration_metrics_26_6(&self) -> Result<[i32; 2], String> {
        unsafe {
            let geometry = self
                .data
                .as_ref()
                .geometry
                .as_ref()
                .ok_or("Paper decoration metrics require a native outline policy")?;
            let face = geometry.face.raw();
            let size = (*face).size;
            if size.is_null() {
                return Err("Invalid paper decoration face metrics".into());
            }
            let scale = (*size).metrics.y_scale;
            let thickness = i32::try_from(ft::FT_MulFix((*face).underline_thickness as _, scale))
                .map_err(|_| "Paper decoration thickness outside fixed domain")?;
            let center = i32::try_from(-ft::FT_MulFix((*face).underline_position as _, scale))
                .map_err(|_| "Paper decoration offset outside fixed domain")?;
            let offset = center
                .checked_sub(thickness / 2)
                .ok_or("Paper decoration metric overflow")?;
            Ok([thickness.max(64), offset])
        }
    }
    pub fn shared_bytes(&self) -> &Arc<[u8]> {
        &self.bytes
    }
    /// Native nominal character coverage without a shaping allocation or font-byte copy.
    pub fn covers_character(&self, ch: char) -> bool {
        let mut glyph = 0;
        unsafe { hb::hb_font_get_nominal_glyph(self.font.ptr.as_ptr(), ch as u32, &mut glyph) != 0 }
    }
    /// A single script/direction run. Paragraph bidi and line wrapping must not
    /// be inferred from this API; they remain separate source-layout contracts.
    pub fn shape_run(&self, text: &str, right_to_left: Option<bool>) -> Result<Vec<Glyph>, String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err("Paper text byte budget exceeded".into());
        }
        if text.is_empty() {
            return Ok(vec![]);
        }
        unsafe {
            if let Some(g) = self.data.as_ref().geometry.as_ref() {
                g.failed.set(false);
            }
            let buffer = Handle::new(hb::hb_buffer_create(), hb::hb_buffer_destroy)?;
            hb::hb_buffer_add_utf8(
                buffer.raw(),
                text.as_ptr().cast(),
                text.len() as i32,
                0,
                text.len() as i32,
            );
            if let Some(rtl) = right_to_left {
                hb::hb_buffer_set_direction(
                    buffer.raw(),
                    if rtl {
                        hb::HB_DIRECTION_RTL
                    } else {
                        hb::HB_DIRECTION_LTR
                    },
                );
            }
            hb::hb_buffer_guess_segment_properties(buffer.raw());
            if hb::hb_buffer_allocation_successful(buffer.raw()) == 0 {
                return Err("Paper text buffer allocation failed".into());
            }
            hb::hb_shape(self.font.raw(), buffer.raw(), std::ptr::null(), 0);
            if hb::hb_buffer_allocation_successful(buffer.raw()) == 0 {
                return Err("Paper text shaping allocation failed".into());
            }
            if self
                .data
                .as_ref()
                .geometry
                .as_ref()
                .is_some_and(|g| g.failed.get())
            {
                return Err(
                    "Paper font contour callback refused invalid outline publication".into(),
                );
            }
            let mut count = 0;
            let infos = hb::hb_buffer_get_glyph_infos(buffer.raw(), &mut count);
            let mut position_count = 0;
            let positions = hb::hb_buffer_get_glyph_positions(buffer.raw(), &mut position_count);
            if count != position_count
                || count as usize > MAX_GLYPHS
                || (count > 0 && (infos.is_null() || positions.is_null()))
            {
                return Err("Invalid paper glyph publication".into());
            }
            if count == 0 {
                return Ok(vec![]);
            }
            let infos = std::slice::from_raw_parts(infos, count as usize);
            let positions = std::slice::from_raw_parts(positions, count as usize);
            let mut out = Vec::new();
            out.try_reserve_exact(count as usize)
                .map_err(|_| "Paper glyph allocation failed")?;
            for (info, p) in infos.iter().zip(positions) {
                out.push(Glyph {
                    id: info.codepoint,
                    cluster: info.cluster,
                    advance: [p.x_advance, p.y_advance],
                    offset: [p.x_offset, p.y_offset],
                });
            }
            Ok(out)
        }
    }
}
