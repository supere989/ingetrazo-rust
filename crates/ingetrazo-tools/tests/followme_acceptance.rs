use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::followme::{sweep_profile,order_path_edges};
fn profile()->Scene{let mut scene=Scene::new();let ids=[[0.,0.,0.],[1.,0.,0.],[1.,0.,1.],[0.,0.,1.]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));let face=scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();scene.mesh.faces[face].attrs.insert("audit_material".into(),serde_json::json!({"name":"profile paint"}));scene}
#[test]
fn malformed_paths_refuse_without_mutation(){for path in [vec![DVec3::ZERO],vec![DVec3::ZERO,DVec3::ZERO],vec![DVec3::ZERO,DVec3::Y,DVec3::ZERO],vec![DVec3::ZERO,DVec3::new(f64::NAN,1.,0.)]]{let mut scene=profile();let before=serde_json::to_value(&scene).unwrap();assert!(sweep_profile(&mut scene,0,&path,false).is_err());assert_eq!(serde_json::to_value(&scene).unwrap(),before);}}
#[test]
fn bent_path_has_corner_and_preserves_profile_material(){let mut scene=profile();sweep_profile(&mut scene,0,&[DVec3::ZERO,DVec3::new(0.,4.,0.),DVec3::new(4.,4.,0.)],false).unwrap();assert!(scene.mesh.vertices.iter().any(|v|(v.position.x-4.).abs()<1e-8));assert!(scene.mesh.vertices.iter().any(|v|(v.position.y-4.+v.position.x).abs()<1e-8));assert!(scene.mesh.faces.iter().all(|f|f.attrs.get("audit_material")==Some(&serde_json::json!({"name":"profile paint"}))));}
#[test]
fn branched_edge_network_is_rejected(){let mut scene=Scene::new();let vertices=[DVec3::ZERO,DVec3::X,-DVec3::X,DVec3::Y].map(|p|scene.mesh.add_vertex(p));let ids=(1..4).map(|i|scene.mesh.add_edge(vertices[0],vertices[i]).unwrap()).collect::<Vec<_>>();assert!(order_path_edges(&scene.mesh,&ids).is_err());}
