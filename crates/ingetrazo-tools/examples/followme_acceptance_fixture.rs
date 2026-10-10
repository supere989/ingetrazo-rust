use glam::DVec3;
use ingetrazo_core::{mesh::Mesh,scene::Scene};
use ingetrazo_tools::followme::sweep_profile;
fn save(mesh:&Mesh,path:&std::path::Path){let faces:Vec<_>=mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices.iter().map(|&v|mesh.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|mesh.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()})).collect();std::fs::write(path,serde_json::to_vec(&serde_json::json!({"igz_format":1,"app_version":"independent-fixture","scene":{"edges":[],"faces":faces}})).unwrap()).unwrap();}
fn main(){let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());for name in ["straight","bend","tube","closed-torus"]{
 let mut scene=Scene::new();let (size,offset)=if name=="closed-torus" {(1.,4.)}else if name=="tube"{(2.,0.)}else{(1.,0.)};
 let outer=[[offset,0.,0.],[offset+size,0.,0.],[offset+size,0.,size],[offset,0.,size]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));
 let holes=if name=="tube"{let inner=[[0.5,0.,0.5],[1.5,0.,0.5],[1.5,0.,1.5],[0.5,0.,1.5]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));vec![inner.to_vec()]}else{vec![]};scene.mesh.add_face(outer.to_vec(),holes).unwrap();
 let path=if name=="closed-torus"{(0..12).map(|i|{let angle=std::f64::consts::TAU*i as f64/12.;DVec3::new(4.5*angle.cos(),4.5*angle.sin(),0.5)}).collect::<Vec<_>>()}else if name=="bend"{vec![DVec3::ZERO,DVec3::new(0.,4.,0.),DVec3::new(4.,4.,0.)]}else{vec![DVec3::ZERO,DVec3::new(0.,if name=="tube"{5.}else{3.},0.)]};
 save(&scene.mesh,&dir.join(format!("{name}-source.igz")));std::fs::write(dir.join(format!("{name}-path.json")),serde_json::to_vec(&path.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap()).unwrap();
 sweep_profile(&mut scene,0,&path,name=="closed-torus").unwrap();save(&scene.mesh,&dir.join(format!("{name}-result.igz")));
}}
