// SPDX-License-Identifier: GPL-3.0-or-later
//! Original Qt paper subset geometry. Scoped cache; no subprocess or renderer geometry narrowing.
use freetype_sys as ft;
use std::{collections::HashMap, ffi::c_void, ptr::NonNull, sync::Arc};
pub const MAX_SUBSET_GLYPHS: usize = u16::MAX as usize;
const MAX_CACHED_GLYPHS: usize = 4096;
pub const MAX_SUBSET_POINTS: usize = 256 * 1024;
pub const MAX_FONT_PROGRAM_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug)]
enum Op {
    Move([f64; 2]),
    Line([f64; 2]),
    Cubic([[f64; 2]; 3]),
}
#[derive(Default)]
struct Outline {
    ops: Vec<Op>,
    last: [f64; 2],
    failed: bool,
}
fn point(v: *const ft::FT_Vector) -> [f64; 2] {
    unsafe { [(*v).x as f64 / 64., (*v).y as f64 / 64.] }
}
extern "C" fn mv(v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    o.last = point(v);
    if !o.push(Op::Move(o.last)) {
        return 1;
    }
    0
}
extern "C" fn ln(v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let next = point(v);
    if next != o.last {
        if !o.push(Op::Line(next)) {
            return 1;
        }
    }
    o.last = next;
    0
}
extern "C" fn quad(c: *const ft::FT_Vector, v: *const ft::FT_Vector, d: *mut c_void) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let c = point(c);
    let e = point(v);
    let a = o.last;
    let first = [(a[0] + 2. * c[0]) / 3., (a[1] + 2. * c[1]) / 3.];
    let second = [(e[0] + 2. * c[0]) / 3., (e[1] + 2. * c[1]) / 3.];
    if !o.push(Op::Cubic([first, second, e])) {
        return 1;
    }
    o.last = e;
    0
}
extern "C" fn cubic(
    a: *const ft::FT_Vector,
    b: *const ft::FT_Vector,
    v: *const ft::FT_Vector,
    d: *mut c_void,
) -> i32 {
    let o = unsafe { &mut *d.cast::<Outline>() };
    let e = point(v);
    if !o.push(Op::Cubic([point(a), point(b), e])) {
        return 1;
    }
    o.last = e;
    0
}
#[derive(Clone, Copy)]
struct P {
    xy: [i32; 2],
    on: bool,
}
fn reduce(
    out: &mut Vec<P>,
    end: [i32; 2],
    second: [i32; 2],
    first: [i32; 2],
    start: [i32; 2],
) -> Result<(), String> {
    let mut stack = vec![[end, second, first, start]];
    let mut try_reduce = out.len() > 1 && out[out.len() - 1].on && !out[out.len() - 2].on;
    let mut split = 0;
    while let Some(v) = stack.pop() {
        let a = [
            v[1][0] + ((v[1][0] - v[0][0]) >> 1),
            v[1][1] + ((v[1][1] - v[0][1]) >> 1),
        ];
        let b = [
            v[2][0] + ((v[2][0] - v[3][0]) >> 1),
            v[2][1] + ((v[2][1] - v[3][1]) >> 1),
        ];
        if (a[0] - b[0]).abs() <= 3 && (a[1] - b[1]).abs() <= 3 {
            let xy = [(a[0] + b[0]) >> 1, (a[1] + b[1]) >> 1];
            if try_reduce {
                let prev = out[out.len() - 2].xy;
                let mid = [(prev[0] + v[2][0]) >> 1, (prev[1] + v[2][1]) >> 1];
                if (mid[0] - v[3][0]).abs() <= 3 && (mid[1] - v[3][1]).abs() <= 3 {
                    out.pop();
                }
                try_reduce = false;
            }
            out.push(P { xy, on: false });
        } else {
            split += 1;
            if split > 16 {
                return Err("source cubic subdivision bound".into());
            }
            let mid = |a: [i32; 2], b: [i32; 2]| [(a[0] + b[0]) >> 1, (a[1] + b[1]) >> 1];
            let a = mid(v[0], v[1]);
            let b = mid(v[3], v[2]);
            let c = mid(v[1], v[2]);
            let aa = mid(a, c);
            let bb = mid(b, c);
            let middle = mid(aa, bb);
            stack.push([v[0], a, aa, middle]);
            stack.push([middle, bb, b, v[3]]);
        }
    }
    Ok(())
}
fn convert(ops: &[Op], em: f64) -> Result<(Vec<[i32; 3]>, Vec<usize>), String> {
    let mut out: Vec<P> = Vec::new();
    out.try_reserve_exact(ops.len().saturating_mul(34).min(65536))
        .map_err(|_| "Paper glyph point allocation failed")?;
    let mut ends: Vec<usize> = Vec::new();
    ends.try_reserve_exact(ops.len())
        .map_err(|_| "Paper contour allocation failed")?;
    let scaled = |v: [f64; 2]| {
        [
            (v[0] * 2048. / em).round() as i32,
            (v[1] * 2048. / em).round() as i32,
        ]
    };
    for op in ops {
        let v: &[[f64; 2]] = match op {
            Op::Move(p) | Op::Line(p) => std::slice::from_ref(p),
            Op::Cubic(p) => p,
        };
        if v.iter().flatten().any(|v| {
            !v.is_finite()
                || (*v * 2048. / em).round() < i16::MIN as f64
                || (*v * 2048. / em).round() > i16::MAX as f64
        }) {
            return Err("Paper glyph coordinate domain exceeded".into());
        }
    }
    for (i, op) in ops.iter().enumerate() {
        if out.len() > 65536 - 64 {
            return Err("Paper glyph point budget exceeded".into());
        }
        match op {
            Op::Move(v) => {
                if i + 1 == ops.len() {
                    continue;
                }
                if i != 0 {
                    let start = if let Some(v) = ends.last() {
                        v.saturating_sub(1)
                    } else {
                        0
                    };
                    if out.last().unwrap().xy == out[start].xy {
                        out.pop();
                    }
                    ends.push(out.len() - 1);
                }
                out.push(P {
                    xy: scaled(*v),
                    on: true,
                });
            }
            Op::Line(v) => {
                if i + 1 < ops.len() {
                    out.push(P {
                        xy: scaled(*v),
                        on: true,
                    });
                }
            }
            Op::Cubic(v) => {
                let end = scaled(v[2]);
                let start = out.last().ok_or("missingcurveorigin")?.xy;
                reduce(&mut out, end, scaled(v[1]), scaled(v[0]), start)?;
                out.push(P { xy: end, on: true });
            }
        }
    }
    if !out.is_empty() {
        let start = ends.last().map_or(0, |v| v + 1);
        if out.last().unwrap().xy == out[start].xy {
            out.pop();
        }
        ends.push(out.len() - 1);
    }
    Ok((
        out.into_iter()
            .map(|p| [p.xy[0], p.xy[1], i32::from(p.on)])
            .collect(),
        ends,
    ))
}

