// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(target_os="macos")]
fn main(){
 use ingetrazo_formats::paper_font_resolver::{Resolver,Request};
 let mut resolver=Resolver::new().expect("native CoreText provider");
 let mut output=Vec::new();
 let mut failed=false;
 for (family,bold,italic,text) in [("Helvetica",false,false,"ABC"),("Helvetica",true,false,"ABC"),("Helvetica",false,true,"ABC"),("Times",true,true,"ABC"),("Helvetica",false,false,"漢字"),("Helvetica",false,false,"العربية")]{
  let request=Request{family:family.into(),bold,italic,required_chars:text.chars().collect()};
  let matched=match resolver.resolve(request.clone()) {
   Ok(matched)=>matched,
   Err(error)=>{ failed=true; output.push(serde_json::json!({"family":family,"bold":bold,"italic":italic,"text":text,"error":error})); continue; }
  };
  let before=resolver.stats();let again=resolver.resolve(request).unwrap();
  assert!(std::rc::Rc::ptr_eq(&matched.font,&again.font));assert_eq!(before,resolver.stats());
  let bytes=matched.font.shared_bytes(); let face=ttf_parser::Face::parse(bytes,matched.font.key().face_index).unwrap();
  let covered=text.chars().all(|ch|face.glyph_index(ch).is_some()); assert!(covered);
  let postscript=face.names().into_iter().find_map(ingetrazo_formats::paper_font_resolver::sfnt_postscript_name);
  let glyphs=matched.font.shape_run(text,None).expect("native shaping");
  let ids=glyphs.iter().map(|g|g.id).collect::<Vec<_>>();
  let program=matched.font.subset(&ids).expect("native path/PDF subset");
  let mut rasters=Vec::new();for g in &glyphs{let bitmap=matched.font.rasterize_glyph(g.id,0.15,[13,7],italic).expect("native CoreGraphics raster");assert_eq!(bitmap.coverage.len(),bitmap.width*bitmap.height);assert!(bitmap.coverage.iter().any(|v|*v>0));rasters.push(serde_json::json!({"id":g.id,"left":bitmap.left,"top":bitmap.top,"width":bitmap.width,"height":bitmap.height,"coverage":bitmap.coverage}));}
  let stats=matched.font.stats();let repeated=matched.font.subset(&ids).unwrap();assert!(std::sync::Arc::ptr_eq(&program,&repeated));assert_eq!(stats,matched.font.stats());
  assert!(matched.font.rasterize_glyph(u32::MAX,0.15,[0,0],false).is_err());assert!(matched.font.rasterize_glyph(ids[0],f64::NAN,[0,0],false).is_err());assert!(matched.font.rasterize_glyph(ids[0],0.15,[64,0],false).is_err());assert_eq!(stats,matched.font.stats());
  output.push(serde_json::json!({"family":family,"bold":bold,"italic":italic,"text":text,"path":matched.path,"index":matched.index,"warm_same_owner":true,"chosen_bold":face.is_bold(),"chosen_italic":face.is_italic(),"pdf_synthetic_bold":matched.font.synthetic_bold_offset_px(bold)>0.,"pdf_synthetic_italic":matched.font.synthetic_italic_shear(italic)!=0.,"coverage":covered,"postscript":postscript,"owned_table_bytes":bytes.len(),"extracted_face_index":matched.font.key().face_index,"glyphs":glyphs.iter().map(|g|serde_json::json!({"id":g.id,"cluster":g.cluster,"advance":g.advance,"offset":g.offset})).collect::<Vec<_>>(),"metrics":[matched.font.ascent_26_6(),matched.font.descent_26_6(),matched.font.leading_26_6(),matched.font.cap_height_26_6()],"rasters":rasters,"pdf_subset_glyphs":program.glyph_count(),"pdf_subset_bytes":program.program_bytes(),"warm_subset_same_owner":true}));
 }
 let cold_request=Request{family:"Helvetica".into(),bold:false,italic:false,required_chars:vec!['A','B','C']};let held=resolver.resolve(cold_request.clone()).unwrap();assert_eq!(held.font.key().face_index,0);
 for i in 0..70{resolver.resolve(Request{family:"Helvetica".into(),bold:false,italic:false,required_chars:vec!['A',char::from_u32('a' as u32+i%26).unwrap(),char::from_u32('a' as u32+i/26).unwrap()]}).unwrap();}
 let before_matches=resolver.stats().1;let cold_again=resolver.resolve(cold_request).unwrap();assert_eq!(cold_again.index,held.index);assert_eq!(cold_again.font.key(),held.font.key());assert!(std::rc::Rc::ptr_eq(&cold_again.font,&held.font));assert!(resolver.stats().1>before_matches);let(entries,order)=resolver.request_cache_entries();assert_eq!(entries,order);assert!(entries<=64);
 let before=resolver.stats();assert!(resolver.resolve(Request{family:"bad\0family".into(),bold:false,italic:false,required_chars:vec![]}).is_err());assert_eq!(before,resolver.stats());
 println!("{}",serde_json::to_string_pretty(&output).unwrap());
 if failed { std::process::exit(1); }
}
#[cfg(not(target_os="macos"))]
fn main(){eprintln!("CoreText provider acceptance requires actual macOS");std::process::exit(2)}
