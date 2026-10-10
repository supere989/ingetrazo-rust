extern crate self as ttf_parser;
pub struct Tag;impl Tag{pub fn from_bytes(_: &[u8;4])->Self{Self}}
pub struct Face;impl Face{pub fn parse(_: &[u8],_:u32)->Result<Self,()>{Ok(Self)}pub fn raw_face(&self)->&Self{self}pub fn table(&self,_:Tag)->Option<&[u8]>{None}pub fn names(&self)->Vec<Name>{Vec::new()}}
pub struct Name;
pub struct Request{family:String,bold:bool,italic:bool,required_chars:Vec<char>}
mod paper_font_resolver{pub fn sfnt_postscript_name(_:crate::Name)->Option<String>{None}}
mod paper_text{pub const MAX_FONT_BYTES:usize=16*1024*1024;pub struct RasterGlyph{pub left:i32,pub top:i32,pub width:usize,pub height:usize,pub coverage:Vec<u8>}}
#[path="/tmp/ingetrazo-coretext-native-identity/crates/ingetrazo-formats/src/paper_coretext_glyph.rs"]mod paper_coretext_glyph;
#[path="/tmp/ingetrazo-coretext-native-identity/crates/ingetrazo-formats/src/paper_font_sfnt.rs"]mod paper_font_sfnt;
#[path="/tmp/ingetrazo-coretext-native-identity/crates/ingetrazo-formats/src/paper_font_coretext.rs"]mod paper_font_coretext;
fn main(){}
