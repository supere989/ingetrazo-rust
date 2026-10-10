// SPDX-License-Identifier: GPL-3.0-or-later
//! In-process Linux paper font matching. Cache refresh is explicit; no command-line resolver.
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
type Ptr = *mut c_void;
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
struct Pattern<'a> {
    ptr: NonNull<c_void>,
    api: &'a Api,
}
impl Drop for Pattern<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.ptr.as_ptr()) }
    }
}
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
    config: NonNull<c_void>,
    api: Api,
    fonts: FontCache,
    requests: HashMap<Request, CachedMatch>,
    order: VecDeque<Request>,
    identities: HashMap<(PathBuf, u32, FtPolicy), uuid::Uuid>,
    matches: u64,
}
impl Drop for Resolver {
    fn drop(&mut self) {
        unsafe { (self.api.config_destroy)(self.config.as_ptr()) }
    }
}
impl Resolver {
    pub fn new() -> Result<Self, String> {
        let api = Api::load()?;
        let config =
            NonNull::new(unsafe { (api.init)() }).ok_or("Native font configuration unavailable")?;
        Ok(Self {
            config,
            api,
            fonts: FontCache::default(),
            requests: HashMap::new(),
            order: VecDeque::new(),
            identities: HashMap::new(),
            matches: 0,
        })
    }
    /// Explicit installed-font/config epoch replacement; outstanding resources retain their old identity safely.
    pub fn refresh(&mut self) -> Result<(), String> {
        let replacement = NonNull::new(unsafe { (self.api.init)() })
            .ok_or("Native font configuration refresh failed")?;
        let old = std::mem::replace(&mut self.config, replacement);
        unsafe { (self.api.config_destroy)(old.as_ptr()) };
        self.requests.clear();
        self.order.clear();
        self.identities.clear();
        self.fonts.clear();
        Ok(())
    }
    pub fn stats(&self) -> (usize, u64, (usize, usize, u64)) {
        (self.requests.len(), self.matches, self.fonts.stats())
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
        let identity_key = (path.clone(), index as u32, policy);
        let identity = if let Some(id) = self.identities.get(&identity_key) {
            *id
        } else {
            if self.identities.len() >= 128 {
                let used = self
                    .requests
                    .values()
                    .map(|m| (m.path.clone(), m.index, m.policy))
                    .collect::<std::collections::HashSet<_>>();
                self.identities.retain(|k, _| used.contains(k));
            }
            self.identities
                .try_reserve(1)
                .map_err(|_| "Paper font identity allocation failed")?;
            let id = uuid::Uuid::new_v4();
            self.identities.insert(identity_key, id);
            id
        };
        let key = FontKey {
            identity,
            face_index: index as u32,
            policy,
        };
        let font = if let Some(font) = self.fonts.get(key) {
            font
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
        if self.requests.len() >= 64 {
            if let Some(old) = self.order.pop_front() {
                self.requests.remove(&old);
            }
        }
        self.requests
            .try_reserve(1)
            .map_err(|_| "Paper font request allocation failed")?;
        self.order
            .try_reserve(1)
            .map_err(|_| "Paper font request allocation failed")?;
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
