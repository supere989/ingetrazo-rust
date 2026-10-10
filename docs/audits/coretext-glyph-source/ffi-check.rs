extern crate self as ttf_parser;
pub struct Face;impl Face{pub fn parse(_: &[u8],_:u32)->Result<Self,()>{Ok(Self)}}
pub struct Request{family:String,bold:bool,italic:bool,required_chars:Vec<char>}
mod paper_text{pub const MAX_FONT_BYTES:usize=16*1024*1024;pub struct RasterGlyph{pub left:i32,pub top:i32,pub width:usize,pub height:usize,pub coverage:Vec<u8>}}
#[path="/tmp/ingetrazo-coretext-glyph/crates/ingetrazo-formats/src/paper_coretext_glyph.rs"]mod paper_coretext_glyph;
#[path="/tmp/ingetrazo-coretext-glyph/crates/ingetrazo-formats/src/paper_font_sfnt.rs"]mod paper_font_sfnt;
#[path="/tmp/ingetrazo-coretext-glyph/crates/ingetrazo-formats/src/paper_font_coretext.rs"]mod paper_font_coretext;
fn main(){}
