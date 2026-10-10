//! Source map-block defaults/conversions. No fetch or renderer admission.
use crate::georef::TileCapture;
use serde_json::Value;
fn truth(v:&Value)->bool{match v{Value::Null=>false,Value::Bool(b)=>*b,Value::Number(n)=>n.as_f64()!=Some(0.),Value::String(s)=>!s.is_empty(),Value::Array(a)=>!a.is_empty(),Value::Object(o)=>!o.is_empty()}}

// Decimal ranges captured from the pinned original Python Unicode database 15.1.0.
fn numeric_text(s:&str)->Option<String>{
 let bases:&[u32]=&[48, 1632, 1776, 1984, 2406, 2534, 2662, 2790, 2918, 3046, 3174, 3302, 3430, 3558, 3664, 3792, 3872, 4160, 4240, 6112, 6160, 6470, 6608, 6784, 6800, 6992, 7088, 7232, 7248, 42528, 43216, 43264, 43472, 43504, 43600, 44016, 65296, 66720, 68912, 69734, 69872, 69942, 70096, 70384, 70736, 70864, 71248, 71360, 71472, 71904, 72016, 72784, 73040, 73120, 73552, 92768, 92864, 93008, 120782, 120792, 120802, 120812, 120822, 123200, 123632, 124144, 125264, 130032];
 let mut text=String::new();
 for c in s.trim().chars(){let n=c as u32; if let Some(base)=bases.iter().find(|base|n>=**base&&n<**base+10){text.push(char::from(b'0'+(n-*base)as u8));}else{text.push(c);}}
 let b=text.as_bytes(); if b.iter().enumerate().any(|(i,c)|*c==b'_'&&(i==0||i+1==b.len()||!b[i-1].is_ascii_digit()||!b[i+1].is_ascii_digit())){return None;}
 Some(text.replace("_",""))
}
fn float(v:&Value)->Option<f64>{match v{Value::Bool(b)=>Some(if *b{1.}else{0.}),Value::Number(n)=>n.as_f64(),Value::String(s)=>numeric_text(s)?.parse().ok(),_=>None}.filter(|v|v.is_finite())}
fn integer(v:&Value)->Option<i64>{match v{Value::Bool(b)=>Some(i64::from(*b)),Value::Number(n)=>n.as_i64().or_else(||n.as_f64().filter(|v|v.is_finite()&&*v>=i64::MIN as f64&&*v<i64::MAX as f64).map(|v|v.trunc()as i64)),Value::String(s)=>numeric_text(s)?.parse().ok(),_=>None}}

fn elements(v:&Value)->Option<Vec<Value>>{match v { Value::Array(a)=>Some(a.clone()), Value::String(s)=>Some(s.chars().map(|c|Value::String(c.to_string())).collect()), Value::Object(o)=>Some(o.keys().map(|k|Value::String(k.clone())).collect()), _=>None }}
impl TileCapture{
 pub fn from_source_json(raw:&Value)->Option<Self>{
  if !truth(raw){return None;}
  let o=raw.as_object()?;
  // Provider recipe fields are source-owned JSON metadata, not GPU strings.
  let source=o.get("source").cloned().unwrap_or(Value::String("custom".into()));
  if source.is_array()||source.is_object(){return None;} // Python preset lookup requires hashable id.
  let (name,url_template,attribution)=match source.as_str().unwrap_or(""){
   "osm"=>(Value::from("OpenStreetMap (streets)"),Value::from("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),Value::from("© OpenStreetMap contributors")),
   "esri_imagery"=>(Value::from("Esri World Imagery (satellite)"),Value::from("https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}"),Value::from("Esri, Maxar, Earthstar Geographics, and the GIS community")),
   "s2cloudless"=>(Value::from("Sentinel-2 cloudless (EOX)"),Value::from("https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2020_3857/default/GoogleMapsCompatible/{z}/{y}/{x}.jpg"),Value::from("Sentinel-2 cloudless — https://s2maps.eu by EOX IT Services GmbH (Contains modified Copernicus Sentinel data)")),
   _=>(o.get("name").cloned().unwrap_or_else(||source.clone()),o.get("url_template").cloned().unwrap_or(Value::String(String::new())),o.get("attribution").cloned().unwrap_or(Value::String(String::new())))
  };
  let zoom=integer(o.get("zoom").unwrap_or(&Value::from(16)))?;
  let radius_m=float(o.get("radius_m").unwrap_or(&Value::from(1200.)))?;
  let mut patches=vec![vec![0.,0.,radius_m,radius_m]];
  if let Some(p)=o.get("patches").filter(|v|truth(v)){patches=elements(p)?.iter().map(|p|elements(p)?.iter().map(float).collect::<Option<Vec<_>>>()).collect::<Option<Vec<_>>>()?;}
  Some(Self{source,zoom,radius_m,patches,name,url_template,attribution,visible:o.get("visible").is_none_or(truth),opacity:float(o.get("opacity").unwrap_or(&Value::from(1.)))?.clamp(0.,1.)})
 }
}
