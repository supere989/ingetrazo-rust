use glam::DVec3;
use ingetrazo_core::math::Ray;
use ingetrazo_tools::scale_tool::grips::{Frame,Grip,GripBox};
use serde_json::{Value,json};
fn vec(v:&Value)->DVec3{DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
fn main(){
 let args=std::env::args().collect::<Vec<_>>();
 let source:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let point=vec(&source["point"]);let ray=Ray{origin:vec(&source["ray_origin"]),dir:vec(&source["ray_direction"])};
 let mut cases=vec![];
 for case in source["cases"].as_array().unwrap(){
  let frame=if case["axes"].is_null(){None}else{Some(Frame{origin:vec(&case["origin"]),axes:std::array::from_fn(|i|vec(&case["axes"][i]))})};
  let b=GripBox::new(vec(&case["low"]),vec(&case["high"]),frame).unwrap();
  let mut controls=vec![];
  for c in case["controls"].as_array().unwrap(){
   let params: [f64;3]=serde_json::from_value(c["params"].clone()).unwrap();
   let mask=std::array::from_fn(|i|c["mask"].as_array().unwrap().iter().any(|v|v.as_u64()==Some(i as u64)));
   let g=Grip{params,mask};assert!(b.grips.contains(&g));
   let anchor=b.anchor(g,c["center"].as_bool().unwrap());let uniform=c["uniform"].as_bool().unwrap();
   let typed=c["typed"].as_array().unwrap().iter().map(|v|{let values=serde_json::from_value::<Vec<f64>>(v["values"].clone()).unwrap();json!({"values":values,"absolute":v["absolute"],"factors":b.typed_factors(g,uniform,&values,v["absolute"].as_bool().unwrap()).map(|v|v.to_array())})}).collect::<Vec<_>>();
   controls.push(json!({"params":params,"mask":c["mask"],"center":c["center"],"uniform":uniform,"position":b.position(g).to_array(),"anchor":anchor.to_array(),"point_factors":b.factors_from_point(g,anchor,point,uniform).map(|v|v.to_array()),"ray_factors":b.factors_from_ray(g,anchor,&ray,uniform).map(|v|v.to_array()),"typed":typed}));
  }
  cases.push(json!({"name":case["name"],"low":b.low.to_array(),"high":b.high.to_array(),"origin":case["origin"],"axes":case["axes"],"grip_count":b.grips.len(),"segments":b.segments().into_iter().map(|(a,b)|[a.to_array(),b.to_array()]).collect::<Vec<_>>(),"controls":controls}));
 }
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"cases":cases})).unwrap()).unwrap();
}