impl Outline {
    fn push(&mut self, op: Op) -> bool {
        if self.ops.len() >= 65536 || self.ops.try_reserve(1).is_err() {
            self.failed = true;
            return false;
        }
        self.ops.push(op);
        true
    }
}
#[derive(Debug)]
pub(crate) struct SubsetGlyph {
    pub points: Vec<[i32; 3]>,
    pub ends: Vec<usize>,
    pub advance: u16,
    pub lsb: i16,
}
/// Thread-confined cached face, sharing immutable input bytes with the shaping face.
pub(crate) struct GlyphStore {
    face: Option<NonNull<ft::FT_FaceRec>>,
    library: Option<NonNull<ft::FT_LibraryRec>>,
    #[cfg(target_os="macos")]native:Option<std::rc::Rc<crate::paper_coretext_glyph::NativeFont>>,
    _bytes: Arc<[u8]>,
    em: u16,
    count: u16,
    glyphs: HashMap<u32, Arc<SubsetGlyph>>,
    points: usize,
}
impl Drop for GlyphStore {
    fn drop(&mut self) {
        unsafe {
            if let Some(face)=self.face{ft::FT_Done_Face(face.as_ptr());}
            if let Some(library)=self.library{ft::FT_Done_FreeType(library.as_ptr());}
        }
    }
}
impl GlyphStore {
    pub fn new(bytes: Arc<[u8]>, index: u32) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > crate::paper_text::MAX_FONT_BYTES {
            return Err("Paper font byte budget exceeded".into());
        }
        let font = ttf_parser::Face::parse(&bytes, index).map_err(|_| "Invalid paper font")?;
        if font.is_variable() {
            return Err("Paper variable font instance policy remains unavailable".into());
        }
        let (em, count) = (font.units_per_em(), font.number_of_glyphs());
        if em == 0 || count == 0 {
            return Err("Invalid paper font metrics".into());
        }
        unsafe {
            let mut lib = std::ptr::null_mut();
            if ft::FT_Init_FreeType(&mut lib) != 0 {
                return Err("Paper subset library failed".into());
            }
            let Some(library) = NonNull::new(lib) else {
                return Err("Paper subset library allocation failed".into());
            };
            let mut face = std::ptr::null_mut();
            if ft::FT_New_Memory_Face(lib, bytes.as_ptr(), bytes.len() as _, index as _, &mut face)
                != 0
            {
                ft::FT_Done_FreeType(lib);
                return Err("Paper subset face failed".into());
            }
            let Some(face) = NonNull::new(face) else {
                ft::FT_Done_FreeType(lib);
                return Err("Paper subset face allocation failed".into());
            };
            if ft::FT_Set_Char_Size(
                face.as_ptr(),
                (i64::from(em) * 64) as _,
                (i64::from(em) * 64) as _,
                0,
                0,
            ) != 0
            {
                ft::FT_Done_Face(face.as_ptr());
                ft::FT_Done_FreeType(lib);
                return Err("Paper subset scale failed".into());
            }
            Ok(Self {
                face:Some(face),
                library:Some(library),
                #[cfg(target_os="macos")]native:None,
                _bytes: bytes,
                em,
                count,
                glyphs: HashMap::new(),
                points: 0,
            })
        }
    }
    #[cfg(target_os="macos")]
    pub(crate) fn new_coretext(native:std::rc::Rc<crate::paper_coretext_glyph::NativeFont>)->Result<Self,String>{
        Ok(Self{face:None,library:None,em:100,count:native.count,_bytes:native.bytes.clone(),native:Some(native),glyphs:HashMap::new(),points:0})
    }
    #[cfg(target_os="macos")]
    fn load_coretext(&mut self,id:u32)->Result<Arc<SubsetGlyph>,String>{
        use crate::paper_coretext_glyph::PathOp;
        let n=self.native.as_ref().expect("Native backend");let mut outline=Outline::default();let mut first=[0.;2];
        for op in n.outline(id)?{match op{
            PathOp::Move(p)=>{first=p;outline.last=p;outline.push(Op::Move(p));},
            PathOp::Line(p)=>{outline.last=p;outline.push(Op::Line(p));},
            PathOp::Quad([c,e])=>{let a=outline.last;outline.push(Op::Cubic([[(a[0]+2.*c[0])/3.,(a[1]+2.*c[1])/3.],[(e[0]+2.*c[0])/3.,(e[1]+2.*c[1])/3.],e]));outline.last=e;},
            PathOp::Cubic(p)=>{outline.push(Op::Cubic(p));outline.last=p[2];},
            PathOp::Close=>{if outline.last!=first{outline.push(Op::Line(first));}outline.last=first;}
        }}
        if outline.failed{return Err("Native subset outline refused".into())}
        let(points,ends)=convert(&outline.ops,100.)?;
        let metrics=n.subset_horizontal_metrics(id)?;
        let advance=u16::try_from(metrics[0] as i64).map_err(|_|"Native subset advance invalid")?;
        let lsb=i16::try_from(metrics[1] as i64).map_err(|_|"Native subset bearing invalid")?;
        let glyph=Arc::new(SubsetGlyph{points,ends,advance,lsb});let total=self.points.checked_add(glyph.points.len()).ok_or("Native point overflow")?;
        if self.glyphs.len()<MAX_CACHED_GLYPHS&&total<=MAX_SUBSET_POINTS{self.glyphs.try_reserve(1).map_err(|_|"Native glyph cache allocation failed")?;self.glyphs.insert(id,glyph.clone());self.points=total;}
        Ok(glyph)
    }
    pub fn stats(&self) -> (usize, usize) {
        (self.glyphs.len(), self.points)
    }
    pub fn load(&mut self, id: u32) -> Result<Arc<SubsetGlyph>, String> {
        if let Some(g) = self.glyphs.get(&id) {
            return Ok(g.clone());
        }
        if id >= u32::from(self.count) {
            return Err("Paper subset glyph identity invalid".into());
        }
        #[cfg(target_os="macos")]if self.native.is_some(){return self.load_coretext(id)}
        unsafe {
            if ft::FT_Load_Glyph(self.face.expect("FreeType backend").as_ptr(), id, ft::FT_LOAD_NO_BITMAP) != 0 {
                return Err("Paper subset glyph load failed".into());
            }
            let slot = (*self.face.expect("FreeType backend").as_ptr()).glyph;
            if slot.is_null() {
                return Err("Paper subset glyph slot unavailable".into());
            }
            if (*slot).format != ft::FT_GLYPH_FORMAT_OUTLINE {
                return Err("Paper bitmap font subset remains unavailable".into());
            }
            let raw = &mut (*slot).outline;
            if raw.n_points < 0 || raw.n_contours < 0 {
                return Err("Invalid paper font outline counts".into());
            }
            if raw.n_points > 0 {
                if raw.points.is_null()
                    || raw.tags.is_null()
                    || raw.contours.is_null()
                    || raw.n_contours == 0
                {
                    return Err("Invalid paper font outline pointers".into());
                }
                let ends = std::slice::from_raw_parts(raw.contours, raw.n_contours as usize);
                let mut previous = -1;
                for end in ends {
                    if *end <= previous || *end >= raw.n_points {
                        return Err("Invalid paper font outline contour".into());
                    }
                    previous = *end;
                }
                if previous != raw.n_points - 1 {
                    return Err("Invalid paper font outline endpoint".into());
                }
            } else if raw.n_contours != 0 {
                return Err("Invalid empty paper font outline".into());
            }
            let mut outline = Outline::default();
            if raw.n_points > 0 {
                let funcs = ft::FT_Outline_Funcs {
                    move_to: mv,
                    line_to: ln,
                    conic_to: quad,
                    cubic_to: cubic,
                    shift: 0,
                    delta: 0,
                };
                if ft::FT_Outline_Decompose(raw, &funcs, (&mut outline as *mut Outline).cast()) != 0
                    || outline.failed
                {
                    return Err("Paper font outline conversion failed".into());
                }
            }
            let (points, ends) = convert(&outline.ops, f64::from(self.em))?;
            let total = self
                .points
                .checked_add(points.len())
                .ok_or("Paper subset point count overflow")?;
            let metric = |v: i64| -> Result<i32, String> {
                let value = (v as f64 / 64. * 2048. / f64::from(self.em)).round();
                if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
                    return Err("Invalid paper subset metric".into());
                }
                Ok(value as i32)
            };
            let advance = u16::try_from(metric((*slot).advance.x as i64)?)
                .map_err(|_| "Paper subset advance domain exceeded")?;
            let lsb = i16::try_from(metric((*slot).metrics.horiBearingX as i64)?)
                .map_err(|_| "Paper subset bearing domain exceeded")?;
            let glyph = Arc::new(SubsetGlyph {
                points,
                ends,
                advance,
                lsb,
            });
            if self.glyphs.len() < MAX_CACHED_GLYPHS && total <= MAX_SUBSET_POINTS {
                self.glyphs
                    .try_reserve(1)
                    .map_err(|_| "Paper subset glyph cache allocation failed")?;
                self.glyphs.insert(id, glyph.clone());
                self.points = total;
            }
            Ok(glyph)
        }
    }
}

