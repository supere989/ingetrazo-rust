// Independent Qt comparison adapter: production dash spans, no raster math.
fn main(){
 let args:Vec<_>=std::env::args().collect();
 let source:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let mut out=Vec::new();
 for row in source["cases"].as_array().unwrap(){
  let point=|name:&str|{let a=row[name].as_array().unwrap();[a[0].as_f64().unwrap(),a[1].as_f64().unwrap()]};
  let spans=ingetrazo_render::qt_cosmetic::rasterize_dashed_line(point("a"),point("b"),100_000).unwrap();
  out.push(serde_json::json!({"name":row["name"],"spans":spans.iter().map(|s|serde_json::json!([s.x,s.y,s.len,s.coverage])).collect::<Vec<_>>()}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&serde_json::json!({"cases":out})).unwrap()).unwrap();
}
