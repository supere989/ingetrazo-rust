//! Bounded CPU engineering protocol, portable across backend platforms.
fn main(){
 use std::io::Read;
 let mut bytes=Vec::new();std::fs::File::open(std::env::args().nth(1).expect("input JSON")).unwrap().take(2*1024*1024+1).read_to_end(&mut bytes).unwrap();assert!(bytes.len()<=2*1024*1024);
 let value:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
 let rows:Vec<_>=value["cases"].as_array().unwrap().iter().map(|row|{
  let camera:ingetrazo_core::camera::OrbitCamera=serde_json::from_value(row["camera"].clone()).unwrap();
  let snap=ingetrazo_gui::navigation_ray::camera_snapshot(&camera).unwrap();
  let old=(camera.projection_matrix()*camera.view_matrix()).as_mat4();let mut old_gl=old.to_cols_array();for c in 0..4{old_gl[c*4+2]=2.*old_gl[c*4+2]-old_gl[c*4+3];}
  serde_json::json!({"name":row["name"],"projection":snap.projection.concat(),"view":snap.view.concat(),"matrix":snap.matrix.concat(),"inverse":snap.inverse().unwrap().concat(),"eye":snap.eye,"forward":snap.forward,"old_matrix":old_gl,"old_inverse":glam::Mat4::from_cols_array(&old_gl).inverse().to_cols_array()})
 }).collect();println!("{}",serde_json::json!({"cases":rows}));
}
