use ingetrazo_core::{scene::Scene,groups::Group,mesh::Mesh};
use glam::{DVec3,DMat4};
use uuid::Uuid;
#[test]
fn prototype_publication_keeps_independent_lists_and_payloads() {
 let mut scene=Scene::new();let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);
 let mut parent=Group::new("container");parent.children.push(Group::with_mesh("ordinary child",mesh));
 let id=parent.id;scene.groups.push(parent);scene.make_component(id).unwrap();
 let copy=scene.groups[0].duplicate_instance();scene.groups.push(copy);
 let token=scene.groups[0].children[0].mesh_share.unwrap();
 let child_copy=scene.groups[0].children[0].duplicate_instance();scene.groups[0].children.push(child_copy);
 scene.groups[0].children[1].transform=DMat4::from_translation(DVec3::X);
 scene.groups[1].children[0].native_document=serde_json::json!({"component":false,"material":{"name":"paint"},"future":71});
 let before=scene.groups.clone();let mut edited=scene.mesh_shares[&token].clone();
 edited.vertices[0].position=DVec3::Z;scene.publish_mesh_share(token,edited).unwrap();
 assert_eq!(scene.groups[0].children.len(),2);assert_eq!(scene.groups[1].children.len(),1);
 for (after,old) in scene.groups.iter().zip(&before) {
  for (a,b) in after.children.iter().zip(&old.children) {
   assert_eq!(a.id,b.id);assert_eq!(a.transform,b.transform);assert_eq!(a.native_document,b.native_document);
   assert_eq!(a.component_definition,None);assert_eq!(a.mesh.vertices[0].position,DVec3::Z);
  }
 }
 let state=scene.clone();let mut invalid=scene.mesh_shares[&token].clone();invalid.vertices[0].position.x=f64::NAN;
 assert!(scene.publish_mesh_share(token,invalid).is_err());assert_eq!(scene,state);
 assert!(scene.publish_mesh_share(Uuid::new_v4(),Mesh::new()).is_err());assert_eq!(scene,state);
 let recovered:Scene=serde_json::from_slice(&serde_json::to_vec(&scene).unwrap()).unwrap();assert_eq!(serde_json::to_value(&recovered.mesh_shares).unwrap(),serde_json::to_value(&scene.mesh_shares).unwrap());
 assert_eq!(serde_json::to_value(&recovered.groups).unwrap(),serde_json::to_value(&scene.groups).unwrap());
}
#[test]
fn equal_authored_geometry_does_not_create_identity_and_unique_detaches() {
 let mut scene=Scene::new();let mut mesh=Mesh::new();mesh.add_vertex(DVec3::X);
 scene.groups.push(Group::with_mesh("a",mesh.clone()));scene.groups.push(Group::with_mesh("b",mesh));
 let a=scene.groups[0].id;let b=scene.groups[1].id;scene.make_component(a).unwrap();scene.make_component(b).unwrap();
 let token=scene.groups[0].mesh_share.unwrap();assert_ne!(Some(token),scene.groups[1].mesh_share);
 scene.groups.push(scene.groups[0].duplicate_instance());let copy=scene.groups[2].id;scene.make_component_unique(copy).unwrap();
 assert_eq!(scene.groups[2].mesh_share,None);
 let mut changed=scene.mesh_shares[&token].clone();changed.vertices[0].position=DVec3::Y;
 scene.publish_mesh_share(token,changed).unwrap();assert_eq!(scene.groups[0].mesh.vertices[0].position,DVec3::Y);
 assert_eq!(scene.groups[1].mesh.vertices[0].position,DVec3::X);assert_eq!(scene.groups[2].mesh.vertices[0].position,DVec3::X);
}

#[test]
fn tokenless_recovery_migration_links_only_definition_members_and_is_idempotent() {
 let mut scene=Scene::new();let mut mesh=Mesh::new();mesh.add_vertex(DVec3::X);
 let mut parent=Group::new("container");parent.children.push(Group::with_mesh("child",mesh.clone()));
 scene.groups.push(parent);scene.make_component(scene.groups[0].id).unwrap();scene.groups.push(scene.groups[0].duplicate_instance());
 scene.groups.push(Group::with_mesh("unrelated equal",mesh));
 fn remove_tokens(value:&mut serde_json::Value) {
  match value {serde_json::Value::Object(map)=>{map.remove("mesh_share");map.remove("mesh_shares");for value in map.values_mut(){remove_tokens(value);}},serde_json::Value::Array(items)=>for value in items {remove_tokens(value);},_=>{}}
 }
 let mut old=serde_json::to_value(&scene).unwrap();remove_tokens(&mut old);
 let mut recovered:Scene=serde_json::from_value(old).unwrap();let placements=recovered.groups.iter().map(|g|(g.id,g.transform,g.children.iter().map(|c|c.id).collect::<Vec<_>>())).collect::<Vec<_>>();
 assert!(recovered.migrate_legacy_mesh_shares().unwrap());let token=recovered.groups[0].children[0].mesh_share.unwrap();
 assert_eq!(recovered.groups[1].children[0].mesh_share,Some(token));assert_eq!(recovered.groups[2].mesh_share,None);
 assert_eq!(recovered.groups.iter().map(|g|(g.id,g.transform,g.children.iter().map(|c|c.id).collect::<Vec<_>>())).collect::<Vec<_>>(),placements);
 let migrated=recovered.clone();assert!(!recovered.migrate_legacy_mesh_shares().unwrap());assert_eq!(recovered,migrated);
 let mut changed=recovered.mesh_shares[&token].clone();changed.vertices[0].position=DVec3::Z;recovered.publish_mesh_share(token,changed).unwrap();
 assert_eq!(recovered.groups[0].children[0].mesh.vertices[0].position,DVec3::Z);assert_eq!(recovered.groups[1].children[0].mesh.vertices[0].position,DVec3::Z);assert_eq!(recovered.groups[2].mesh.vertices[0].position,DVec3::X);
 let mut broken=migrated.clone();broken.groups[0].component_definition=Some(Uuid::new_v4());let before=broken.clone();assert!(broken.migrate_legacy_mesh_shares().is_err());assert_eq!(broken,before);
}
