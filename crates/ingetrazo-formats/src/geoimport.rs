//! Source-compatible alignment geometry parsing; projection and UI are separate.
use serde::{Serialize,Deserialize};
use serde_json::Value;
#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub struct GeoFeature {pub points:Vec<[f64;2]>,pub closed:bool,pub name:String}
fn add(mut points:Vec<[f64;2]>,closed:bool,name:&str,out:&mut Vec<GeoFeature>){
 if closed && points.len()>1 {let a=points[0];let b=*points.last().unwrap();if (a[0]-b[0]).abs()<1e-9&&(a[1]-b[1]).abs()<1e-9{points.pop();}}
 if points.len()>=if closed{3}else{2}{out.push(GeoFeature{points,closed,name:name.into()});}
}
pub fn parse_kml(text:&str)->Vec<GeoFeature>{
 let Ok(doc)=roxmltree::Document::parse(text) else{return vec![]};
 let mut out=vec![];
 for pm in doc.descendants().filter(|n|n.is_element()&&n.tag_name().name()=="Placemark"){
  let mut name="";
  for ch in pm.children().filter(|n|n.is_element()&&n.tag_name().name()=="name"){if let Some(t)=ch.text(){name=t.trim();}}
  for geom in pm.descendants().filter(|n|n.is_element()){
   let closed=match geom.tag_name().name(){"Polygon"=>true,"LineString"=>false,_=>continue};
   let text=geom.descendants().find(|n|n.is_element()&&n.tag_name().name()=="coordinates"&&n.text().is_some()).and_then(|n|n.text()).unwrap_or("");
   let points=text.split_whitespace().filter_map(|t|{let mut c=t.split(',');let lon=c.next()?.parse::<f64>().ok()?;let lat=c.next()?.parse::<f64>().ok()?;Some([lat,lon])}).collect();
   add(points,closed,name,&mut out);
  }
 }
 out
}
pub fn parse_geojson(text:&str)->Result<Vec<GeoFeature>,String>{
 let Ok(data)=serde_json::from_str::<Value>(text) else{return Ok(vec![])};
 fn coords(v:&Value)->Result<Vec<[f64;2]>,String>{
  let Some(cs)=v.as_array() else{if v.is_null(){return Ok(vec![])}return Err("Alignment coordinates must be arrays".into())};
  cs.iter().filter_map(|c|match c.as_array(){Some(a) if a.len()<2=>None,Some(a)=>Some(match (a[1].as_f64(),a[0].as_f64()){(Some(lat),Some(lon))=>Ok([lat,lon]),_=>Err("Alignment coordinates must be numeric".into())}),None=>Some(Err("Alignment coordinate must be an array".into()))}).collect()
 }
 fn walk(v:&Value,name:&str,out:&mut Vec<GeoFeature>)->Result<(),String>{
  match v.get("type").and_then(Value::as_str).unwrap_or(""){
   "FeatureCollection"=>{if let Some(fs)=v.get("features").and_then(Value::as_array){for f in fs{walk(f,"",out)?}}},
   "Feature"=>{let n=v.pointer("/properties/name").filter(|n|!n.is_null());let name=match n{Some(Value::String(s))=>s.clone(),Some(Value::Bool(true))=>"True".into(),Some(Value::Bool(false))=>String::new(),Some(Value::Number(n)) if n.as_f64()==Some(0.)=>String::new(),Some(Value::Number(n))=>n.to_string(),None=>String::new(),_=>return Err("Structured feature names unsupported".into())};walk(&v["geometry"],&name,out)?},
   "GeometryCollection"=>{if let Some(gs)=v.get("geometries").and_then(Value::as_array){for g in gs{walk(g,name,out)?}}},
   "LineString"=>add(coords(&v["coordinates"])?,false,name,out),
   "Polygon"=>{if let Some(r)=v["coordinates"].as_array().and_then(|a|a.first()){add(coords(r)?,true,name,out)}},
   "MultiLineString"=>{if let Some(lines)=v["coordinates"].as_array(){for l in lines{add(coords(l)?,false,name,out)}}},
   "MultiPolygon"=>{if let Some(polys)=v["coordinates"].as_array(){for p in polys{if let Some(r)=p.as_array().and_then(|a|a.first()){add(coords(r)?,true,name,out)}}}},
   _=>{}
  }
  Ok(())
 }
 let mut out=vec![];walk(&data,"",&mut out)?;Ok(out)
}

pub fn load_features(path:&std::path::Path)->Result<Vec<GeoFeature>,String>{
 use std::io::Read;
 let ext=path.extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
 let text=if ext=="kmz"{
  let file=std::fs::File::open(path).map_err(|e|e.to_string())?;
  let mut zip=zip::ZipArchive::new(file).map_err(|e|e.to_string())?;
  let names:Vec<String>=zip.file_names().filter(|n|n.to_ascii_lowercase().ends_with(".kml")).map(str::to_owned).collect();
  if names.is_empty(){return Ok(vec![])}
  let name=if names.iter().any(|n|n=="doc.kml"){"doc.kml"}else{&names[0]};
  let mut bytes=vec![];zip.by_name(name).map_err(|e|e.to_string())?.read_to_end(&mut bytes).map_err(|e|e.to_string())?;
  return Ok(parse_kml(&String::from_utf8_lossy(&bytes)))
 }else{String::from_utf8_lossy(&std::fs::read(path).map_err(|e|e.to_string())?).into_owned()};
 if ext=="kml" || (ext!="geojson"&&ext!="json"&&text.trim_start().starts_with('<')){Ok(parse_kml(&text))}else{parse_geojson(&text)}
}
