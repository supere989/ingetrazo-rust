use glam::DVec3;
use ingetrazo_core::{camera::OrbitCamera,walkthrough::{first_person_step}};
fn main(){
 let args:Vec<String>=std::env::args().collect();
 let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let mut rows=vec![];
 for row in input["cases"].as_array().unwrap(){
  let mut c=OrbitCamera::new();c.look_from(DVec3::Z*1.68,DVec3::Y);
  let floor=row["floor"].as_f64();let wall=row["wall"].as_f64();
  let axes=std::array::from_fn(|i|row["axes"][i].as_i64().unwrap()as i8);
  first_person_step(&mut c,axes,row["dt"].as_f64().unwrap(),1.68,row["run"].as_bool().unwrap(),row["alt"].as_bool().unwrap(),|o,d|{
   let mut best=None::<f64>;
   if d.z < -1e-9 {if let Some(z)=floor{let t=(z-o.z)/d.z;if t>=0.{best=Some(t);}}}
   if d.y.abs()>1e-9 {if let Some(y)=wall{let t=(y-o.y)/d.y;if t>=0.{best=Some(best.map_or(t,|b|b.min(t)));}}}
   best
  });
  rows.push(serde_json::json!({"target":c.target.to_array(),"eye":c.eye().to_array(),"forward":c.forward().to_array(),"yaw":c.yaw,"pitch":c.pitch,"distance":c.distance}));
 }
 println!("{}",serde_json::json!({"cases":rows}));
}
