// Independent Qt comparison adapter: production dash spans, no raster math.
fn main(){
 let args:Vec<_>=std::env::args().collect();
 let source:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let mut out=Vec::new();
 for row in source["cases"].as_array().unwrap(){
  let point=|name:&str|{let a=row[name].as_array().unwrap();[a[0].as_f64().unwrap(),a[1].as_f64().unwrap()]};
  let spans=if args.iter().any(|a|a=="--selected"){rasterize_selected_image_line(point("a"),point("b"),[256,256],100_000)}else{ingetrazo_render::qt_cosmetic::rasterize_dashed_line_clipped(point("a"),point("b"),[256,256],100_000)}.unwrap();
  out.push(serde_json::json!({"name":row["name"],"spans":spans.iter().map(|s|serde_json::json!([s.x,s.y,s.len,s.coverage])).collect::<Vec<_>>()}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&serde_json::json!({"cases":out})).unwrap()).unwrap();
}

// REJECTED rectangular stroke hypothesis: kept only in engineering probe.
/// Source selected-image pen is width two with the QPen default SquareCap.
/// Rasterizes its one rectangular region, avoiding overlapping cap draws.
fn rasterize_selected_image_line(a:[f64;2],b:[f64;2],size:[u32;2],limit:usize)->Result<Vec<ingetrazo_render::qt_gray::Span>,String>{
 if a.iter().chain(b.iter()).any(|v|!v.is_finite()){return Ok(Vec::new());}
 if size.contains(&0)||size.iter().any(|v|*v>1_000_000)||limit==0{return Err("Invalid selected image stroke admission".into());}
 let dx=b[0]-a[0];let dy=b[1]-a[1];let length=dx.hypot(dy);
 let (u,v)=if length==0.{([1.,0.],[0.,1.])}else{([dx/length,dy/length],[-dy/length,dx/length])};
 let points=vec![[a[0]-u[0]+v[0],a[1]-u[1]+v[1]],[b[0]+u[0]+v[0],b[1]+u[1]+v[1]],[b[0]+u[0]-v[0],b[1]+u[1]-v[1]],[a[0]-u[0]-v[0],a[1]-u[1]-v[1]]];
 let mut spans=ingetrazo_render::qt_gray::rasterize(&[points],limit.saturating_mul(8),limit)?;
 spans.retain_mut(|span|{
  if span.y<0||i64::from(span.y)>=i64::from(size[1]){return false;}
  let left=i64::from(span.x).max(0);let right=(i64::from(span.x)+i64::from(span.len)).min(i64::from(size[0]));
  if right<=left{return false;}span.x=left as i32;span.len=(right-left)as u32;true
 });
 Ok(spans)
}
