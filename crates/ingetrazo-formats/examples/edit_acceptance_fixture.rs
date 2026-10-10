use glam::{DMat4,DVec3};
use ingetrazo_core::{scene::Scene,groups::Group,mesh::Mesh};
use ingetrazo_formats::igz::{load_igz,save_igz};
fn main(){
    let dir=std::path::PathBuf::from(std::env::args().nth(1).expect("artifact directory"));
    let mut edited=load_igz(&dir.join("original.igz")).unwrap();
    edited.mesh.vertices[0].position.z=2.;edited.groups[0].name="edited instance".into();edited.groups[0].transform=DMat4::from_translation(DVec3::new(7.,8.,9.));
    save_igz(&edited,&dir.join("edited.igz")).unwrap();
    let mut fresh=Scene::new();let mut mesh=Mesh::new();let ids=[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]].map(|p|mesh.add_vertex(DVec3::from_array(p)));mesh.add_face(ids.to_vec(),vec![]).unwrap();
    let mut group=Group::with_mesh("fresh Rust group",mesh);group.transform=DMat4::from_translation(DVec3::new(4.,5.,6.));fresh.groups.push(group);
    let mut view=ingetrazo_core::saved_views::SavedView::new("Rust saved view");view.fov_deg=61.;view.perspective=false;view.parallel=true;view.target=DVec3::new(2.,3.,4.);fresh.saved_views.push(view);
    fresh.section_planes.push(ingetrazo_core::section::SectionPlane::new(DVec3::new(3.,4.,5.),DVec3::X,"Rust section","Q"));
    fresh.geo_datum=Some(ingetrazo_core::georef::SceneDatum::new(-16.,-71.,2400.,12.));
    save_igz(&fresh,&dir.join("fresh.igz")).unwrap();
}
