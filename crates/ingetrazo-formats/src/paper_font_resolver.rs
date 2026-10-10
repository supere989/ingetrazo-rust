// SPDX-License-Identifier: GPL-3.0-or-later
//! In-process native paper font matching. Cache refresh is explicit; no command-line resolver.
use crate::{
    paper_font::{CachedFont, FontCache, FontKey},
    paper_text::{FtPolicy, Hinting},
};
use std::{
    collections::{HashMap, VecDeque},
    ffi::{c_char, c_void, CStr, CString},
    path::PathBuf,
    ptr::NonNull,
    rc::Rc,
    sync::Arc,
};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Request {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub required_chars: Vec<char>,
}
#[derive(Clone, Debug)]
pub struct Match {
    pub path: PathBuf,
    pub index: u32,
    pub policy: FtPolicy,
    pub font: Rc<CachedFont>,
}
struct CachedMatch {
    path: PathBuf,
    index: u32,
    policy: FtPolicy,
    font: std::rc::Weak<CachedFont>,
}
#[cfg(not(target_os = "macos"))]
type Ptr = *mut c_void;
#[cfg(not(target_os = "macos"))]
struct Api {
    init: unsafe extern "C" fn() -> Ptr,
    config_destroy: unsafe extern "C" fn(Ptr),
    create: unsafe extern "C" fn() -> Ptr,
    destroy: unsafe extern "C" fn(Ptr),
    string: unsafe extern "C" fn(Ptr, *const c_char, *const u8) -> i32,
    integer: unsafe extern "C" fn(Ptr, *const c_char, i32) -> i32,
    double: unsafe extern "C" fn(Ptr, *const c_char, f64) -> i32,
    substitute: unsafe extern "C" fn(Ptr, Ptr, i32) -> i32,
    defaults: unsafe extern "C" fn(Ptr),
    match_font: unsafe extern "C" fn(Ptr, Ptr, *mut i32) -> Ptr,
    get_string: unsafe extern "C" fn(Ptr, *const c_char, i32, *mut *mut u8) -> i32,
    get_integer: unsafe extern "C" fn(Ptr, *const c_char, i32, *mut i32) -> i32,
    get_bool: unsafe extern "C" fn(Ptr, *const c_char, i32, *mut i32) -> i32,
    charset_create: unsafe extern "C" fn() -> Ptr,
    charset_destroy: unsafe extern "C" fn(Ptr),
    charset_add: unsafe extern "C" fn(Ptr, u32) -> i32,
    pattern_charset: unsafe extern "C" fn(Ptr, *const c_char, Ptr) -> i32,
    _library: libloading::Library,
}
#[cfg(not(target_os = "macos"))]
struct Pattern<'a> {
    ptr: NonNull<c_void>,
    api: &'a Api,
}
#[cfg(not(target_os = "macos"))]
impl Drop for Pattern<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.ptr.as_ptr()) }
    }
}
#[cfg(not(target_os = "macos"))]
impl Api {
    fn load() -> Result<Self, String> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err("This native font provider requires Linux Fontconfig; other platform providers remain unimplemented".into());
        }
        #[cfg(target_os = "linux")]
        unsafe {
            let library = libloading::Library::new("libfontconfig.so.1")
                .map_err(|e| format!("Native Fontconfig unavailable: {e}"))?;
            macro_rules! sym {
                ($n:literal) => {
                    *library
                        .get(concat!($n, "\0").as_bytes())
                        .map_err(|e| format!("Native Fontconfig ABI unavailable: {e}"))?
                };
            }
            Ok(Self {
                init: sym!("FcInitLoadConfigAndFonts"),
                config_destroy: sym!("FcConfigDestroy"),
                create: sym!("FcPatternCreate"),
                destroy: sym!("FcPatternDestroy"),
                string: sym!("FcPatternAddString"),
                integer: sym!("FcPatternAddInteger"),
                double: sym!("FcPatternAddDouble"),
                substitute: sym!("FcConfigSubstitute"),
                defaults: sym!("FcDefaultSubstitute"),
                match_font: sym!("FcFontMatch"),
                get_string: sym!("FcPatternGetString"),
                get_integer: sym!("FcPatternGetInteger"),
                get_bool: sym!("FcPatternGetBool"),
                charset_create: sym!("FcCharSetCreate"),
                charset_destroy: sym!("FcCharSetDestroy"),
                charset_add: sym!("FcCharSetAddChar"),
                pattern_charset: sym!("FcPatternAddCharSet"),
                _library: library,
            })
        }
    }
}
/// Resolves one face per request; paragraph fallback segmentation is a separate layout responsibility.
pub struct Resolver {
    #[cfg(not(target_os = "macos"))]
    config: NonNull<c_void>,
    #[cfg(not(target_os = "macos"))]
    api: Api,
    fonts: FontCache,
    requests: HashMap<Request, CachedMatch>,
    order: VecDeque<Request>,
    namespace: uuid::Uuid,
    matches: u64,
}
impl Drop for Resolver {
    fn drop(&mut self) {
        #[cfg(not(target_os = "macos"))]
        unsafe { (self.api.config_destroy)(self.config.as_ptr()) }
    }
}
impl Resolver {
    pub fn new() -> Result<Self, String> {
        #[cfg(not(target_os = "macos"))]
        let api = Api::load()?;
        #[cfg(not(target_os = "macos"))]
        let config =
            NonNull::new(unsafe { (api.init)() }).ok_or("Native font configuration unavailable")?;
        Ok(Self {
            #[cfg(not(target_os = "macos"))]
            config,
            #[cfg(not(target_os = "macos"))]
            api,
            fonts: FontCache::default(),
            requests: HashMap::new(),
            order: VecDeque::new(),
            namespace: uuid::Uuid::new_v4(),
            matches: 0,
        })
    }
    /// Explicit installed-font/config epoch replacement; outstanding resources retain their old identity safely.
    pub fn refresh(&mut self) -> Result<(), String> {
        #[cfg(not(target_os = "macos"))]
        {
        let replacement = NonNull::new(unsafe { (self.api.init)() })
            .ok_or("Native font configuration refresh failed")?;
        let old = std::mem::replace(&mut self.config, replacement);
        unsafe { (self.api.config_destroy)(old.as_ptr()) };
        }
        self.requests.clear();
        self.order.clear();
        self.namespace = uuid::Uuid::new_v4();
        self.fonts.clear();
        Ok(())
    }
    /// Request cache ownership counters, including its bounded FIFO bookkeeping.
    pub fn request_cache_entries(&self) -> (usize, usize) {
        (self.requests.len(), self.order.len())
    }
    pub fn stats(&self) -> (usize, u64, (usize, usize, u64)) {
        (self.requests.len(), self.matches, self.fonts.stats())
    }
    #[cfg(target_os = "macos")]
    fn match_native(&self, request: &Request, _: &CString) -> Result<(PathBuf, i32, FtPolicy, Option<Arc<[u8]>>), String> {
        coretext::match_font(request)
    }
    #[cfg(not(target_os = "macos"))]
    fn match_native(&self, request: &Request, family: &CString) -> Result<(PathBuf, i32, FtPolicy, Option<Arc<[u8]>>), String> {
        let pattern = Pattern {
            ptr: NonNull::new(unsafe { (self.api.create)() })
                .ok_or("Native font pattern allocation failed")?,
            api: &self.api,
        };
        let ptr = pattern.ptr.as_ptr();
        unsafe {
            if (self.api.string)(ptr, c"family".as_ptr(), family.as_ptr().cast()) == 0
                || (self.api.integer)(ptr, c"weight".as_ptr(), if request.bold { 200 } else { 80 })
                    == 0
                || (self.api.integer)(ptr, c"slant".as_ptr(), if request.italic { 100 } else { 0 })
                    == 0
                || (self.api.double)(ptr, c"pixelsize".as_ptr(), 100.) == 0
            {
                return Err("Native font pattern allocation failed".into());
            }
            if !request.required_chars.is_empty() {
                let charset = NonNull::new((self.api.charset_create)())
                    .ok_or("Native font charset allocation failed")?;
                let mut ok = true;
                for ch in &request.required_chars {
                    ok &= (self.api.charset_add)(charset.as_ptr(), *ch as u32) != 0;
                }
                ok &= (self.api.pattern_charset)(ptr, c"charset".as_ptr(), charset.as_ptr()) != 0;
                (self.api.charset_destroy)(charset.as_ptr());
                if !ok {
                    return Err("Native font charset allocation failed".into());
                }
            }
            if (self.api.substitute)(self.config.as_ptr(), ptr, 0) == 0 {
                return Err("Native font configuration substitution failed".into());
            }
            (self.api.defaults)(ptr);
        }
        let mut result = 0;
        let matched = Pattern {
            ptr: NonNull::new(unsafe {
                (self.api.match_font)(self.config.as_ptr(), ptr, &mut result)
            })
            .ok_or("No native font matched")?,
            api: &self.api,
        };
        if result != 0 {
            return Err("No native font matched".into());
        }
        let ptr = matched.ptr.as_ptr();
        let mut file = std::ptr::null_mut();
        let mut index = 0;
        let mut hintstyle = 1;
        let mut hinting = 1;
        let mut autohint = 0;
        unsafe {
            if (self.api.get_string)(ptr, c"file".as_ptr(), 0, &mut file) != 0 || file.is_null() {
                return Err("Matched native font has no file".into());
            }
            (self.api.get_integer)(ptr, c"index".as_ptr(), 0, &mut index);
            (self.api.get_integer)(ptr, c"hintstyle".as_ptr(), 0, &mut hintstyle);
            (self.api.get_bool)(ptr, c"hinting".as_ptr(), 0, &mut hinting);
            (self.api.get_bool)(ptr, c"autohint".as_ptr(), 0, &mut autohint);
        }
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStrExt;
            PathBuf::from(std::ffi::OsStr::from_bytes(
                unsafe { CStr::from_ptr(file.cast()) }.to_bytes(),
            ))
        };
        #[cfg(not(unix))]
        let path = PathBuf::from(
            unsafe { CStr::from_ptr(file.cast()) }
                .to_string_lossy()
                .as_ref(),
        );
        if index < 0 || path.as_os_str().len() > 4096 {
            return Err("Invalid native font file/index".into());
        }
        let policy = FtPolicy {
            hinting: if hinting == 0 || hintstyle == 0 {
                Hinting::None
            } else if hintstyle == 1 {
                Hinting::Vertical
            } else {
                Hinting::Full
            },
            force_auto_hint: autohint != 0,
        };
        Ok((path, index, policy, None))
    }
    pub fn resolve(&mut self, mut request: Request) -> Result<Match, String> {
        if request.family.is_empty() {
            request.family = "Sans Serif".into();
        }
        if request.family.len() > 1024 || request.required_chars.len() > 256 {
            return Err("Paper font request budget exceeded".into());
        }
        request.required_chars.sort_unstable();
        request.required_chars.dedup();
        let family =
            CString::new(request.family.as_bytes()).map_err(|_| "Invalid paper font family")?;
        if let Some(found) = self.requests.get(&request) {
            if let Some(font) = found.font.upgrade() {
                return Ok(Match {
                    path: found.path.clone(),
                    index: found.index,
                    policy: found.policy,
                    font,
                });
            }
        }
        let (path, index, policy, matched_bytes) = self.match_native(&request, &family)?;
        // Stable physical identity across FIFO/font eviction and external retained owners.
        // No geometry equality deduplication or growing historical identity map.
        let mut identity_name = Vec::new();
        identity_name
            .try_reserve_exact(path.as_os_str().as_encoded_bytes().len() + 6)
            .map_err(|_| "Paper font identity allocation failed")?;
        identity_name.extend_from_slice(path.as_os_str().as_encoded_bytes());
        identity_name.extend_from_slice(&(index as u32).to_be_bytes());
        identity_name.push(match policy.hinting {
            Hinting::None => 0,
            Hinting::Vertical => 1,
            Hinting::Full => 2,
        });
        identity_name.push(u8::from(policy.force_auto_hint));
        let identity = uuid::Uuid::new_v5(&self.namespace, &identity_name);
        let key = FontKey {
            identity,
            face_index: index as u32,
            policy,
        };
        let font = if let Some(font) = self.fonts.get(key) {
            font
        } else if let Some(bytes) = matched_bytes {
            self.fonts.load(key, bytes)?
        } else {
            let metadata = std::fs::metadata(&path)
                .map_err(|e| format!("Paper font file unavailable: {e}"))?;
            if !metadata.is_file() || metadata.len() > crate::paper_text::MAX_FONT_BYTES as u64 {
                return Err("Paper font file budget exceeded".into());
            }
            use std::io::Read;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(metadata.len() as usize)
                .map_err(|_| "Paper font file allocation failed")?;
            std::fs::File::open(&path)
                .map_err(|e| e.to_string())?
                .take(crate::paper_text::MAX_FONT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            self.fonts.load(key, Arc::from(bytes))?
        };
        let found = Match {
            path,
            index: index as u32,
            policy,
            font,
        };
        // Admit both containers before removing/replacing any published entry.
        // A refused reserve may change capacity, but never logical queue/map ownership.
        self.requests
            .try_reserve(1)
            .map_err(|_| "Paper font request allocation failed")?;
        self.order
            .try_reserve(1)
            .map_err(|_| "Paper font request allocation failed")?;
        // Replace an expired request only after the complete cold match/load succeeds.
        self.order.retain(|old| old != &request);
        if self.requests.len() >= 64 && !self.requests.contains_key(&request) {
            if let Some(old) = self.order.pop_front() {
                self.requests.remove(&old);
            }
        }
        self.order.push_back(request.clone());
        self.requests.insert(
            request,
            CachedMatch {
                path: found.path.clone(),
                index: found.index,
                policy: found.policy,
                font: Rc::downgrade(&found.font),
            },
        );
        self.matches = self.matches.saturating_add(1);
        Ok(found)
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    #[test]
    fn missing_native_provider_is_an_explicit_error() {
        assert!(matches!(Resolver::new(), Err(error) if error.contains("other platform providers remain unimplemented")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refused_request_preserves_published_fifo_and_owners() {
        let mut resolver = Resolver::new().unwrap();
        let request = Request {
            family: "DejaVu Sans".into(),
            bold: false,
            italic: false,
            required_chars: vec![],
        };
        let held = resolver.resolve(request.clone()).unwrap();
        let order = resolver.order.clone();
        let stats = resolver.stats();
        for family in ["bad\0family".to_owned(), "x".repeat(1025)] {
            let mut invalid = request.clone();
            invalid.family = family;
            assert!(resolver.resolve(invalid).is_err());
            assert_eq!(resolver.order, order);
            assert_eq!(resolver.stats(), stats);
            assert_eq!(resolver.requests.len(), resolver.order.len());
            assert!(Rc::ptr_eq(&resolver.resolve(request.clone()).unwrap().font, &held.font));
        }
    }
}

#[cfg(target_os = "macos")]
#[path = "paper_font_coretext.rs"]
mod coretext;

/// Decode a physical SFNT PostScript identity, including Macintosh Roman records.
/// PostScript names are ASCII; other legacy encodings must not be guessed.
pub fn sfnt_postscript_name(name: ttf_parser::name::Name<'_>) -> Option<String> {
    if name.name_id != ttf_parser::name_id::POST_SCRIPT_NAME { return None; }
    name.to_string().or_else(|| {
        (name.platform_id == ttf_parser::PlatformId::Macintosh
            && name.encoding_id == 0 && !name.name.is_empty()
            && name.name.iter().all(|b| (33..=126).contains(b)))
            .then(|| String::from_utf8(name.name.to_vec()).expect("ASCII name"))
    })
}

#[cfg(test)]
mod postscript_identity_tests {
    use super::sfnt_postscript_name;
    #[test]
    fn mac_roman_identity_is_exact_and_other_encodings_are_not_guessed() {
        let name = ttf_parser::name::Name { platform_id: ttf_parser::PlatformId::Macintosh,
            encoding_id: 0, language_id: 0, name_id: ttf_parser::name_id::POST_SCRIPT_NAME,
            name: b"Helvetica-Bold" };
        assert_eq!(sfnt_postscript_name(name).as_deref(), Some("Helvetica-Bold"));
        assert_eq!(sfnt_postscript_name(ttf_parser::name::Name { encoding_id: 1, ..name }), None);
        assert_eq!(sfnt_postscript_name(ttf_parser::name::Name { name: b"bad\xff", ..name }), None);
        assert_eq!(sfnt_postscript_name(ttf_parser::name::Name { name_id: 1, ..name }), None);
        let unicode = ttf_parser::name::Name { platform_id: ttf_parser::PlatformId::Unicode,
            name: b"\0A\0B", ..name };
        assert_eq!(sfnt_postscript_name(unicode).as_deref(), Some("AB"));
    }
}
