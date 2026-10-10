//! Actual IGZ loader observations for map-block parity audit.
fn main(){
 let root=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
 let source:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("original.json")).unwrap()).unwrap();
 let rows=source["cases"].as_array().unwrap().iter().enumerate().map(|(i,row)|{
  match ingetrazo_formats::igz::load_igz(&root.join(format!("case-{i}.igz"))){
   Ok(scene)=>{if let Some(out)=std::env::args().nth(2){let out=std::path::PathBuf::from(out);std::fs::create_dir_all(&out).unwrap();ingetrazo_formats::igz::save_igz(&scene,&out.join(format!("case-{i}.igz"))).unwrap();}serde_json::json!({"input":row["input"],"document_opened":true,"tile_capture":scene.tile_layer})},
   Err(error)=>serde_json::json!({"input":row["input"],"document_opened":false,"error":error.to_string()})
  }
 }).collect::<Vec<_>>();
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"cases":rows})).unwrap());
}
