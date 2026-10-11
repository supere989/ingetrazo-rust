use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_render::face_sweep::{Input,Packet};
use serde_json::{Value,json};
fn vec(v:&Value)->DVec3{DVec3::new(v[0].as_f64().unwrap(),v[1].as_f64().unwrap(),v[2].as_f64().unwrap())}
fn main(){let args=std::env::args().collect::<Vec<_>>();let source:Value=serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();let mut rows=Vec::new();
 for case in source["cases"].as_array().unwrap(){let mut scene=Scene::new();for f in case["source"].as_array().unwrap(){let mut ring=|v:&Value|v.as_array().unwrap().iter().map(|p|scene.mesh.add_vertex(vec(p))).collect::<Vec<_>>();let vertices=ring(&f["vertices"]);let holes=f["holes"].as_array().unwrap().iter().map(|h|ring(h)).collect();let id=scene.mesh.add_face(vertices,holes).unwrap();scene.mesh.faces[id].attrs=serde_json::from_value(f["attrs"].clone()).unwrap();}
 let face=case["face"].as_u64().unwrap() as usize;let normal=vec(&case["normal"]);let offset=(normal.as_vec3()*(case["distance"].as_f64().unwrap() as f32)).as_dvec3();let anchor=scene.mesh.faces[face].vertices.iter().map(|&id|scene.mesh.vertices[id].position).sum::<DVec3>()/scene.mesh.faces[face].vertices.len() as f64;
 let input=Input{face,face_identity:scene.mesh.faces[face].history_id,mesh_revision:scene.version,anchor,normal,offset,attached:case["name"]!="lone-sheet",prism_cap:false,keep_base:false};
 // Captured recipes have explicit source suppression; this protocol isolates geometry.
 let eye=if case["suppressed"]==true{anchor-offset}else{anchor+offset};let before=serde_json::to_value(&scene.mesh).unwrap();let packet=Packet::build(&scene,input,eye).unwrap();let preview=packet.mesh.faces.iter().map(|f|json!({"vertices":f.vertices.iter().map(|&id|scene_to_array(packet.mesh.vertices[id].position)).collect::<Vec<_>>()})).collect::<Vec<_>>();assert_eq!(json!(preview),case["preview"],"{}",case["name"]);assert_eq!(!packet.suppressed.is_empty(),case["suppressed"].as_bool().unwrap());assert_eq!(before,serde_json::to_value(&scene.mesh).unwrap());rows.push(json!({"name":case["name"],"preview":preview,"inert":true,"suppressed":packet.suppressed,"line_endpoints":packet.lines.len()}));}
 std::fs::write(&args[2],serde_json::to_vec_pretty(&json!({"source_pin":source["source_pin"],"scope":"retained source preview polygons; synthetic eye isolates recorded suppression; no GPU claim","rows":rows})).unwrap()).unwrap();}
fn scene_to_array(p:DVec3)->[f64;3]{p.to_array()}
