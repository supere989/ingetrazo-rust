// Actual input recipes only; production visual font runs, not paragraph/UI acceptance.
use ingetrazo_formats::{paper_font_resolver::{Request,Resolver},paper_text_layout::shape_line,sheet_pdf::{self,Draw,Page,TextDraw,TextGlyph}};
fn main(){
 let args=std::env::args().collect::<Vec<_>>();
 let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let out=std::path::Path::new(&args[2]);std::fs::create_dir_all(out).unwrap();
 let mut resolver=Resolver::new().unwrap();
 for case in input["cases"].as_array().unwrap(){
  let name=case["name"].as_str().unwrap();
  if !["fallback_bold","mixed_cjk_rtl","mixed_latin_cjk","mixed_scripts"].contains(&name){continue;}
  let call=&case["source_draw_calls"][0];let f=&call["font"];
  let request=Request{family:f["requested"].as_str().unwrap().into(),bold:f["bold"].as_bool().unwrap(),italic:f["italic"].as_bool().unwrap(),required_chars:vec![]};
  let line=shape_line(&mut resolver,&request,call["text"].as_str().unwrap()).unwrap();
  let mut draws=vec![];
  for run in line.runs {
   let mut x=run.x_26_6 as f64/64.;let mut glyphs=vec![];
   for glyph in run.glyphs {glyphs.push(TextGlyph{glyph:glyph.id,position_px:[x+glyph.offset[0]as f64/64.,line.ascent_26_6 as f64/64.+glyph.offset[1]as f64/64.]});x+=glyph.advance[0]as f64/64.;}
   draws.push(Draw{ rotation_deg:0.,origin_mm:[20.,30.],text:Some(TextDraw{rotation_deg:0.,font:run.font,scale_mm:14.*25.4/72./100.*0.75,italic:request.italic,bold:request.bold,glyphs,decorations:vec![]}),even_odd:true,path:vec![],stroke:None,stroke_options:None,fill:Some([30f32/255.,36f32/255.,44f32/255.,1.]),clip:None,clip_intersection:None,flatten_transform:false});
  }
  let page=Page{device_dpi:Some(300.),size_mm:[210.,297.],media_box_points:Some([595.,842.]),origin_mm:[10.08/(72./25.4);2],draws};
  sheet_pdf::save_original_qt_paper(&out.join(format!("{name}.native.pdf")),&[page]).unwrap();
 }
}
