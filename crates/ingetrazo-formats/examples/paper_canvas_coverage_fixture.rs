//! Observes actual shared FT raster coverage before GPU compositing; no painter replacement.
use ingetrazo_formats::paper_font_resolver::{Request,Resolver};
fn main(){
 let mut resolver=Resolver::new().unwrap();let result=resolver.resolve(Request{family:"Sans Serif".into(),bold:false,italic:false,required_chars:vec![]}).unwrap();let mut rows=Vec::new();
 for text in ["0","1","/","6"]{let glyph=result.font.shape_run(text,None).unwrap()[0].id;for scale in [14.*25.4/72.*0.75/100.*1.5]{for phase in [0,16,32,48]{let b=result.font.rasterize_glyph(glyph,scale,[phase,0],false).unwrap();rows.push(serde_json::json!({"text":text,"glyph":glyph,"scale":scale,"phase":phase,"left":b.left,"top":b.top,"width":b.width,"height":b.height,"coverage":b.coverage}));}}}
 println!("{}",serde_json::json!({"font_path":result.path,"rows":rows}));
}
