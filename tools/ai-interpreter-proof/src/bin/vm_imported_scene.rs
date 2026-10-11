#[path = "../face_owner.rs"]
mod face_owner;
#[path = "../journal.rs"]
mod journal;
#[path = "../owner.rs"]
mod owner;
#[path = "../owner_budget.rs"]
mod owner_budget;
#[path = "../owner_registry.rs"]
mod owner_registry;
#[path = "../session.rs"]
mod session;
#[path = "../source_holes.rs"]
mod source_holes;
#[path = "../vm_groups.rs"]
mod vm_groups;
#[path = "../vm_mesh.rs"]
mod vm_mesh;

fn main(){let mut scene=ingetrazo_core::scene::Scene::new();let ids=[glam::DVec3::ZERO,glam::DVec3::X,glam::DVec3::Y].into_iter().map(|p|scene.mesh.add_vertex(p)).collect();let id=scene.mesh.add_face(ids,vec![]).unwrap();scene.mesh.faces[id].attrs.insert("color".into(),serde_json::json!([0.2,0.4,0.8]));scene.mesh.faces[id].attrs.insert("layer".into(),serde_json::json!("Imported"));scene.mesh.faces[id].layer=Some("Imported".into());let uids=scene.mesh.vertices.iter().map(|v|v.uid).collect::<Vec<_>>();let mut c=vm_mesh::Controller::new();c.import_native_mesh(&scene.mesh).unwrap();let b=c.budget();let charge=b.snapshot().total();let prior=c.native_mesh_snapshot().unwrap();b.set_limit(Some(charge));assert!(c.import_native_mesh(&scene.mesh).is_err());b.set_limit(None);assert_eq!(c.native_mesh_snapshot().unwrap(),prior);assert_eq!(scene.mesh,prior);assert_eq!(b.snapshot().total(),charge);c.enable_history();let mut states=vec![c.observe_native_face_fields("imported")];assert!(c.execute("f=m.faces[0]\nf.attrs['mat']='Edited'\nf.loop[0].position.setZ(2)").unwrap());states.push(c.observe_native_face_fields("edited"));scene.mesh=c.native_mesh_snapshot().unwrap();assert_eq!(scene.mesh.vertices[0].position.z,2.);assert_eq!(scene.mesh.vertices.iter().map(|v|v.uid).collect::<Vec<_>>(),uids);assert!(c.undo().unwrap());states.push(c.observe_native_face_fields("undone"));assert_eq!(c.native_mesh_snapshot().unwrap().vertices[0].position.z,0.);assert!(c.redo().unwrap());states.push(c.observe_native_face_fields("redone"));let b=c.budget();drop(c);assert_eq!(b.snapshot().total(),0);println!("{}",serde_json::json!({"states":states,"actual_native_scene_import":true,"packed_uids_preserved":true,"native_document_edit_return":true,"final_charge":0,"production_enabled":false}));}
