// SPDX-License-Identifier: GPL-3.0-or-later
//! Thread-confined native glyph owner. No full collection copy and no hvgl parser.
//! CoreText owns the selected instance; bounded SFNT tables serve HarfBuzz/PDF metadata.
use std::{cell::Cell, ffi::c_void, rc::Rc, sync::Arc};
type Ref = *const c_void;
pub(crate) struct Owned(Ref);
impl Owned {
    pub(crate) fn new(p: Ref) -> Result<Self, String> {
        if p.is_null() {
            Err("CoreText resource unavailable".into())
        } else {
            Ok(Self(p))
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct Point {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    origin: Point,
    size: Size,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    tx: f64,
    ty: f64,
}
#[repr(C)]
struct Element {
    kind: i32,
    points: *const Point,
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(p: Ref);
    fn CFStringGetLength(p: Ref) -> isize;
    fn CFStringGetCString(p: Ref, buffer: *mut i8, bytes: isize, encoding: u32) -> u8;
    fn CFDataGetLength(p: Ref) -> isize;
    fn CFDataGetBytePtr(p: Ref) -> *const u8;
    fn CFDictionaryGetCount(p: Ref) -> isize;
    fn CFDictionaryGetKeysAndValues(p: Ref, keys: *mut Ref, values: *mut Ref);
    fn CFNumberGetValue(p: Ref, kind: i32, out: *mut c_void) -> u8;
}
#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCopyTable(f: Ref, tag: u32, options: u32) -> Ref;
    fn CTFontCopyFamilyName(f: Ref) -> Ref;
    fn CTFontCopyPostScriptName(f: Ref) -> Ref;
    fn CTFontCreateCopyWithAttributes(f: Ref, size: f64, matrix: *const Matrix, attributes: Ref) -> Ref;
    fn CTFontCopyVariation(f: Ref) -> Ref;
    fn CTFontGetMatrix(f: Ref) -> Matrix;
    fn CTFontGetGlyphCount(f: Ref) -> isize;
    fn CTFontGetAdvancesForGlyphs(f: Ref, o: u32, g: *const u16, a: *mut Size, n: isize) -> f64;
    fn CTFontGetBoundingRectsForGlyphs(
        f: Ref,
        o: u32,
        g: *const u16,
        r: *mut Rect,
        n: isize,
    ) -> Rect;
    fn CTFontCreatePathForGlyph(f: Ref, g: u16, m: *const Matrix) -> Ref;
    fn CTFontGetAscent(f: Ref) -> f64;
    fn CTFontGetDescent(f: Ref) -> f64;
    fn CTFontGetLeading(f: Ref) -> f64;
    fn CTFontGetCapHeight(f: Ref) -> f64;
    fn CTFontGetUnderlineThickness(f: Ref) -> f64;
    fn CTFontGetUnderlinePosition(f: Ref) -> f64;
    fn CTFontDrawGlyphs(f: Ref, g: *const u16, p: *const Point, n: usize, c: Ref);
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPathApply(p: Ref, info: *mut c_void, callback: extern "C" fn(*mut c_void, *const Element));
    fn CGPathRelease(p: Ref);
    fn CGColorSpaceCreateWithName(name: Ref) -> Ref;
    static kCGColorSpaceSRGB: Ref;
    static kCGColorBlack: Ref;
    fn CGColorGetConstantColor(name: Ref) -> Ref;
    fn CGColorSpaceRelease(p: Ref);
    fn CGBitmapContextCreate(
        data: *mut c_void,
        w: usize,
        h: usize,
        bits: usize,
        row: usize,
        space: Ref,
        info: u32,
    ) -> Ref;
    fn CGContextRelease(p: Ref);
    fn CGContextSetShouldAntialias(p: Ref, v: bool);
    fn CGContextSetShouldSmoothFonts(p: Ref, v: bool);
    fn CGContextSetFillColorWithColor(p: Ref, color: Ref);
    fn CGContextSetTextMatrix(p: Ref, m: Matrix);
    fn CGContextSetTextPosition(p: Ref, x: f64, y: f64);
}
#[derive(Clone, Debug)]
pub(crate) enum PathOp {
    Move([f64; 2]),
    Line([f64; 2]),
    Quad([[f64; 2]; 2]),
    Cubic([[f64; 2]; 3]),
    Close,
}
struct Path {
    ops: Vec<PathOp>,
    failed: bool,
}
extern "C" fn path_element(info: *mut c_void, e: *const Element) {
    let out = unsafe { &mut *info.cast::<Path>() };
    if out.failed {
        return;
    }
    let e = unsafe { &*e };
    let n = match e.kind {
        0 | 1 => 1,
        2 => 2,
        3 => 3,
        4 => 0,
        _ => {
            out.failed = true;
            return;
        }
    };
    if n > 0 && e.points.is_null() {
        out.failed = true;
        return;
    }
    if out.ops.len() >= 65536 || out.ops.try_reserve(1).is_err() {
        out.failed = true;
        return;
    }
    let mut p = [[0.; 2]; 3];
    for (i, v) in p.iter_mut().enumerate().take(n) {
        let point = unsafe { *e.points.add(i) };
        *v = [point.x, point.y];
        if v.iter().any(|x| !x.is_finite() || x.abs() > 1e6) {
            out.failed = true;
            return;
        }
    }
    out.ops.push(match e.kind {
        0 => PathOp::Move(p[0]),
        1 => PathOp::Line(p[0]),
        2 => PathOp::Quad([p[0], p[1]]),
        3 => PathOp::Cubic(p),
        _ => PathOp::Close,
    });
}
/// Qt CoreText defines this in float precision, separately from FreeType's0.3.
pub(crate) fn synthetic_italic_shear() -> f64 {
    (14f32 * 0f32.acos() / 90f32).tan() as f64
}
// Qt RGB32 uses 32Little + ignored FIRST alpha: bytes BGRX.
// Match opaque source blending and invert RGB BEFORE qGray extraction.
const NATIVE_BITMAP_BGRX: u32 = 0x2000 | 6;
fn native_bgrx_coverage(p: &[u8]) -> u8 {
    ((11 * (255 - u32::from(p[2])) + 16 * (255 - u32::from(p[1]))
        + 5 * (255 - u32::from(p[0]))) / 32) as u8
}

fn subset_metric(value: f64, em: f64) -> f64 {
    (fixed_metric(fixed_metric(value) * em / 100.) * 2048. / em).round()
}

fn fixed_metric(value: f64) -> f64 {
    ((value * 64.) as i32) as f64 / 64.
}
fn raster_width(raw_width: f64, raw_height: f64, shear: f64, scale: f64) -> f64 {
    // Qt boundingBox first incorporates synthesis into the native CGRect,
    // then qcoretextfontengine_scaleMetrics quantizes the device scaling.
    fixed_metric(fixed_metric(raw_width + raw_height * shear) * scale) + 3.
}
/// Retained native font references are thread confined by Rc, just like CachedFont.
pub(crate) struct NativeFont {
    raster_phase_counts: Cell<[u8;4]>,
    owner: Owned,
    pub bytes: Arc<[u8]>,
    pub count: u16,
    pub traits: u32,
    pub instance_identity: Vec<u8>,
}
impl NativeFont {
    pub(crate) fn names(&self)->Result<[String;2],String>{
        fn text(raw:Ref)->Result<String,String>{
            let name=Owned::new(raw)?;
            let length=unsafe{CFStringGetLength(name.0)};
            if !(1..=256).contains(&length){return Err("Native font name outside admission".into());}
            let mut bytes=[0i8;1025];
            if unsafe{CFStringGetCString(name.0,bytes.as_mut_ptr(),bytes.len()as isize,0x08000100)}==0 {
                return Err("Native font name conversion failed".into());
            }
            unsafe{std::ffi::CStr::from_ptr(bytes.as_ptr())}.to_str()
                .map(str::to_owned).map_err(|_|"Native font name UTF8 unavailable".into())
        }
        Ok([text(unsafe{CTFontCopyFamilyName(self.owner.0)})?,
            text(unsafe{CTFontCopyPostScriptName(self.owner.0)})?])
    }
    pub(crate) fn at_pixel_size(&self, pixels: u32) -> Result<Rc<Self>, String> {
        if pixels == 0 || pixels > 4096 { return Err("Native font pixel size outside admission".into()); }
        let owner = Owned::new(unsafe { CTFontCreateCopyWithAttributes(self.owner.0,
            f64::from(pixels), std::ptr::null(), std::ptr::null()) })?;
        // Native physical instance remains owned; immutable shaping tables are
        // shared rather than reading/copying the original TTC at every size.
        Ok(Rc::new(Self { raster_phase_counts: Cell::new([0;4]), owner,
            bytes: self.bytes.clone(), count: self.count, traits: self.traits,
            instance_identity: Vec::new() }))
    }
    pub(crate) fn from_raw(p: Ref, traits: u32) -> Result<Rc<Self>, String> {
        Self::from_owned(Owned::new(p)?, traits)
    }
    fn from_owned(owner: Owned, traits: u32) -> Result<Rc<Self>, String> {
        if traits & (1 << 13) != 0 {
            return Err("Native color-font paper glyph policy remains unavailable".into());
        }
        let native_metrics = unsafe {
            [
                CTFontGetAscent(owner.0),
                CTFontGetDescent(owner.0),
                CTFontGetLeading(owner.0),
                CTFontGetCapHeight(owner.0),
                CTFontGetUnderlineThickness(owner.0),
                CTFontGetUnderlinePosition(owner.0),
            ]
        };
        if native_metrics
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        {
            return Err("Invalid native font metrics".into());
        }
        let mut instance_identity = Vec::new();
        let matrix = unsafe { CTFontGetMatrix(owner.0) };
        let components = [matrix.a, matrix.b, matrix.c, matrix.d, matrix.tx, matrix.ty];
        if components
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
        {
            return Err("Invalid native font matrix".into());
        }
        instance_identity
            .try_reserve_exact(52)
            .map_err(|_| "Native matrix identity allocation failed")?;
        instance_identity.extend_from_slice(&traits.to_be_bytes());
        for v in components {
            instance_identity.extend_from_slice(&v.to_bits().to_be_bytes());
        }

        let raw = unsafe { CTFontCopyVariation(owner.0) };
        if !raw.is_null() {
            let variation = Owned::new(raw)?;
            let count = unsafe { CFDictionaryGetCount(variation.0) };
            if !(0..=64).contains(&count) {
                return Err("Native variation identity budget exceeded".into());
            }
            let mut keys = [std::ptr::null(); 64];
            let mut values = [std::ptr::null(); 64];
            unsafe {
                CFDictionaryGetKeysAndValues(variation.0, keys.as_mut_ptr(), values.as_mut_ptr())
            };
            let mut pairs = Vec::new();
            pairs
                .try_reserve_exact(count as usize)
                .map_err(|_| "Native axis identity allocation failed")?;
            for i in 0..count as usize {
                let mut tag = 0i64;
                let mut value = 0f64;
                let ok = unsafe {
                    CFNumberGetValue(keys[i], 4, (&mut tag as *mut i64).cast()) != 0
                        && CFNumberGetValue(values[i], 13, (&mut value as *mut f64).cast()) != 0
                };
                if !ok || tag < 0 || tag > u32::MAX as i64 || !value.is_finite() {
                    return Err("Invalid native variation identity".into());
                }
                pairs.push((tag as u32, value.to_bits()))
            }
            pairs.sort_unstable();
            instance_identity
                .try_reserve_exact(pairs.len() * 12)
                .map_err(|_| "Native instance identity allocation failed")?;
            for (tag, value) in pairs {
                instance_identity.extend_from_slice(&tag.to_be_bytes());
                instance_identity.extend_from_slice(&value.to_be_bytes());
            }
        }
        // Native tracking policy and HB point-size are part of instance identity;
        // changing either cannot reuse an earlier paper layout/shape cache.
        instance_identity.try_reserve_exact(24)
            .map_err(|_| "Native shaping identity allocation failed")?;
        instance_identity.extend_from_slice(b"qrgb32sr");
        instance_identity.extend_from_slice(b"qtrk14.5");
        instance_identity.extend_from_slice(b"qptm");
        instance_identity.extend_from_slice(&crate::paper_text::QT_PAPER_SHAPING_POINT_SIZE.to_bits().to_be_bytes());
        let count = unsafe { CTFontGetGlyphCount(owner.0) };
        let count = u16::try_from(count).map_err(|_| "Native glyph count budget exceeded")?;
        if count == 0 {
            return Err("Native font has no glyphs".into());
        }
        // No outline/collection/variation program copy. Native instance controls paths and widths.
        let tags = [
            *b"head", *b"hhea", *b"hmtx", *b"maxp", *b"cmap", *b"name", *b"OS/2", *b"post",
            *b"GDEF", *b"GPOS", *b"GSUB", *b"kern", *b"morx", *b"mort", *b"kerx", *b"trak",
            *b"BASE",
        ];
        let mut tables = Vec::new();
        let mut total = 12 + tags.len() * 16;
        for tag in tags {
            let raw = unsafe { CTFontCopyTable(owner.0, u32::from_be_bytes(tag), 0) };
            if raw.is_null() {
                continue;
            }
            let data = Owned::new(raw)?;
            let n = unsafe { CFDataGetLength(data.0) };
            let n = usize::try_from(n).map_err(|_| "Invalid native font table")?;
            total = total
                .checked_add((n + 3) & !3)
                .ok_or("Native table byte overflow")?;
            if total > crate::paper_text::MAX_FONT_BYTES {
                return Err("Native shaping table budget exceeded".into());
            }
            let ptr = unsafe { CFDataGetBytePtr(data.0) };
            if n > 0 && ptr.is_null() {
                return Err("Invalid native font table pointer".into());
            }
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(n)
                .map_err(|_| "Native table allocation failed")?;
            if n > 0 {
                bytes.extend_from_slice(unsafe { std::slice::from_raw_parts(ptr, n) })
            }
            tables.push((tag, bytes));
        }
        tables.sort_by_key(|t| t.0);
        let n = tables.len();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| "Native SFNT allocation failed")?;
        bytes.resize(12 + n * 16, 0);
        bytes[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        bytes[4..6].copy_from_slice(&(n as u16).to_be_bytes());
        let power = 1usize << (usize::BITS - 1 - n.leading_zeros());
        bytes[6..8].copy_from_slice(&((power * 16) as u16).to_be_bytes());
        bytes[8..10].copy_from_slice(&(power.trailing_zeros() as u16).to_be_bytes());
        bytes[10..12].copy_from_slice(&((n * 16 - power * 16) as u16).to_be_bytes());
        for (i, (tag, data)) in tables.into_iter().enumerate() {
            let offset = bytes.len();
            let row = 12 + i * 16;
            bytes[row..row + 4].copy_from_slice(&tag);
            let checksum = data.chunks(4).fold(0u32, |sum, c| {
                let mut a = [0; 4];
                a[..c.len()].copy_from_slice(c);
                sum.wrapping_add(u32::from_be_bytes(a))
            });
            bytes[row + 4..row + 8].copy_from_slice(&checksum.to_be_bytes());
            bytes[row + 8..row + 12].copy_from_slice(&(offset as u32).to_be_bytes());
            bytes[row + 12..row + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&data);
            while bytes.len() % 4 != 0 {
                bytes.push(0)
            }
        }
        ttf_parser::Face::parse(&bytes, 0).map_err(|_| "Invalid extracted native SFNT metadata")?;
        Ok(Rc::new(Self {
            raster_phase_counts: Cell::new([0;4]),
            owner,
            bytes: Arc::from(bytes),
            count,
            traits,
            instance_identity,
        }))
    }
    pub(crate) fn metrics(&self) -> [i32; 4] {
        unsafe {
            [
                CTFontGetAscent(self.owner.0),
                CTFontGetDescent(self.owner.0),
                CTFontGetLeading(self.owner.0),
                CTFontGetCapHeight(self.owner.0),
            ]
            .map(|v| (v * 64.) as i32)
        }
    }
    pub(crate) fn decoration(&self) -> [i32; 2] {
        unsafe {
            [
                (CTFontGetUnderlineThickness(self.owner.0) * 64.) as i32,
                (-CTFontGetUnderlinePosition(self.owner.0) * 64.) as i32,
            ]
        }
    }
    pub(crate) fn advance(&self, id: u32) -> Result<f64, String> {
        let g = u16::try_from(id).map_err(|_| "Native glyph ID invalid")?;
        if g >= self.count {
            return Err("Native glyph ID invalid".into());
        }
        let mut a = Size::default();
        unsafe { CTFontGetAdvancesForGlyphs(self.owner.0, 0, &g, &mut a, 1) };
        if !a.width.is_finite() || a.width.abs() > 1e6 {
            return Err("Native advance invalid".into());
        }
        Ok(a.width)
    }
    pub(crate) fn right_bearing_26_6(&self,id:u32)->Result<i32,String>{
        let g=u16::try_from(id).map_err(|_|"Native glyph ID invalid")?;if g>=self.count{return Err("Native glyph ID invalid".into());}
        let rect=unsafe{CTFontGetBoundingRectsForGlyphs(self.owner.0,0,&g,std::ptr::null_mut(),1)};
        let value=(self.advance(id)?-rect.origin.x-rect.size.width)*64.;
        if !value.is_finite()||value<i32::MIN as f64||value>i32::MAX as f64{return Err("Native glyph bearing outside fixed domain".into());}
        Ok(value as i32)
    }
    /// Source getUnscaledGlyph quantizes at pixel size and again at the native
    /// em size before QFontSubset rounds into its 2048-unit metric domain.
    pub(crate) fn subset_horizontal_metrics(&self, id: u32) -> Result<[f64; 2], String> {
        let g = u16::try_from(id).map_err(|_| "Native glyph ID invalid")?;
        if g >= self.count { return Err("Native glyph ID invalid".into()); }
        let rect = unsafe { CTFontGetBoundingRectsForGlyphs(self.owner.0, 0, &g, std::ptr::null_mut(), 1) };
        let em = f64::from(ttf_parser::Face::parse(&self.bytes, 0)
            .map_err(|_| "Native subset metadata invalid")?.units_per_em());
        let raw = [self.advance(id)?, rect.origin.x];
        if em <= 0. || raw.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err("Native subset metric invalid".into());
        }
        Ok(raw.map(|v| subset_metric(v, em)))
    }
    pub(crate) fn outline(&self, id: u32) -> Result<Vec<PathOp>, String> {
        let g = u16::try_from(id).map_err(|_| "Native glyph ID invalid")?;
        if g >= self.count {
            return Err("Native glyph ID invalid".into());
        }
        let raw = unsafe { CTFontCreatePathForGlyph(self.owner.0, g, std::ptr::null()) };
        if raw.is_null() {
            return Ok(Vec::new());
        }
        let mut path = Path {
            ops: Vec::new(),
            failed: false,
        };
        unsafe {
            CGPathApply(raw, (&mut path as *mut Path).cast(), path_element);
            CGPathRelease(raw)
        };
        if path.failed {
            return Err("Native glyph outline admission failed".into());
        }
        Ok(path.ops)
    }
    /// Qt probes twelve raster positions once, using the first nonempty outline.
    /// Font engine state retains the resulting count across later glyphs/zooms.
    pub(crate) fn raster_phase_count(&self,glyph:u32,scale:f64,shear:f64,bold_offset:f64)->Result<u8,String>{
        self.raster_phase_count_impl::<false>(glyph,scale,shear,bold_offset).map(|v|v.0)
    }
    pub(crate) fn observe_raster_phase_count(&self,glyph:u32,scale:f64,shear:f64,bold_offset:f64)->Result<(u8,Option<serde_json::Value>),String>{
        self.raster_phase_count_impl::<true>(glyph,scale,shear,bold_offset)
    }
    fn raster_phase_count_impl<const OBSERVE:bool>(&self,glyph:u32,scale:f64,shear:f64,bold_offset:f64)->Result<(u8,Option<serde_json::Value>),String>{
        let style=usize::from(shear!=0.)*2+usize::from(bold_offset!=0.);
        let mut counts=self.raster_phase_counts.get();
        if counts[style]!=0 {
            let observation=if OBSERVE{Some(serde_json::json!({"cache":"warm","glyph":glyph,"scale":scale,"shear":shear,"bold_offset":bold_offset,"style":style,"before":counts,"after":counts,"masks":[],"scope":"No new raster probes; original acquisition was not observed"}))}else{None};
            return Ok((counts[style],observation));
        }
        if self.outline(glyph)?.is_empty(){return Ok((0,None));}
        let before=counts;
        let mut images:Vec<crate::paper_text::RasterGlyph>=Vec::new();
        images.try_reserve_exact(12).map_err(|_|"Native raster phase allocation failed")?;
        let mut observed=Vec::new();
        if OBSERVE{observed.try_reserve_exact(12).map_err(|_|"Native phase diagnostic allocation refused")?;}
        let mut observed_bytes=0usize;
        let mut bytes=0usize;
        for i in 0..12 {
            let phase=(i*64/12) as u8;
            let image=self.raster(glyph,scale,[phase,0],shear,bold_offset)?;
            let duplicate=images.iter().position(|old|old.width==image.width&&old.height==image.height&&old.coverage==image.coverage);
            if OBSERVE {
                observed_bytes=observed_bytes.checked_add(image.coverage.len()).filter(|&n|n<=1024*1024).ok_or("Native phase diagnostic mask budget exceeded")?;
                observed.push(serde_json::json!({"probe":i,"phase":[phase,0],"left":image.left,"top":image.top,"width":image.width,"height":image.height,"duplicate_of_unique":duplicate,"coverage":image.coverage}));
            }
            if duplicate.is_some(){continue;}
            bytes=bytes.checked_add(image.coverage.len()).filter(|&n|n<=16*1024*1024).ok_or("Native raster phase byte budget exceeded")?;
            images.push(image);
        }
        let count=images.len()as u8;
        if count==0{return Err("Native raster phase count invariant failed".into());}
        counts[style]=count;self.raster_phase_counts.set(counts);
        let observation=if OBSERVE{Some(serde_json::json!({"cache":"cold","glyph":glyph,"scale":scale,"shear":shear,"bold_offset":bold_offset,"style":style,"before":before,"after":counts,"count":count,"masks":observed,"scope":"Actual twelve phase rasters used by this cache initialization, including duplicates"}))}else{None};
        Ok((count,observation))
    }
    pub(crate) fn raster(
        &self, id:u32, scale:f64, phase:[u8;2], shear:f64, bold_offset:f64,
    ) -> Result<crate::paper_text::RasterGlyph,String> {
        let image = self.raster_coverage(id,scale,phase,shear,bold_offset,false,0.)?;
        Ok(crate::paper_text::RasterGlyph{left:image.left,top:image.top,
            width:image.width,height:image.height,coverage:image.coverage})
    }
    pub(crate) fn raster_lcd(
        &self,id:u32,scale:f64,phase:[u8;2],shear:f64,bold_offset:f64,
    )->Result<crate::paper_text::RasterLcdGlyph,String>{
        self.raster_coverage(id,scale,phase,shear,bold_offset,true,0.)
    }
    pub(crate) fn raster_rotation(&self,id:u32,scale:f64,phase:[u8;2],shear:f64,bold_offset:f64,angle:f64)->Result<crate::paper_text::RasterGlyph,String>{
        let image=self.raster_coverage(id,scale,phase,shear,bold_offset,false,angle)?;
        Ok(crate::paper_text::RasterGlyph{left:image.left,top:image.top,width:image.width,height:image.height,coverage:image.coverage})
    }
    /// Capture device transform, using the retained native font and an owned
    /// temporary bitmap context. No font/face transform or size is mutated.
    pub(crate) fn raster_anisotropic(&self,id:u32,scale:[f64;2],phase:[u8;2])
        ->Result<crate::paper_text::RasterGlyph,String>{
        let image=self.raster_coverage_axes(id,scale,phase,0.,0.,false,0.)?;
        Ok(crate::paper_text::RasterGlyph{left:image.left,top:image.top,
            width:image.width,height:image.height,coverage:image.coverage})
    }
    fn raster_coverage(
        &self,id:u32,scale:f64,phase:[u8;2],shear:f64,bold_offset:f64,lcd:bool,angle:f64,
    )->Result<crate::paper_text::RasterLcdGlyph,String>{
        self.raster_coverage_axes(id,[scale;2],phase,shear,bold_offset,lcd,angle)
    }
    fn raster_coverage_axes(
        &self,id:u32,axes:[f64;2],phase:[u8;2],shear:f64,bold_offset:f64,lcd:bool,angle:f64,
    )->Result<crate::paper_text::RasterLcdGlyph,String>{
        let [scale,scale_y]=axes;
        if axes.iter().any(|v|!v.is_finite()||*v<=0.||*v>32.)
            || !angle.is_finite() || !scale.is_finite()
            || scale <= 0.
            || scale > 32.
            || phase.iter().any(|p| *p >= 64)
            || !shear.is_finite()
            || !bold_offset.is_finite() || bold_offset < 0. || bold_offset > 1e6
        {
            return Err("Native raster transform refused".into());
        }
        let g = u16::try_from(id).map_err(|_| "Native glyph ID invalid")?;
        if g >= self.count {
            return Err("Native glyph ID invalid".into());
        }
        let rect = unsafe {
            CTFontGetBoundingRectsForGlyphs(self.owner.0, 0, &g, std::ptr::null_mut(), 1)
        };
        let raw = [
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        ];
        if raw.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || rect.size.width < 0.
            || rect.size.height < 0.
        {
            return Err("Invalid native glyph rectangle".into());
        }
        let expressions = [
            rect.origin.x * scale,
            (-rect.origin.y - rect.size.height) * scale_y,
            (rect.size.width + rect.size.height * shear) * scale,
            rect.size.height * scale_y,
            bold_offset * scale,
        ];
        if expressions
            .iter()
            .any(|v| !v.is_finite() || v.abs() > f64::from(i32::MAX) / 64. - 4.)
        {
            return Err("Native transformed metric domain exceeded".into());
        }
        // QCoreTextFontEngine alphaMapBoundingBox: QFixed metrics, x-1,width+3,y-1,height+2.
        let fixed = |x: f64| ((x * 64.) as i32) as f64 / 64.;
        let mut x = fixed(fixed(rect.origin.x) * scale) - 1.;
        let mut y = fixed((-fixed(rect.origin.y) - fixed(rect.size.height)) * scale_y) - 1.;
        let mut w = raster_width(rect.size.width, rect.size.height, shear, scale);
        let mut h = fixed(fixed(rect.size.height) * scale_y) + 2.;
        let(sin,cos)=angle.to_radians().sin_cos();
        if angle!=0.{
            let mut bounds=[f64::INFINITY,f64::INFINITY,f64::NEG_INFINITY,f64::NEG_INFINITY];
            for xx in [fixed(rect.origin.x),fixed(rect.origin.x+rect.size.width)]{for yy in [-fixed(rect.origin.y),-fixed(rect.origin.y+rect.size.height)]{
                let xx=xx-yy*shear;let q=[scale*(xx*cos-yy*sin),scale_y*(xx*sin+yy*cos)];for i in 0..2{bounds[i]=bounds[i].min(q[i]);bounds[i+2]=bounds[i+2].max(q[i]);}
            }}
            x=fixed(bounds[0])-1.;y=fixed(bounds[1])-1.;w=fixed(bounds[2]-bounds[0])+3.;h=fixed(bounds[3]-bounds[1])+2.;
        }
        if [x, y, w, h].iter().any(|v| !v.is_finite() || v.abs() > 1e6) || w < 0. || h < 0. {
            return Err("Native raster bounds refused".into());
        }
        let width = w.ceil() as usize;
        let height = h.ceil() as usize;
        let pixels = width
            .checked_mul(height)
            .ok_or("Native raster byte overflow")?;
        if pixels > 4 * 1024 * 1024 {
            return Err("Native raster byte budget exceeded".into());
        }
        let mut rgba = Vec::<u8>::new();
        rgba.try_reserve_exact(pixels * 4)
            .map_err(|_| "Native raster allocation failed")?;
        rgba.resize(pixels * 4, 255);
        let black = unsafe { CGColorGetConstantColor(kCGColorBlack) };
        if black.is_null() {
            return Err("Native constant black color unavailable".into());
        }
        let space = unsafe { CGColorSpaceCreateWithName(kCGColorSpaceSRGB) };
        if space.is_null() {
            return Err("Native sRGB space unavailable".into());
        }
        let ctx = unsafe {
            CGBitmapContextCreate(
                rgba.as_mut_ptr().cast(),
                width,
                height,
                8,
                width * 4,
                space,
                NATIVE_BITMAP_BGRX,
            )
        };
        unsafe { CGColorSpaceRelease(space) };
        if ctx.is_null() {
            return Err("Native raster context unavailable".into());
        }
        unsafe {
            CGContextSetShouldAntialias(ctx, true);
            CGContextSetShouldSmoothFonts(ctx, true);
            CGContextSetFillColorWithColor(ctx, black);
            CGContextSetTextMatrix(
                ctx,
                Matrix {
                    a: scale*cos,
                    b: -scale_y*sin,
                    c: scale*(sin+cos*shear),
                    d: scale_y*(cos-sin*shear),
                    tx: 0.,
                    ty: 0.,
                },
            );
            CGContextSetTextPosition(
                ctx,
                -x.floor() + f64::from(phase[0]) / 64.,
                height as f64 + y - f64::from(phase[1]) / 64.,
            );
            CTFontDrawGlyphs(self.owner.0, &g, &Point::default(), 1, ctx);
            // QCoreTextFontEngine::imageForGlyph paints the second pass to
            // the RIGHT. The separate PDF writer policy shifts LEFT.
            if bold_offset > 0. {
                CGContextSetTextPosition(ctx,
                    -x.floor() + f64::from(phase[0]) / 64. + bold_offset * scale,
                    height as f64 + y - f64::from(phase[1]) / 64.);
                CTFontDrawGlyphs(self.owner.0, &g, &Point::default(), 1, ctx);
            }
            CGContextRelease(ctx)
        };
        let mut coverage = Vec::new();
        coverage
            .try_reserve_exact(pixels.checked_mul(if lcd {3}else{1}).ok_or("Native coverage overflow")?)
            .map_err(|_| "Native coverage allocation failed")?;
        if lcd {
            for p in rgba.chunks_exact(4) {coverage.extend_from_slice(&[255-p[2],255-p[1],255-p[0]]);}
        } else {
            coverage.extend(rgba.chunks_exact(4).map(native_bgrx_coverage));
        }
        Ok(crate::paper_text::RasterLcdGlyph {
            left: x.floor() as i32,
            top: -y.floor() as i32,
            width,
            height,
            coverage,
        })
    }
}

#[cfg(test)]
mod synthesis_metric_tests {
    use super::*;
    #[test]
    fn opaque_bgrx_inverts_before_gray_without_alpha_alias() {
        assert_eq!(native_bgrx_coverage(&[0, 0, 0, 255]), 255);
        assert_eq!(native_bgrx_coverage(&[255, 255, 255, 255]), 0);
        assert_eq!(native_bgrx_coverage(&[0, 0, 255, 255]), 167);
        assert_eq!(native_bgrx_coverage(&[255, 0, 0, 255]), 215);
        assert_ne!(native_bgrx_coverage(&[0, 0, 0, 255]), 168);
    }
    #[test]
    fn subset_uses_quantized_native_bearing_not_outline_minimum() {
        let c_bearing = 90. * 100. / 2048.;
        assert_eq!(subset_metric(c_bearing, 2048.), 90.);
        assert_eq!(subset_metric(-c_bearing, 2048.), -90.);
        assert_ne!(subset_metric(c_bearing, 2048.), 89.);
        assert_eq!(subset_metric(100., 1000.), 2048.);
        assert_eq!(subset_metric(0., 2048.), 0.);
    }
    #[test]
    fn source_float_angle_and_quantization_order() {
        assert_eq!(
            synthetic_italic_shear(),
            f64::from((14f32 * 0f32.acos() / 90f32).tan())
        );
        assert_ne!(synthetic_italic_shear(), 0.3);
        let (width, height, scale) = (69.12345, 67.23456, 1.);
        let shear = synthetic_italic_shear();
        let actual = raster_width(width, height, shear, scale);
        assert_eq!(
            actual,
            fixed_metric(fixed_metric(width + height * shear) * scale) + 3.
        );
        // Fixture covers fractional native metrics rather than integer-only cases.
        let previous =
            fixed_metric((fixed_metric(width) + fixed_metric(height) * shear) * scale) + 3.;
        assert_ne!(actual, previous);
    }
}
