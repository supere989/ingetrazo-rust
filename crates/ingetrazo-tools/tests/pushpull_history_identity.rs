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
