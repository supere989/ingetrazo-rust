// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(target_os="macos")]
fn main(){
 use ingetrazo_formats::paper_font_resolver::{Resolver,Request};
 let mut resolver=Resolver::new().expect("native CoreText provider");
 let mut output=Vec::new();
 for (family,bold,italic,text) in [("Helvetica",false,false,"ABC"),("Helvetica",true,false,"ABC"),("Helvetica",false,true,"ABC"),("Times",true,true,"ABC"),("Helvetica",false,false,"漢字"),("Helvetica",false,false,"العربية")]{
  let request=Request{family:family.into(),bold,italic,required_chars:text.chars().collect()};
  let matched=resolver.resolve(request.clone()).unwrap_or_else(|e| panic!("native face/style/coverage {request:?}: {e}"));
  let before=resolver.stats();let again=resolver.resolve(request).unwrap();
  assert!(std::rc::Rc::ptr_eq(&matched.font,&again.font));assert_eq!(before,resolver.stats());
  let bytes=std::fs::read(&matched.path).unwrap(); let face=ttf_parser::Face::parse(&bytes,matched.index).unwrap();
  let covered=text.chars().all(|ch|face.glyph_index(ch).is_some()); assert!(covered);
  let postscript=face.names().into_iter().find_map(ingetrazo_formats::paper_font_resolver::sfnt_postscript_name);
  output.push(serde_json::json!({"family":family,"bold":bold,"italic":italic,"text":text,"path":matched.path,"index":matched.index,"warm_same_owner":true,"chosen_bold":face.is_bold(),"chosen_italic":face.is_italic(),"coverage":covered,"postscript":postscript}));
 }
 let before=resolver.stats();assert!(resolver.resolve(Request{family:"bad\0family".into(),bold:false,italic:false,required_chars:vec![]}).is_err());assert_eq!(before,resolver.stats());
 println!("{}",serde_json::to_string_pretty(&output).unwrap());
}
#[cfg(not(target_os="macos"))]
fn main(){eprintln!("CoreText provider acceptance requires actual macOS");std::process::exit(2)}
