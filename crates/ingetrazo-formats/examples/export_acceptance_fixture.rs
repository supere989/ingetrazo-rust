use glam::{DMat4,DVec3};
use ingetrazo_core::{scene::Scene,groups::Group,mesh::Mesh};
fn main() {
    let directory=std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut scene=Scene::new();let mut mesh=Mesh::new();
    let ids=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]].map(|p|mesh.add_vertex(DVec3::from_array(p)));
    mesh.add_face(ids.to_vec(),vec![]).unwrap();
    let mut child=Group::with_mesh("reflected child",mesh);
    child.transform=DMat4::from_translation(DVec3::new(2.,3.,4.))*DMat4::from_scale(DVec3::new(-1.,1.,1.));
    let mut parent=Group::new("translated parent");parent.transform=DMat4::from_translation(DVec3::new(10.,20.,30.));parent.children.push(child);scene.groups.push(parent);
    let mut json=std::fs::File::create(directory.join("fixture.gltf")).unwrap();
    ingetrazo_formats::gltf::export_gltf(&scene,&mut json).unwrap();
    let mut binary=std::fs::File::create(directory.join("fixture.glb")).unwrap();
    ingetrazo_formats::gltf::export_glb(&scene,&mut binary).unwrap();
}
