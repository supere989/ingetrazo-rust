use glam::DVec3;
use ingetrazo_core::mesh::Mesh;
use ingetrazo_tools::followme::{manual_path_extend,manual_path_start};
use std::collections::HashSet;
#[test]
fn original_manual_drag_sequence_and_plane_crossing(){
 let mut mesh=Mesh::new();let vertices=(0..5).map(|i|mesh.add_vertex(DVec3::new(0.,i as f64,0.))).collect::<Vec<_>>();
 let edges=vertices.windows(2).map(|v|mesh.add_edge(v[0],v[1]).unwrap()).collect::<Vec<_>>();
 let mut chain=vertices[..2].to_vec();let no_skip=HashSet::new();
 for (edge,length) in [(2,4),(1,3),(1,2),(1,3)] {assert!(manual_path_extend(&mesh,&mut chain,edges[edge],&no_skip));assert_eq!(chain,vertices[..length]);}
 let baseline=chain.clone();assert!(!manual_path_extend(&mesh,&mut chain,edges[1],&HashSet::from([edges[1]])));assert_eq!(chain,baseline);
 let ring=[DVec3::new(2.,0.,0.),DVec3::new(3.,0.,0.),DVec3::new(3.,0.,1.),DVec3::new(2.,0.,1.)].map(|p|mesh.add_vertex(p));let face=mesh.add_face(ring.to_vec(),vec![]).unwrap();
 let a=mesh.add_vertex(DVec3::new(2.5,-2.,0.5));let b=mesh.add_vertex(DVec3::new(2.5,2.,0.5));let edge=mesh.add_edge(a,b).unwrap();
 for (toward,far) in [(DVec3::new(2.5,-1.,0.5),a),(DVec3::new(2.5,1.,0.5),b)]{let(start,chain)=manual_path_start(&mesh,face,edge,toward).unwrap();assert_eq!(start,Some(DVec3::new(2.5,0.,0.5)));assert_eq!(chain,vec![far]);}
}
