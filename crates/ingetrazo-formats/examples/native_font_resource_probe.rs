//! Executes native font FFI/resource lifecycle; no source rendered parity claim.
use ingetrazo_formats::{paper_font::{FontCache,FontKey},paper_text::{FtPolicy,Hinting}};
use std::sync::Arc;
fn main(){
 let path=std::env::args().nth(1).expect("native font path");
 let bytes:Arc<[u8]>=std::fs::read(&path).unwrap().into();
 let key=FontKey{identity:uuid::Uuid::new_v4(),face_index:0,policy:FtPolicy{hinting:Hinting::Vertical,force_auto_hint:false}};
 let mut cache=FontCache::default();let font=cache.load(key,bytes.clone()).unwrap();
 let glyphs=font.shape_run("Office AVATAR fi ffi café",None).unwrap();assert!(!glyphs.is_empty());assert!(glyphs.iter().all(|g|g.id!=0));
 let ids=glyphs.iter().map(|g|g.id).collect::<Vec<_>>();let resource=font.subset(&ids).unwrap();assert!(resource.program_bytes()>0);
 let stats=font.stats();assert!(Arc::ptr_eq(&resource,&font.subset(&ids).unwrap()));assert_eq!(font.stats(),stats);
 assert!(std::rc::Rc::ptr_eq(&font,&cache.load(key,bytes).unwrap()));cache.clear();
 assert_eq!(font.shape_run("Office AVATAR fi ffi café",None).unwrap(),glyphs);
 assert!(Arc::ptr_eq(&resource,&font.subset(&ids).unwrap()));
 println!("PASS actual native shaping/subset/warm cache/retained resource lifetime; no rendered source parity claim");
}
