use ingetrazo_formats::igz::{load_igz,save_igz};
fn main(){
 let dir=std::path::PathBuf::from(std::env::args().nth(1).expect("fixture directory"));
 let mut scene=load_igz(&dir.join("source.igz")).unwrap();
 let token=scene.groups[0].mesh_share.unwrap();
 assert_eq!(scene.groups[1].mesh_share,Some(token));
 assert_ne!(scene.groups[2].mesh_share,Some(token));
 let mut mesh=scene.mesh_shares[&token].clone();mesh.vertices[0].position.z=7.;
 scene.publish_mesh_share(token,mesh).unwrap();
 save_igz(&scene,&dir.join("published.igz")).unwrap();
 scene.make_component_unique(scene.groups[0].id).unwrap();
 save_igz(&scene,&dir.join("unique.igz")).unwrap();
}
