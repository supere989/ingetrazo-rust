//! Actual Rust document loader, BIM takeoff and export consumer.
use std::{path::PathBuf,fs};
fn main(){
 let args:Vec<_>=std::env::args().collect();let input=PathBuf::from(&args[1]);let out=PathBuf::from(&args[2]);fs::create_dir_all(&out).unwrap();
 let original:serde_json::Value=serde_json::from_slice(&fs::read(input.join("original.json")).unwrap()).unwrap();let mut records=vec![];
 for case in original["cases"].as_array().unwrap(){
  let name=case["name"].as_str().unwrap();let scene=ingetrazo_formats::load_igz(&input.join(format!("{name}.igz"))).unwrap();
  let rows=ingetrazo_core::bim::takeoff(&scene).unwrap();let csv=ingetrazo_core::bim::quantities_csv(&rows);fs::write(out.join(format!("{name}.csv")),csv).unwrap();
  let path=out.join(format!("{name}.ifc"));let result=ingetrazo_formats::ifc::save_ifc(&scene,&path,"José's Project");
  records.push(serde_json::json!({"name":name,"objects":rows,"export_count":result.as_ref().ok(),"export_error":result.err().map(|e|e.to_string())}));
 }
 fs::write(out.join("native.json"),serde_json::to_vec_pretty(&records).unwrap()).unwrap();
}
