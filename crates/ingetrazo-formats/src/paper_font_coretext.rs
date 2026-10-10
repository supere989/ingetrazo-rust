// SPDX-License-Identifier: GPL-3.0-or-later
//! Native CoreText matching; all CF Create/Copy results have scoped ownership.
use super::Request;
use std::{ffi::c_void, path::PathBuf};
type Ref = *const c_void;
pub(crate) struct Owned(pub(crate) Ref);
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
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(p: Ref);
    fn CFDataGetLength(p: Ref) -> isize;
    fn CFDataGetBytePtr(p: Ref) -> *const u8;
    fn CFStringCreateWithBytes(a: Ref, b: *const u8, n: isize, encoding: u32, external: u8) -> Ref;
    fn CFStringGetLength(s: Ref) -> isize;
    fn CFStringGetMaximumSizeForEncoding(n: isize, e: u32) -> isize;
    fn CFStringGetCString(s: Ref, b: *mut i8, n: isize, e: u32) -> u8;
    fn CFURLGetFileSystemRepresentation(u: Ref, absolute: u8, b: *mut u8, n: isize) -> u8;
    fn CFArrayGetCount(a: Ref) -> isize;
    fn CFArrayGetValueAtIndex(a: Ref, i: isize) -> Ref;
    fn CFCharacterSetIsLongCharacterMember(s: Ref, c: u32) -> u8;
    fn CFLocaleCopyPreferredLanguages() -> Ref;
    fn CFDictionaryCreate(
        a: Ref,
        keys: *const Ref,
        values: *const Ref,
        n: isize,
        k: Ref,
        v: Ref,
    ) -> Ref;
    fn CFDictionaryGetValue(d: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(n: Ref, kind: i32, out: *mut c_void) -> u8;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
}
#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    fn CTFontCreateWithName(n: Ref, size: f64, matrix: Ref) -> Ref;
    fn CTFontGetSymbolicTraits(f: Ref) -> u32;
    fn CTFontCopyCharacterSet(f: Ref) -> Ref;
    fn CTFontCopyDefaultCascadeListForLanguages(f: Ref, languages: Ref) -> Ref;
    fn CTFontCreateWithFontDescriptor(d: Ref, size: f64, matrix: Ref) -> Ref;
    fn CTFontCopyAttribute(f: Ref, key: Ref) -> Ref;
    fn CTFontCopyPostScriptName(f: Ref) -> Ref;
    fn CTFontCopyFamilyName(f: Ref) -> Ref;
    fn CTFontCopyTable(f: Ref, tag: u32, options: u32) -> Ref;
    static kCTFontURLAttribute: Ref;
    static kCTFontFamilyNameAttribute: Ref;
    static kCTFontTraitsAttribute: Ref;
    static kCTFontWeightTrait: Ref;
    static kCTFontStyleNameAttribute: Ref;
    static kCTFontSlantTrait: Ref;
    fn CTFontManagerCopyAvailableFontFamilyNames() -> Ref;
    fn CTFontDescriptorCreateWithAttributes(a: Ref) -> Ref;
    fn CTFontDescriptorCreateMatchingFontDescriptors(d: Ref, mandatory: Ref) -> Ref;
    fn CTFontDescriptorCopyAttribute(d: Ref, key: Ref) -> Ref;
}
const UTF8: u32 = 0x08000100;
pub(crate) fn text(s: Ref) -> Result<String, String> {
    unsafe {
        let n = CFStringGetMaximumSizeForEncoding(CFStringGetLength(s), UTF8);
        if !(0..=4096).contains(&n) {
            return Err("CoreText name budget exceeded".into());
        }
        let mut b = vec![0u8; n as usize + 1];
        if CFStringGetCString(s, b.as_mut_ptr().cast(), b.len() as isize, UTF8) == 0 {
            return Err("Invalid CoreText name".into());
        }
        let end = b
            .iter()
            .position(|v| *v == 0)
            .ok_or("Invalid CoreText name")?;
        String::from_utf8(b[..end].to_vec()).map_err(|_| "Invalid CoreText UTF-8 name".into())
    }
}
/// A virtual cascade face may expose native tables without a file URL. Resolve
/// their stored PostScript identities through CoreText, then verify exact metadata
/// before borrowing a physical descriptor URL. Keep the original CTFont for glyphs.
fn physical_url(font: Ref, bytes: &[u8]) -> Result<Owned, String> {
    let url = unsafe { CTFontCopyAttribute(font, kCTFontURLAttribute) };
    if !url.is_null() {
        return Owned::new(url);
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|e| format!("Virtual font metadata parse: {e:?}"))?;
    let head = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"head"))
        .ok_or("Virtual font head unavailable")?;
    let names = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"name"))
        .ok_or("Virtual font names unavailable")?;
    if head.len() != 54 || names.len() > 256 * 1024 {
        return Err("Virtual font physical identity budget exceeded".into());
    }
    let mut seen = Vec::new();
    for name in face
        .names()
        .into_iter()
        .filter_map(crate::paper_font_resolver::sfnt_postscript_name)
    {
        if seen.contains(&name) {
            continue;
        }
        if seen.len() >= 16 {
            return Err("Virtual font identity candidate budget exceeded".into());
        }
        seen.push(name.clone());
        let cf = Owned::new(unsafe {
            CFStringCreateWithBytes(
                std::ptr::null(),
                name.as_ptr(),
                name.len() as isize,
                UTF8,
                0,
            )
        })?;
        let candidate = Owned::new(unsafe { CTFontCreateWithName(cf.0, 100., std::ptr::null()) })?;
        let mut exact = true;
        for (tag, expected) in [(b"head", head), (b"name", names)] {
            let table = unsafe { CTFontCopyTable(candidate.0, u32::from_be_bytes(*tag), 0) };
            if table.is_null() {
                exact = false;
                break;
            }
            let table = Owned::new(table)?;
            let length = unsafe { CFDataGetLength(table.0) };
            if usize::try_from(length).ok() != Some(expected.len()) {
                exact = false;
                break;
            }
            let ptr = unsafe { CFDataGetBytePtr(table.0) };
            if ptr.is_null() {
                return Err("Invalid virtual font identity table pointer".into());
            }
            if unsafe { std::slice::from_raw_parts(ptr, expected.len()) } != expected {
                exact = false;
                break;
            }
        }
        if exact {
            let url = unsafe { CTFontCopyAttribute(candidate.0, kCTFontURLAttribute) };
            if !url.is_null() {
                return Owned::new(url);
            }
        }
    }
    Err("CoreText virtual font has no exact physical metadata URL".into())
}
fn covers(font: Ref, chars: &[char]) -> Result<bool, String> {
    let set = Owned::new(unsafe { CTFontCopyCharacterSet(font) })?;
    Ok(chars
        .iter()
        .all(|c| unsafe { CFCharacterSetIsLongCharacterMember(set.0, *c as u32) != 0 }))
}
fn family_name(descriptor: Ref) -> Result<String, String> {
    let raw = unsafe { CTFontDescriptorCopyAttribute(descriptor, kCTFontFamilyNameAttribute) };
    if !raw.is_null() {
        return text(Owned::new(raw)?.0);
    }
    // Sparse cascade descriptors need not carry family attributes; the instantiated
    // font provides the resolved family, as Qt's getFontDescription does.
    let font =
        Owned::new(unsafe { CTFontCreateWithFontDescriptor(descriptor, 100., std::ptr::null()) })
            .map_err(|e| format!("CoreText cascade font creation: {e}"))?;
    let name = Owned::new(unsafe { CTFontCopyFamilyName(font.0) })
        .map_err(|e| format!("CoreText resolved cascade family: {e}"))?;
    text(name.0)
}
fn match_family(family: &str, request: &Request) -> Result<Option<Owned>, String> {
    unsafe {
        let name = Owned::new(CFStringCreateWithBytes(
            std::ptr::null(),
            family.as_ptr(),
            family.len() as isize,
            UTF8,
            0,
        ))?;
        let key = kCTFontFamilyNameAttribute;
        let value = name.0;
        let attributes = Owned::new(CFDictionaryCreate(
            std::ptr::null(),
            &key,
            &value,
            1,
            (&kCFTypeDictionaryKeyCallBacks as *const u8).cast(),
            (&kCFTypeDictionaryValueCallBacks as *const u8).cast(),
        ))?;
        let descriptor = Owned::new(CTFontDescriptorCreateWithAttributes(attributes.0))?;
        let raw = CTFontDescriptorCreateMatchingFontDescriptors(descriptor.0, std::ptr::null());
        if raw.is_null() {
            return Ok(None);
        }
        let matches = Owned::new(raw)?;
        let count = CFArrayGetCount(matches.0);
        if !(0..=512).contains(&count) {
            return Err("Native family descriptor budget exceeded".into());
        }
        let mut slots: Vec<(i32, bool, String, Owned)> = Vec::new();
        for i in 0..count {
            let d = CFArrayGetValueAtIndex(matches.0, i);
            if family_name(d)?.to_lowercase() != family.to_lowercase() {
                continue;
            }
            let font = Owned::new(CTFontCreateWithFontDescriptor(d, 100., std::ptr::null()))?;
            let traits = Owned::new(CTFontCopyAttribute(font.0, kCTFontTraitsAttribute))?;
            let number = CFDictionaryGetValue(traits.0, kCTFontWeightTrait);
            let mut weight = 0.;
            if !number.is_null() {
                CFNumberGetValue(number, 13, (&mut weight as *mut f64).cast());
            }
            // Qt converts native weight to nearest QFont weight before matching family styles.
            let weights: [(f64, i32); 9] = [
                (-0.8, 100),
                (-0.6, 200),
                (-0.4, 300),
                (0., 400),
                (0.23, 500),
                (0.3, 600),
                (0.4, 700),
                (0.56, 800),
                (0.62, 900),
            ];
            let qt = weights
                .iter()
                .min_by(|a, b| (a.0 - weight).abs().total_cmp(&(b.0 - weight).abs()))
                .unwrap()
                .1;
            let number = CFDictionaryGetValue(traits.0, kCTFontSlantTrait);
            let mut slant = 0.;
            if !number.is_null() {
                CFNumberGetValue(number, 13, (&mut slant as *mut f64).cast());
            }
            let italic = slant > 0.;
            let style = Owned::new(CTFontDescriptorCopyAttribute(d, kCTFontStyleNameAttribute))
                .map_err(|e| format!("CoreText style name for {family:?}: {e}"))?;
            let style = text(style.0)?;
            if let Some(slot) = slots
                .iter_mut()
                .find(|(w, i, s, _)| *w == qt && *i == italic && *s == style)
            {
                slot.3 = font
            } else {
                slots
                    .try_reserve(1)
                    .map_err(|_| "Native style registry allocation failed")?;
                slots.push((qt, italic, style, font));
            }
        }
        // Source bestStyle takes first minimum; equivalent registered slots replace handles.
        let mut best = None;
        let mut score = i32::MAX;
        for (weight, italic, _, font) in slots {
            let next = ((weight - if request.bold { 700 } else { 400 }) / 10).abs()
                + if italic != request.italic { 0x1000 } else { 0 };
            if next < score {
                score = next;
                best = Some(font)
            }
        }
        if let Some(font) = best {
            if covers(font.0, &request.required_chars)? {
                Ok(Some(font))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
pub(super) fn match_font(
    request: &Request,
) -> Result<
    (
        PathBuf,
        i32,
        std::rc::Rc<crate::paper_coretext_glyph::NativeFont>,
    ),
    String,
> {
    unsafe {
        // Generic Qt sans family maps to the native system sans rather than a Linux family.
        let family = if request.family == "Sans Serif" {
            "Helvetica"
        } else {
            &request.family
        };
        let name = Owned::new(CFStringCreateWithBytes(
            std::ptr::null(),
            family.as_ptr(),
            family.len() as isize,
            UTF8,
            0,
        ))?;
        let base = Owned::new(CTFontCreateWithName(name.0, 100., std::ptr::null()))?;
        let available = Owned::new(CTFontManagerCopyAvailableFontFamilyNames())?;
        let count = CFArrayGetCount(available.0);
        if !(0..=8192).contains(&count) {
            return Err("Native family inventory budget exceeded".into());
        }
        let registered = (0..count).any(|i| {
            text(CFArrayGetValueAtIndex(available.0, i))
                .is_ok_and(|n| n.eq_ignore_ascii_case(family))
        });
        let mut selected = if registered {
            match_family(family, request)?
        } else {
            None
        };
        if selected.is_none() {
            let languages = Owned::new(CFLocaleCopyPreferredLanguages())?;
            let cascade = Owned::new(CTFontCopyDefaultCascadeListForLanguages(
                base.0,
                languages.0,
            ))?;
            let count = CFArrayGetCount(cascade.0);
            if !(0..=512).contains(&count) {
                return Err("CoreText cascade budget exceeded".into());
            }
            for i in 0..count {
                let descriptor = CFArrayGetValueAtIndex(cascade.0, i);
                let name = family_name(descriptor)?;
                // Qt private cascade families register exactly their provided descriptor, not synthesized variants.
                let candidate = if name.starts_with('.') || name == "LastResort" {
                    Some(Owned::new(CTFontCreateWithFontDescriptor(
                        descriptor,
                        100.,
                        std::ptr::null(),
                    ))?)
                } else {
                    match_family(&name, request)?
                };
                if let Some(candidate) = candidate {
                    if covers(candidate.0, &request.required_chars)? {
                        selected = Some(candidate);
                        break;
                    }
                }
            }
        }
        let selected =
            selected.ok_or("No source-style native fallback covers the requested characters")?;
        let ptr = selected.0;
        std::mem::forget(selected);
        let font =
            crate::paper_coretext_glyph::NativeFont::from_raw(ptr, CTFontGetSymbolicTraits(ptr))?;
        let url = physical_url(ptr, &font.bytes)
            .map_err(|e| format!("CoreText selected file URL for {family:?}: {e}"))?;
        let mut path = [0u8; 4097];
        if CFURLGetFileSystemRepresentation(url.0, 1, path.as_mut_ptr(), path.len() as isize) == 0 {
            return Err("CoreText font file path unavailable".into());
        }
        let len = path
            .iter()
            .position(|v| *v == 0)
            .ok_or("CoreText font path budget exceeded")?;
        use std::os::unix::ffi::OsStrExt;
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(&path[..len]));
        let postscript = Owned::new(CTFontCopyPostScriptName(ptr))?;
        let postscript = text(postscript.0)?;
        // Native CTFont retains the selected instance. Full TTC bytes are never read.
        let mut file = std::fs::File::open(&path)
            .map_err(|e| format!("CoreText font identity file unavailable: {e}"))?;
        let index = match crate::paper_font_sfnt::collection_index_match(&mut file, &postscript)? {
            Some(index) => index,
            None => {
                let face = ttf_parser::Face::parse(&font.bytes, 0)
                    .map_err(|e| format!("Native identity metadata parse: {e:?}"))?;
                let head = face
                    .raw_face()
                    .table(ttf_parser::Tag::from_bytes(b"head"))
                    .ok_or("Missing native identity head")?;
                let name = face
                    .raw_face()
                    .table(ttf_parser::Tag::from_bytes(b"name"))
                    .ok_or("Missing native identity name")?;
                crate::paper_font_sfnt::collection_index_by_tables(&mut file, head, name).map_err(
                    |e| {
                        format!("Native PostScript identity {postscript:?} has no stored face; {e}")
                    },
                )?
            }
        };
        Ok((path, index as i32, font))
    }
}
