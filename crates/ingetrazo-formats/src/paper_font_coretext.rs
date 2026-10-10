// SPDX-License-Identifier: GPL-3.0-or-later
//! Native CoreText matching; all CF Create/Copy results have scoped ownership.
use super::{Request, FtPolicy, Hinting};
use std::{ffi::c_void, path::PathBuf};
type Ref = *const c_void;
struct Owned(Ref);
impl Owned { fn new(p: Ref) -> Result<Self,String> { if p.is_null(){Err("CoreText resource unavailable".into())}else{Ok(Self(p))} } }
impl Drop for Owned {fn drop(&mut self){unsafe{CFRelease(self.0)}}}
#[link(name="CoreFoundation",kind="framework")]
unsafe extern "C" {
 fn CFRelease(p:Ref);
 fn CFStringCreateWithBytes(a:Ref,b:*const u8,n:isize,encoding:u32,external:u8)->Ref;
 fn CFStringGetLength(s:Ref)->isize;
 fn CFStringGetMaximumSizeForEncoding(n:isize,e:u32)->isize;
 fn CFStringGetCString(s:Ref,b:*mut i8,n:isize,e:u32)->u8;
 fn CFURLGetFileSystemRepresentation(u:Ref,absolute:u8,b:*mut u8,n:isize)->u8;
 fn CFArrayGetCount(a:Ref)->isize;
 fn CFArrayGetValueAtIndex(a:Ref,i:isize)->Ref;
 fn CFCharacterSetIsLongCharacterMember(s:Ref,c:u32)->u8;
}
#[link(name="CoreText",kind="framework")]
unsafe extern "C" {
 fn CTFontCreateWithName(n:Ref,size:f64,matrix:Ref)->Ref;
 fn CTFontCreateCopyWithSymbolicTraits(f:Ref,size:f64,matrix:Ref,value:u32,mask:u32)->Ref;
 fn CTFontGetSymbolicTraits(f:Ref)->u32;
 fn CTFontCopyCharacterSet(f:Ref)->Ref;
 fn CTFontCopyDefaultCascadeListForLanguages(f:Ref,languages:Ref)->Ref;
 fn CTFontCreateWithFontDescriptor(d:Ref,size:f64,matrix:Ref)->Ref;
 fn CTFontCopyAttribute(f:Ref,key:Ref)->Ref;
 fn CTFontCopyPostScriptName(f:Ref)->Ref;
 static kCTFontURLAttribute:Ref;
}
const UTF8:u32=0x08000100;
fn text(s:Ref)->Result<String,String>{unsafe{
 let n=CFStringGetMaximumSizeForEncoding(CFStringGetLength(s),UTF8);
 if !(0..=4096).contains(&n){return Err("CoreText name budget exceeded".into())}
 let mut b=vec![0u8;n as usize+1];
 if CFStringGetCString(s,b.as_mut_ptr().cast(),b.len() as isize,UTF8)==0{return Err("Invalid CoreText name".into())}
 let end=b.iter().position(|v|*v==0).ok_or("Invalid CoreText name")?;
 String::from_utf8(b[..end].to_vec()).map_err(|_|"Invalid CoreText UTF-8 name".into())
}}
fn covers(font:Ref, chars:&[char])->Result<bool,String>{let set=Owned::new(unsafe{CTFontCopyCharacterSet(font)})?;Ok(chars.iter().all(|c|unsafe{CFCharacterSetIsLongCharacterMember(set.0,*c as u32)!=0}))}
fn styled(font:Ref, request:&Request)->Result<Owned,String>{
 let traits=u32::from(request.italic)| (u32::from(request.bold)<<1);
 let out=Owned::new(unsafe{CTFontCreateCopyWithSymbolicTraits(font,100.,std::ptr::null(),traits,3)})?;
 if unsafe{CTFontGetSymbolicTraits(out.0)}&3!=traits{return Err("CoreText requested style requires unsupported synthetic traits".into())}
 Ok(out)
}
pub(super) fn match_font(request:&Request)->Result<(PathBuf,i32,FtPolicy,Option<std::sync::Arc<[u8]>>),String>{unsafe{
 // Generic Qt sans family maps to the native system sans rather than a Linux family.
 let family=if request.family=="Sans Serif"{"Helvetica"}else{&request.family};
 let name=Owned::new(CFStringCreateWithBytes(std::ptr::null(),family.as_ptr(),family.len() as isize,UTF8,0))?;
 let base=Owned::new(CTFontCreateWithName(name.0,100.,std::ptr::null()))?;
 let mut selected=styled(base.0,request)?;
 if !covers(selected.0,&request.required_chars)?{
  let cascade=Owned::new(CTFontCopyDefaultCascadeListForLanguages(selected.0,std::ptr::null()))?;
  let count=CFArrayGetCount(cascade.0);
  if !(0..=512).contains(&count){return Err("CoreText cascade budget exceeded".into())}
  let mut found=None;
  for i in 0..count{
   let candidate=Owned::new(CTFontCreateWithFontDescriptor(CFArrayGetValueAtIndex(cascade.0,i),100.,std::ptr::null()))?;
   if let Ok(candidate)=styled(candidate.0,request){if covers(candidate.0,&request.required_chars)?{found=Some(candidate);break}}
  }
  selected=found.ok_or("No CoreText fallback covers the requested characters/style")?;
 }
 let url=Owned::new(CTFontCopyAttribute(selected.0,kCTFontURLAttribute))?;
 let mut path=[0u8;4097];
 if CFURLGetFileSystemRepresentation(url.0,1,path.as_mut_ptr(),path.len() as isize)==0{return Err("CoreText font file path unavailable".into())}
 let len=path.iter().position(|v|*v==0).ok_or("CoreText font path budget exceeded")?;
 use std::os::unix::ffi::OsStrExt;
 let path=PathBuf::from(std::ffi::OsStr::from_bytes(&path[..len]));
 let postscript=Owned::new(CTFontCopyPostScriptName(selected.0))?;
 let postscript=text(postscript.0)?;
 let metadata=std::fs::metadata(&path).map_err(|e|format!("CoreText font file unavailable: {e}"))?;
 if !metadata.is_file()||metadata.len()>crate::paper_text::MAX_FONT_BYTES as u64{return Err("CoreText font file budget exceeded".into())}
 // The collection's actual SFNT identity decides the index; never assume face zero.
 use std::io::Read;
 let mut bytes=Vec::new();bytes.try_reserve_exact(metadata.len() as usize).map_err(|_|"CoreText font file allocation failed")?;
 std::fs::File::open(&path).map_err(|e|e.to_string())?.take(crate::paper_text::MAX_FONT_BYTES as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()>crate::paper_text::MAX_FONT_BYTES{return Err("CoreText font grew beyond byte budget".into())}
 let count=ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
 if count>4096{return Err("CoreText font collection budget exceeded".into())}
 for index in 0..count{
  let face=ttf_parser::Face::parse(&bytes,index).map_err(|_|"Invalid CoreText font face")?;
  if face.names().into_iter().any(|n|n.name_id==ttf_parser::name_id::POST_SCRIPT_NAME&&n.to_string().as_deref()==Some(postscript.as_str())){
   if face.is_bold()!=request.bold||face.is_italic()!=request.italic{return Err("CoreText style requires unsupported synthetic or variable face traits".into())}
   if !request.required_chars.iter().all(|ch|face.glyph_index(*ch).is_some()){return Err("CoreText file face does not cover its matched character set".into())}
   return Ok((path,index as i32,FtPolicy{hinting:Hinting::None,force_auto_hint:false},Some(std::sync::Arc::from(bytes))))
  }
 }
 Err("CoreText face identity cannot be mapped to its font collection".into())
}}
