fn main(){
 let args:Vec<_>=std::env::args().collect();let data:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let rows=data["cases"].as_array().unwrap().iter().map(|row|{
  let matrix: [f32;16]=row["mvp"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap()as f32).collect::<Vec<_>>().try_into().unwrap();
  let segments=row["outline_segments"].as_array().unwrap().iter().map(|seg|{
   let point=|name:&str|-> [f32;3]{seg[name].as_array().unwrap().iter().map(|v|v.as_f64().unwrap()as f32).collect::<Vec<_>>().try_into().unwrap()};
   let native=ingetrazo_render::image_outline::projected_segment(&matrix,point("a"),point("b"),[800,600]);
   serde_json::json!({"plane":seg["plane"],"projected":native})
  }).collect::<Vec<_>>();
  serde_json::json!({"case":row["case"],"segments":segments})
 }).collect::<Vec<_>>();
 std::fs::write(&args[2],serde_json::to_vec_pretty(&serde_json::json!({"cases":rows})).unwrap()).unwrap();
}
