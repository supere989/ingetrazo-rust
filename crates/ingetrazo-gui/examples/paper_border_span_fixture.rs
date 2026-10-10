//! Native source border coverage protocol (CPU geometry; GPU/UI is separate acceptance).
use std::io::Read;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let mut input=String::new();std::io::stdin().read_to_string(&mut input)?;let cases:serde_json::Value=serde_json::from_str(&input)?;let mut out=Vec::new();
 for c in cases.as_array().ok_or("Expected case inventory")?{
  let n=|k:&str|c[k].as_f64().ok_or("Missing numeric input");let page=[c["page"][0].as_f64().ok_or("page")?,c["page"][1].as_f64().ok_or("page")?];let phase=[c["phase"][0].as_f64().ok_or("phase")?,c["phase"][1].as_f64().ok_or("phase")?];
  let border=ingetrazo_core::composition::SourcePaperBorder{on:true,mm:n("width")?,color:"#000000".into(),radius_mm:n("radius")?,style:c["style"].as_str().ok_or("style")?.into()};
  let result=ingetrazo_gui::composer_border::source_spans(page,n("margin")?,&border,n("density")?,phase);
  out.push(match result{Ok(strokes)=>serde_json::json!({"name":c["name"],"strokes":strokes.into_iter().map(|spans|spans.into_iter().map(|s|[i64::from(s.x),i64::from(s.y),i64::from(s.len),i64::from(s.coverage)]).collect::<Vec<_>>()).collect::<Vec<_>>()}),Err(e)=>serde_json::json!({"name":c["name"],"error":e})});
 }
 println!("{}",serde_json::to_string(&out)?);Ok(())
}
