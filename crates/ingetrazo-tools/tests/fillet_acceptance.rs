use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::fillet::fillet_edge;
fn scene()->Scene{let mut s=Scene::new();for p in [[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,0.],[0.,0.,1.],[2.,0.,1.],[2.,1.,1.],[0.,1.,1.]]{s.mesh.add_vertex(DVec3::from_array(p));}for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]{s.mesh.add_face(f.to_vec(),vec![]).unwrap();}s}
#[test]
fn invalid_radii_are_refused_atomically(){for radius in [0.,-0.1,f64::NAN,f64::INFINITY,2.] {let mut s=scene();let edge=s.mesh.edges.iter().position(|e|s.mesh.vertices[e.v0].position.z==1.&&s.mesh.vertices[e.v1].position.z==1.&&s.mesh.vertices[e.v0].position.y==0.&&s.mesh.vertices[e.v1].position.y==0.).unwrap();let before=serde_json::to_value(&s).unwrap();assert!(fillet_edge(&mut s,edge,radius,8).is_err());assert_eq!(serde_json::to_value(&s).unwrap(),before);}}
#[test]
fn radius_and_segments_create_tangent_cylinder_geometry(){for segments in [3,8,16]{let mut s=scene();let edge=s.mesh.edges.iter().position(|e|s.mesh.vertices[e.v0].position.z==1.&&s.mesh.vertices[e.v1].position.z==1.&&s.mesh.vertices[e.v0].position.y==0.&&s.mesh.vertices[e.v1].position.y==0.).unwrap();fillet_edge(&mut s,edge,0.2,segments).unwrap();let curved=s.mesh.faces.iter().filter(|f|f.vertices.iter().all(|&v|{let p=s.mesh.vertices[v].position;((p.y-0.2).hypot(p.z-0.8)-0.2).abs()<1e-8})).count();assert_eq!(curved,segments);assert!(s.mesh.edges.iter().any(|e|e.soft));}}
