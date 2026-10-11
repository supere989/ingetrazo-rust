use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_tools::pushpull::PushPullTool;

#[test]
fn collapsed_cap_preserves_surviving_face_and_edge_objects() {
    let mut scene=Scene::new();
    let ring=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|scene.mesh.add_vertex(p));
    let face=scene.mesh.add_face(ring.to_vec(),vec![]).unwrap();
    PushPullTool::extrude_face(&mut scene,face,3.).unwrap();
    let base=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==0.)).unwrap();
    let token=scene.mesh.faces[base].history_id;
    let edges=scene.mesh.edges.iter().filter(|e|scene.mesh.vertices[e.v0].position.z==0.&&scene.mesh.vertices[e.v1].position.z==0.).map(|e|(scene.mesh.vertices[e.v0].position,scene.mesh.vertices[e.v1].position,e.history_id)).collect::<Vec<_>>();
    let cap=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==3.)).unwrap();
    PushPullTool::extrude_face(&mut scene,cap,-3.).unwrap();
    assert_eq!(scene.mesh.faces.len(),1);assert_eq!(scene.mesh.faces[0].history_id,token);
    for (a,b,token) in edges {let edge=scene.mesh.edges.iter().find(|e| {let p=scene.mesh.vertices[e.v0].position;let q=scene.mesh.vertices[e.v1].position;(p==a&&q==b)||(p==b&&q==a)}).unwrap();assert_eq!(edge.history_id,token);}
}

#[test]
fn punched_mother_is_new_but_protected_partitions_keep_identity(){
    let mut scene=Scene::new();let ring=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|scene.mesh.add_vertex(p));let id=scene.mesh.add_face(ring.to_vec(),vec![]).unwrap();PushPullTool::extrude_face(&mut scene,id,3.).unwrap();
    let base=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==0.)).unwrap();scene.mesh.faces.remove(base);scene.mesh.rebuild_incidence();let vertices=[DVec3::ZERO,DVec3::Y,DVec3::X+DVec3::Y,DVec3::X].map(|p|scene.mesh.add_vertex(p));
    let a=scene.mesh.add_face(vec![vertices[0],vertices[1],vertices[2]],vec![]).unwrap();let b=scene.mesh.add_face(vec![vertices[0],vertices[2],vertices[3]],vec![]).unwrap();let tokens=[scene.mesh.faces[a].history_id,scene.mesh.faces[b].history_id];
    let cap=scene.mesh.faces.iter().position(|f|f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z==3.)).unwrap();let mother=scene.mesh.faces[cap].history_id;PushPullTool::extrude_face(&mut scene,cap,-3.).unwrap();let replacement=scene.mesh.faces.iter().find(|f|f.holes.len()==2).unwrap();assert_ne!(replacement.history_id,mother);for token in tokens {assert!(scene.mesh.faces.iter().any(|f|f.history_id==token));}
}

#[test]
fn open_sweep_preserves_remote_face_identity_and_unrelated_tjunction() {
    let mut scene=Scene::new();
    let selected=[DVec3::ZERO,DVec3::X,DVec3::X+DVec3::Y,DVec3::Y].map(|p|scene.mesh.add_vertex(p));
    scene.mesh.add_face(selected.to_vec(),vec![]).unwrap();
    let remote=[DVec3::new(10.,0.,5.),DVec3::new(14.,0.,5.),DVec3::new(14.,4.,5.),DVec3::new(10.,4.,5.)].map(|p|scene.mesh.add_vertex(p));
    let id=scene.mesh.add_face(remote.to_vec(),vec![]).unwrap();let token=scene.mesh.faces[id].history_id;
    scene.mesh.add_vertex(DVec3::new(12.,0.,5.));
    PushPullTool::extrude_face(&mut scene,0,-0.6).unwrap();
    let unchanged=scene.mesh.faces.iter().find(|f|f.history_id==token).expect("remote Face remains same object");
    assert_eq!(unchanged.vertices.len(),4);assert_eq!(unchanged.vertices.iter().map(|&v|scene.mesh.vertices[v].position).collect::<Vec<_>>(),vec![DVec3::new(10.,0.,5.),DVec3::new(14.,0.,5.),DVec3::new(14.,4.,5.),DVec3::new(10.,4.,5.)]);
}
