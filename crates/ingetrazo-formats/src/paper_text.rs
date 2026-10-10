// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct paper font shaping foundation. No Qt, worker or runtime subprocess.
//! Not yet a full text painter: wrapping, bidi runs, fallback and embedding are separate.
use harfbuzz_sys as hb;
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
}
unsafe extern "C" fn advance(
    _: *mut hb::hb_font_t,
    _: *mut c_void,
    glyph: u32,
    data: *mut c_void,
) -> i32 {
    // HarfBuzz owns this immutable Box through the font-function destroy hook.
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
unsafe extern "C" fn release_advances(data: *mut c_void) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(data.cast::<Advances>()) });
    }
}
/// Immutable read-only font bytes remain alive until AFTER the HarfBuzz font drops.
/// Reuse this object for all strings of the same face; buffers are per shape call.
/// Deliberately not Send/Sync: no unsafe cross-thread ownership promise.
pub struct PaperFont {
    font: Handle<hb::hb_font_t>,
    bytes: Arc<[u8]>,
    pub units_per_em: u16,
    /// Pixel100 baseline metrics in QFixed units (1/64px).
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
impl PaperFont {
    pub fn new(bytes: Arc<[u8]>, face_index: u32) -> Result<Self, String> {
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
        let fixed = |units: i16| (i64::from(units) * 6400 / i64::from(upem)) as i32;
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
            widths.push((u64::from(width) * 6400 / u64::from(upem)) as i32);
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
            hb::hb_font_set_scale(parent.raw(), 6400, -6400);
            hb::hb_font_set_ppem(parent.raw(), 100, 100);
            let font = Handle::new(
                hb::hb_font_create_sub_font(parent.raw()),
                hb::hb_font_destroy,
            )?;
            let funcs = Handle::new(hb::hb_font_funcs_create(), hb::hb_font_funcs_destroy)?;
            if hb::hb_font_is_immutable(font.raw()) != 0
                || hb::hb_font_funcs_is_immutable(funcs.raw()) != 0
            {
                return Err("Paper font callback allocation failed".into());
            }
            let data = Box::into_raw(Box::new(Advances {
                widths,
                metrics: [ascent_26_6, descent_26_6, leading_26_6],
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
            hb::hb_font_funcs_make_immutable(funcs.raw());
            hb::hb_font_set_funcs(font.raw(), funcs.raw(), std::ptr::null_mut(), None);
            hb::hb_font_make_immutable(font.raw());
            Ok(Self {
                font,
                bytes,
                units_per_em: upem,
                ascent_26_6,
                descent_26_6,
                leading_26_6,
            })
        }
    }
    pub fn shared_bytes(&self) -> &Arc<[u8]> {
        &self.bytes
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
