//! Source map-block defaults/conversions. No fetch or renderer admission.
use crate::georef::TileCapture;
use serde_json::Value;
fn truth(v:&Value)->bool{match v{Value::Null=>false,Value::Bool(b)=>*b,Value::Number(n)=>n.as_f64()!=Some(0.),Value::String(s)=>!s.is_empty(),Value::Array(a)=>!a.is_empty(),Value::Object(o)=>!o.is_empty()}}
fn float(v:&Value)->Option<f64>{match v{Value::Bool(b)=>Some(if *b{1.}else{0.}),Value::Number(n)=>n.as_f64(),Value::String(s)=>s.trim().parse().ok(),_=>None}.filter(|v|v.is_finite())}
fn integer(v:&Value)->Option<i64>{match v{Value::Bool(b)=>Some(i64::from(*b)),Value::Number(n)=>n.as_i64().or_else(||n.as_f64().filter(|v|v.is_finite()&&*v>=i64::MIN as f64&&*v<i64::MAX as f64).map(|v|v.trunc()as i64)),Value::String(s)=>s.trim().parse().ok(),_=>None}}

fn elements(v:&Value)->Option<Vec<Value>>{match v { Value::Array(a)=>Some(a.clone()), Value::String(s)=>Some(s.chars().map(|c|Value::String(c.to_string())).collect()), Value::Object(o)=>Some(o.keys().map(|k|Value::String(k.clone())).collect()), _=>None }}
impl TileCapture{
 pub fn from_source_json(raw:&Value)->Option<Self>{
  if !truth(raw){return None;}
  let o=raw.as_object()?;
  let text=|key:&str,default:&str|->Option<String>{Some(o.get(key).map_or(Some(default),Value::as_str)?.to_owned())};
  let source=text("source","custom")?;
  let (name,url_template,attribution)=match source.as_str(){
   "osm"=>("OpenStreetMap (streets)".into(),"https://tile.openstreetmap.org/{z}/{x}/{y}.png".into(),"© OpenStreetMap contributors".into()),
   "esri_imagery"=>("Esri World Imagery (satellite)".into(),"https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}".into(),"Esri, Maxar, Earthstar Geographics, and the GIS community".into()),
   "s2cloudless"=>("Sentinel-2 cloudless (EOX)".into(),"https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2020_3857/default/GoogleMapsCompatible/{z}/{y}/{x}.jpg".into(),"Sentinel-2 cloudless — https://s2maps.eu by EOX IT Services GmbH (Contains modified Copernicus Sentinel data)".into()),
   _=>(text("name",&source)?,text("url_template","")?,text("attribution","")?)
  };
  let zoom=integer(o.get("zoom").unwrap_or(&Value::from(16)))?;
  let radius_m=float(o.get("radius_m").unwrap_or(&Value::from(1200.)))?;
  let mut patches=vec![vec![0.,0.,radius_m,radius_m]];
  if let Some(p)=o.get("patches").filter(|v|truth(v)){patches=elements(p)?.iter().map(|p|elements(p)?.iter().map(float).collect::<Option<Vec<_>>>()).collect::<Option<Vec<_>>>()?;}
  Some(Self{source,zoom,radius_m,patches,name,url_template,attribution,visible:o.get("visible").is_none_or(truth),opacity:float(o.get("opacity").unwrap_or(&Value::from(1.)))?.clamp(0.,1.)})
 }
}
