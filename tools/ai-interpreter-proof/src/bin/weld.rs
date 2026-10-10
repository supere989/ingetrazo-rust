#[path="../journal.rs"] mod journal;
use glam::DVec3;use ingetrazo_core::mesh::Mesh;
fn main(){let cases:serde_json::Value=serde_json::from_str(include_str!("../../weld-cases.json")).unwrap();let mut results=vec![];
for c in cases.as_array().unwrap(){let mut m=Mesh::new();let mut j=journal::MeshJournal::begin(&m,1);for p in c["points"].as_array().unwrap(){let a=p.as_array().unwrap();j.add_edge(DVec3::new(a[0].as_f64().unwrap(),a[1].as_f64().unwrap(),a[2].as_f64().unwrap()),DVec3::new(0.,10.,0.)).unwrap();}j.publish(&mut m,1).unwrap();results.push(serde_json::json!({"name":c["name"],"vertices":m.vertices.iter().map(|v|v.position.to_array()).collect::<Vec<_>>(),"edges":m.edges.iter().map(|e|[e.v0,e.v1]).collect::<Vec<_>>()}));}println!("{}",serde_json::to_string_pretty(&results).unwrap());}
