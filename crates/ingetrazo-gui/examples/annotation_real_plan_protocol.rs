//! CPU-only inspection of the exact production annotation planner.
#[cfg(not(target_os="macos"))]
fn main(){
 use ingetrazo_render::annotations::{Primitive,Gate};use std::io::Read;
 let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),3);
 let mut app=ingetrazo_gui::IngeTrazoApp::new();app.scene=ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1])).unwrap();
 let mut bytes=Vec::new();std::fs::File::open(&args[2]).unwrap().take(16385).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=16384);app.camera=serde_json::from_slice(&bytes).unwrap();
 let plan=ingetrazo_gui::native_vulkan::annotation_viewport::plan_for_probe(&app,egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(800.,600.))).unwrap();
 let operations:Vec<_>=plan.operations.iter().map(|o|{let mut v=match &o.primitive{
 Primitive::Line{a,b,width,color}=>serde_json::json!({"kind":"line","a":a,"b":b,"width":width,"pen":color}),
 Primitive::Polygon{points,color}=>serde_json::json!({"kind":"polygon","points":points,"brush":color}),
 Primitive::Ellipse{center,radius,width,color}=>serde_json::json!({"kind":"ellipse","center":center,"radius":radius,"width":width,"brush":color}),
 Primitive::Text{position,text,size_pt,color}=>serde_json::json!({"kind":"text","position":position,"text":text.as_ref(),"size_pt":size_pt,"pen":color})};
 v["gate"]=match o.gate{Gate::Always=>serde_json::json!([]),Gate::Point(a)=>serde_json::json!([a]),Gate::Both(a,b)=>serde_json::json!([a,b])};v}).collect();
 println!("{}",serde_json::json!({"scope":"CPU-only exact production planner; no GPU visibility acceptance","operations":operations,"queries":plan.queries}));
}
#[cfg(target_os="macos")]fn main(){eprintln!("Vulkan diagnostic is Linux/Windows only");std::process::exit(2);}
