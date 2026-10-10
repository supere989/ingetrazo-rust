use glam::DVec3;
use ingetrazo_core::camera::OrbitCamera;
fn vec(v:&serde_json::Value)->DVec3{DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
fn main(){
 let args:Vec<String>=std::env::args().collect();
 let input:serde_json::Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
 let mut rows=vec![];
 for row in input["cases"].as_array().unwrap(){
  let mut c=OrbitCamera::new();let a=&row["args"];
  let accepted=match row["op"].as_str().unwrap(){
   "look_from"=>c.look_from(vec(&a[0]),vec(&a[1])),
   "turn"=>c.turn(a[0].as_f64().unwrap(),a[1].as_f64().unwrap()),
   "move_eye"=>c.move_eye(vec(&a[0])),_=>panic!("unknown operation")};
  assert!(accepted);
  rows.push(serde_json::json!({"target":c.target.to_array(),"eye":c.eye().to_array(),"forward":c.forward().to_array(),"yaw":c.yaw,"pitch":c.pitch,"distance":c.distance}));
 }
 println!("{}",serde_json::json!({"cases":rows}));
}
