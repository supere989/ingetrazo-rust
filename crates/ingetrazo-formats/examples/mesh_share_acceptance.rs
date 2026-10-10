use ingetrazo_formats::igz::{load_igz,save_igz};
fn main(){
 let dir=std::path::PathBuf::from(std::env::args().nth(1).expect("fixture directory"));
 let mut scene=load_igz(&dir.join("source.igz")).unwrap();
 let token=scene.groups[0].mesh_share.unwrap();
 assert_eq!(scene.groups[1].mesh_share,Some(token));
 assert_ne!(scene.groups[2].mesh_share,Some(token));
 let mut mesh=scene.mesh_shares[&token].clone();mesh.vertices[0].position.z=7.;
 scene.publish_mesh_share(token,mesh).unwrap();
 let copied=scene.groups[3].children[0].duplicate_instance();
 scene.groups[3].children.push(copied);
 save_igz(&scene,&dir.join("published.igz")).unwrap();
 scene.make_component_unique(scene.groups[0].id).unwrap();
 save_igz(&scene,&dir.join("unique.igz")).unwrap();
 // A component-marked placement can have private own geometry while its
 // definition and siblings retain their independently registered prototype.
 let private=uuid::Uuid::new_v4();
 let mesh=scene.groups[1].mesh.clone();
 scene.register_mesh_share(private,mesh.clone()).unwrap();
 scene.groups[3].mesh_share=Some(private);scene.groups[3].mesh=mesh;
 save_igz(&scene,&dir.join("private-container.igz")).unwrap();
}
