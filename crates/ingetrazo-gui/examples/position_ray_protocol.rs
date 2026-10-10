use ingetrazo_gui::navigation_ray;
fn main() {
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let rows:Vec<_>=source["cases"].as_array().unwrap().iter().map(|row|{let camera:ingetrazo_core::camera::OrbitCamera=serde_json::from_value(row["camera"].clone()).unwrap();let cursor=row["cursor"].as_array().unwrap();let ray=navigation_ray::cursor_ray(&camera,cursor[0].as_f64().unwrap(),cursor[1].as_f64().unwrap(),800.,600.).unwrap();serde_json::json!({"name":row["name"],"origin":ray.origin.to_array(),"direction":ray.dir.to_array(),"ground":navigation_ray::plane_hit(&ray,glam::DVec3::ZERO,glam::DVec3::Z).map(|p|p.to_array())})}).collect();
    println!("{}", serde_json::json!({"cases":rows}));
}