pub(crate) struct Program {
    pub bytes: Arc<[u8]>,
    pub ids: Vec<u32>,
    pub cids: std::collections::BTreeMap<u32, u16>,
    pub widths: Vec<f32>,
    pub bounds_px: Vec<[f64; 4]>,
}
fn append(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    if out
        .len()
        .checked_add(bytes.len())
        .is_none_or(|n| n > MAX_FONT_PROGRAM_BYTES)
    {
        return Err("Paper font program byte budget exceeded".into());
    }
    out.try_reserve(bytes.len())
        .map_err(|_| "Paper font program allocation failed")?;
    out.extend_from_slice(bytes);
    Ok(())
}
fn short(out: &mut Vec<u8>, v: i16) -> Result<(), String> {
    append(out, &v.to_be_bytes())
}
fn checksum(b: &[u8]) -> u32 {
    b.chunks(4).fold(0u32, |sum, v| {
        let mut a = [0u8; 4];
        a[..v.len()].copy_from_slice(v);
        sum.wrapping_add(u32::from_be_bytes(a))
    })
}
pub(crate) fn ordered_ids(ids: &[u32]) -> Result<Vec<u32>, String> {
    if ids.len() > crate::paper_text::MAX_GLYPHS {
        return Err("Paper text glyph budget exceeded".into());
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(ids.len().saturating_add(1).min(MAX_SUBSET_GLYPHS))
        .map_err(|_| "Paper subset identity allocation failed")?;
    let mut seen = std::collections::HashSet::new();
    seen.try_reserve(ids.len().saturating_add(1).min(MAX_SUBSET_GLYPHS))
        .map_err(|_| "Paper subset identity allocation failed")?;
    result.push(0);
    seen.insert(0);
    for id in ids {
        if seen.insert(*id) {
            if result.len() >= MAX_SUBSET_GLYPHS {
                return Err("Paper subset glyph budget exceeded".into());
            }
            result.push(*id);
        }
    }
    Ok(result)
}
pub(crate) fn build(
    store: &mut GlyphStore,
    bytes: &[u8],
    index: u32,
    ids: Vec<u32>,
) -> Result<Program, String> {
    let face = ttf_parser::Face::parse(bytes, index).map_err(|_| "Invalid paper font")?;
    let raw = face.raw_face();
    let table = |tag| {
        raw.table(ttf_parser::Tag::from_bytes(tag))
            .ok_or("Paper font subset table unavailable")
    };
    let head_source = table(b"head")?;
    let hhea_source = table(b"hhea")?;
    let name = table(b"name")?;
    let os2 = table(b"OS/2")?;
    if head_source.len() < 54 || hhea_source.len() < 36 {
        return Err("Invalid paper font subset table".into());
    }
    let mut glyphs = Vec::new();
    glyphs
        .try_reserve_exact(ids.len())
        .map_err(|_| "Paper subset allocation failed")?;
    let mut needed = 12usize
        + 8 * 16
        + 54
        + 36
        + 32
        + name.len()
        + os2.len()
        + (ids.len() + 1) * 4
        + ids.len() * 4
        + 32;
    if needed > MAX_FONT_PROGRAM_BYTES {
        return Err("Paper font program byte budget exceeded".into());
    }
    for id in &ids {
        let g = store.load(*id)?;
        needed = needed
            .checked_add(12 + g.ends.len() * 2 + g.points.len() * 5 + 3)
            .ok_or("Paper subset byte count overflow")?;
        if needed > MAX_FONT_PROGRAM_BYTES {
            return Err("Paper font program byte budget exceeded".into());
        }
        glyphs.push(g);
    }
    let mut glyf = Vec::new();
    glyf.try_reserve(needed)
        .map_err(|_| "Paper subset allocation failed")?;
    let mut loca = Vec::new();
    loca.try_reserve_exact((ids.len() + 1) * 4)
        .map_err(|_| "Paper subset allocation failed")?;
    let mut hmtx = Vec::new();
    hmtx.try_reserve_exact(ids.len() * 4)
        .map_err(|_| "Paper subset allocation failed")?;
    let mut glyph_bounds = Vec::new();
    glyph_bounds
        .try_reserve_exact(ids.len())
        .map_err(|_| "Paper subset allocation failed")?;
    let mut max_points = 0usize;
    let mut max_contours = 0usize;
    let mut bounds = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];
    let mut widths = Vec::new();
    widths
        .try_reserve_exact(ids.len())
        .map_err(|_| "Paper subset allocation failed")?;
    for g in &glyphs {
        append(&mut loca, &(glyf.len() as u32).to_be_bytes())?;
        append(&mut hmtx, &g.advance.to_be_bytes())?;
        short(&mut hmtx, g.lsb)?;
        widths.push((f64::from(g.advance) * 1000. / 2048.).round() as f32);
        max_points = max_points.max(g.points.len());
        max_contours = max_contours.max(g.ends.len());
        if g.points.is_empty() {
            glyph_bounds.push([0.; 4]);
            continue;
        }
        let bbox = [
            g.points.iter().map(|p| p[0]).min().unwrap(),
            g.points.iter().map(|p| p[1]).min().unwrap(),
            g.points.iter().map(|p| p[0]).max().unwrap(),
            g.points.iter().map(|p| p[1]).max().unwrap(),
        ];
        glyph_bounds.push(bbox.map(|v| f64::from(v) * 100. / 2048.));
        short(
            &mut glyf,
            i16::try_from(g.ends.len()).map_err(|_| "Paper contour count exceeded")?,
        )?;
        for (i, v) in bbox.iter().enumerate() {
            let value = i16::try_from(*v).map_err(|_| "Paper glyph coordinate domain exceeded")?;
            short(&mut glyf, value)?;
            bounds[i] = if i < 2 {
                bounds[i].min(value)
            } else {
                bounds[i].max(value)
            };
        }
        for end in &g.ends {
            append(
                &mut glyf,
                &u16::try_from(*end)
                    .map_err(|_| "Paper glyph point index exceeded")?
                    .to_be_bytes(),
            )?;
        }
        short(&mut glyf, 0)?;
        for p in &g.points {
            append(&mut glyf, &[p[2] as u8])?;
        }
        for axis in 0..2 {
            let mut previous = 0;
            for p in &g.points {
                short(
                    &mut glyf,
                    i16::try_from(p[axis] - previous)
                        .map_err(|_| "Paper glyph delta domain exceeded")?,
                )?;
                previous = p[axis];
            }
        }
        while glyf.len() % 4 != 0 {
            append(&mut glyf, &[0])?;
        }
    }
    append(&mut loca, &(glyf.len() as u32).to_be_bytes())?;
    let copy = |b: &[u8]| -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        v.try_reserve_exact(b.len())
            .map_err(|_| "Paper subset table allocation failed")?;
        v.extend_from_slice(b);
        Ok(v)
    };
    let mut head = copy(&head_source[..54])?;
    head[8..12].fill(0);
    head[16..18].fill(0);
    head[18..20].copy_from_slice(&2048u16.to_be_bytes());
    if bounds[0] == i16::MAX {
        bounds = [0; 4];
    }
    for (i, v) in bounds.iter().enumerate() {
        head[36 + 2 * i..38 + 2 * i].copy_from_slice(&v.to_be_bytes());
    }
    head[50..52].copy_from_slice(&1i16.to_be_bytes());
    let mut hhea = copy(&hhea_source[..36])?;
    hhea[34..36].copy_from_slice(&(ids.len() as u16).to_be_bytes());
    let mut maxp = vec![0u8; 32];
    maxp[..4].copy_from_slice(&0x10000u32.to_be_bytes());
    maxp[4..6].copy_from_slice(&(ids.len() as u16).to_be_bytes());
    maxp[6..8].copy_from_slice(
        &u16::try_from(max_points)
            .map_err(|_| "Paper glyph points exceeded")?
            .to_be_bytes(),
    );
    maxp[8..10].copy_from_slice(
        &u16::try_from(max_contours)
            .map_err(|_| "Paper glyph contours exceeded")?
            .to_be_bytes(),
    );
    let mut tables = std::collections::BTreeMap::from([
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"maxp", maxp),
        (*b"glyf", glyf),
        (*b"loca", loca),
        (*b"hmtx", hmtx),
        (*b"name", copy(name)?),
        (*b"OS/2", copy(os2)?),
    ]);
    let count = tables.len() as u16;
    let power = 1u16 << (15 - count.leading_zeros() as u16);
    let mut program = Vec::new();
    program
        .try_reserve_exact(needed)
        .map_err(|_| "Paper font program allocation failed")?;
    append(&mut program, &0x10000u32.to_be_bytes())?;
    append(&mut program, &count.to_be_bytes())?;
    append(&mut program, &(power * 16).to_be_bytes())?;
    append(&mut program, &(power.trailing_zeros() as u16).to_be_bytes())?;
    append(&mut program, &((count - power) * 16).to_be_bytes())?;
    let mut offset = 12 + 16 * tables.len();
    let mut head_offset = 0;
    for (tag, bytes) in &tables {
        append(&mut program, tag)?;
        append(&mut program, &checksum(bytes).to_be_bytes())?;
        append(&mut program, &(offset as u32).to_be_bytes())?;
        append(&mut program, &(bytes.len() as u32).to_be_bytes())?;
        if tag == b"head" {
            head_offset = offset;
        }
        offset += (bytes.len() + 3) & !3;
    }
    for bytes in tables.values_mut() {
        append(&mut program, bytes)?;
        while program.len() % 4 != 0 {
            append(&mut program, &[0])?;
        }
    }
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(checksum(&program));
    program[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    let cids = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i as u16))
        .collect();
    Ok(Program {
        bytes: program.into(),
        ids,
        cids,
        widths,
        bounds_px: glyph_bounds,
    })
}
