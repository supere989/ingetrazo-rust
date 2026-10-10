use glam::DVec3;
use ingetrazo_core::{mesh::Mesh,scene::Scene};
use ingetrazo_tools::followme::sweep_profile;
fn save(mesh:&Mesh,path:&std::path::Path){let faces:Vec<_>=mesh.faces.iter().map(|f|serde_json::json!({"vertices":f.vertices.iter().map(|&v|mesh.vertices[v].position.to_array()).collect::<Vec<_>>(),"holes":f.holes.iter().map(|h|h.iter().map(|&v|mesh.vertices[v].position.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>()})).collect();std::fs::write(path,serde_json::to_vec(&serde_json::json!({"igz_format":1,"app_version":"independent-fixture","scene":{"edges":mesh.edges.iter().map(|e|serde_json::json!({"a":mesh.vertices[e.v0].position.to_array(),"b":mesh.vertices[e.v1].position.to_array(),"soft":e.soft})).collect::<Vec<_>>(),"faces":faces}})).unwrap()).unwrap();}
fn main(){let dir=std::path::PathBuf::from(std::env::args().nth(1).unwrap());for name in ["straight","bend","tube","closed-torus"]{
 let mut scene=Scene::new();let (size,offset)=if name=="closed-torus" {(1.,4.)}else if name=="tube"{(2.,0.)}else{(1.,0.)};
 let outer=[[offset,0.,0.],[offset+size,0.,0.],[offset+size,0.,size],[offset,0.,size]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));
 let holes=if name=="tube"{let inner=[[0.5,0.,0.5],[1.5,0.,0.5],[1.5,0.,1.5],[0.5,0.,1.5]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));vec![inner.to_vec()]}else{vec![]};scene.mesh.add_face(outer.to_vec(),holes).unwrap();
 let path=if name=="closed-torus"{(0..12).map(|i|{let angle=std::f64::consts::TAU*i as f64/12.;DVec3::new(4.5*angle.cos(),4.5*angle.sin(),0.5)}).collect::<Vec<_>>()}else if name=="bend"{vec![DVec3::ZERO,DVec3::new(0.,4.,0.),DVec3::new(4.,4.,0.)]}else{vec![DVec3::ZERO,DVec3::new(0.,if name=="tube"{5.}else{3.},0.)]};
 save(&scene.mesh,&dir.join(format!("{name}-source.igz")));std::fs::write(dir.join(format!("{name}-path.json")),serde_json::to_vec(&path.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap()).unwrap();
 sweep_profile(&mut scene,0,&path,name=="closed-torus").unwrap();save(&scene.mesh,&dir.join(format!("{name}-result.igz")));
 }
 for (label,phi) in [("0",0_f64),("7_5",7.5),("33",33.),("wire0",0.),("wire7_5",7.5)] {for (rlabel,r,plabel,pr) in [("1",1.,"1",1.),("0_5",0.5,"1",1.),("1",1.,"0_5",0.5)] {
 if label.starts_with("wire") && (r!=1. || pr!=1.){continue;}let name=format!("sphere-{label}-{rlabel}-{plabel}");let mut scene=Scene::new();let phi=phi.to_radians();let u=DVec3::new(phi.cos(),phi.sin(),0.);
 let ring=(0..24).map(|i|{let a=std::f64::consts::TAU*i as f64/24.;scene.mesh.add_vertex(u*(r*a.cos())+DVec3::Z*(r*a.sin()))}).collect();scene.mesh.add_face(ring,vec![]).unwrap();
 let path=(0..24).map(|i|{let a=std::f64::consts::TAU*i as f64/24.;DVec3::new(pr*a.cos(),pr*a.sin(),0.)}).collect::<Vec<_>>();
 if label.starts_with("wire"){let ids=path.iter().map(|&p|scene.mesh.add_vertex(p)).collect::<Vec<_>>();for i in 0..ids.len(){scene.mesh.add_edge(ids[i],ids[(i+1)%ids.len()]);}}
 save(&scene.mesh,&dir.join(format!("{name}-source.igz")));std::fs::write(dir.join(format!("{name}-path.json")),serde_json::to_vec(&path.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap()).unwrap();sweep_profile(&mut scene,0,&path,true).unwrap();save(&scene.mesh,&dir.join(format!("{name}-result.igz")));
 }}
 for name in ["attached-straight","attached-bend"] {
 let mut scene=Scene::new();let ids=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
 ingetrazo_tools::pushpull::PushPullTool::extrude_face(&mut scene,0,1.).unwrap();
 let face=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|(scene.mesh.vertices[v].position.z-1.).abs()<1e-8)).unwrap();
 let path=if name=="attached-straight"{vec![DVec3::new(0.5,0.5,1.),DVec3::new(0.5,0.5,2.)]}else{vec![DVec3::new(0.5,0.5,1.),DVec3::new(0.5,0.5,2.),DVec3::new(1.5,0.5,2.)]};
 save(&scene.mesh,&dir.join(format!("{name}-source.igz")));std::fs::write(dir.join(format!("{name}-path.json")),serde_json::to_vec(&path.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap()).unwrap();sweep_profile(&mut scene,face,&path,false).unwrap();save(&scene.mesh,&dir.join(format!("{name}-result.igz")));
 }

 for name in ["open-perpendicular","open-perpendicular-bend","open-coplanar"] {
 let mut scene=Scene::new();let ring=[DVec3::ZERO,DVec3::X,DVec3::new(1.,1.,0.),DVec3::Y].map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ring.to_vec(),vec![]).unwrap();
 let other=if name=="open-coplanar"{[DVec3::X,DVec3::X*2.,DVec3::new(2.,1.,0.),DVec3::new(1.,1.,0.)]}else{[DVec3::ZERO,DVec3::Y,DVec3::new(0.,1.,-1.),-DVec3::Z]};let ids=other.map(|p|scene.mesh.add_vertex(p));scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
 let path=if name.ends_with("bend"){vec![DVec3::new(0.5,0.5,0.),DVec3::new(0.5,0.5,1.),DVec3::new(1.5,0.5,1.)]}else{vec![DVec3::new(0.5,0.5,0.),DVec3::new(0.5,0.5,1.)]};
 save(&scene.mesh,&dir.join(format!("{name}-source.igz")));std::fs::write(dir.join(format!("{name}-path.json")),serde_json::to_vec(&path.iter().map(|p|p.to_array()).collect::<Vec<_>>()).unwrap()).unwrap();sweep_profile(&mut scene,0,&path,false).unwrap();save(&scene.mesh,&dir.join(format!("{name}-result.igz")));
 }

}
