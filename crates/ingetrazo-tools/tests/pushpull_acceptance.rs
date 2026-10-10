use glam::DVec3;
use ingetrazo_core::{scene::Scene,mesh::Mesh};
use ingetrazo_tools::pushpull::PushPullTool;
fn cube()->Mesh {
    let mut m=Mesh::new();for p in [[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.],[0.,0.,1.],[1.,0.,1.],[1.,1.,1.],[0.,1.,1.]] {m.add_vertex(DVec3::from_array(p));}
    for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]] {m.add_face(f.to_vec(),vec![]).unwrap();}m
}
fn volume(m:&Mesh)->f64 { m.faces.iter().map(|f| {let a=m.vertices[f.vertices[0]].position;(1..f.vertices.len()-1).map(|i|a.dot(m.vertices[f.vertices[i]].position.cross(m.vertices[f.vertices[i+1]].position))/6.).sum::<f64>()}).sum::<f64>().abs() }
fn closed(m:&Mesh) {let mut edges=std::collections::HashMap::new();for f in &m.faces {for i in 0..f.vertices.len(){let a=f.vertices[i];let b=f.vertices[(i+1)%f.vertices.len()];let e=edges.entry((a.min(b),a.max(b))).or_insert((0,0));e.0+=1;e.1+=if a<b {1}else{-1};}}assert!(edges.values().all(|&(n,o)|n==2&&o==0),"nonmanifold or incorrectly oriented output");}
#[test]
fn existing_solid_cap_push_and_recess_preserve_single_closed_boundary() {
    for (distance,expected) in [(0.5,1.5),(-0.5,0.5)] {let mut s=Scene::new();s.mesh=cube();PushPullTool::extrude_face(&mut s,1,distance).unwrap();assert!((volume(&s.mesh)-expected).abs()<1e-8,"wrong cap volume");closed(&s.mesh);}
}
#[test]
fn invalid_face_does_not_mutate_document() {let mut s=Scene::new();s.mesh=cube();let before=serde_json::to_value(&s).unwrap();assert!(PushPullTool::extrude_face(&mut s,99,1.).is_err());assert_eq!(serde_json::to_value(&s).unwrap(),before);}
#[test]
fn standalone_holed_profile_creates_caps_and_inner_walls() {
    for distance in [1.,-1.] {
        let mut scene=Scene::new();
        let outer=[[0.,0.,0.],[4.,0.,0.],[4.,4.,0.],[0.,4.,0.]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));
        let inner=[[1.,1.,0.],[3.,1.,0.],[3.,3.,0.],[1.,3.,0.]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));
        scene.mesh.add_face(outer.to_vec(),vec![inner.to_vec()]).unwrap();
        PushPullTool::extrude_face(&mut scene,0,distance).unwrap();
        // Surface integral over polygon rings; subtract hole contributions independently.
        let mut signed=0.;let mut edges=std::collections::HashMap::new();
        for face in &scene.mesh.faces {
            let ring_volume=|ring:&Vec<usize>| {let a=scene.mesh.vertices[ring[0]].position;(1..ring.len()-1).map(|i|a.dot(scene.mesh.vertices[ring[i]].position.cross(scene.mesh.vertices[ring[i+1]].position))/6.).sum::<f64>()};
            let outer=ring_volume(&face.vertices);signed+=outer;
            for hole in &face.holes {signed-=outer.signum()*ring_volume(hole).abs();}
            for ring in std::iter::once(&face.vertices).chain(face.holes.iter()) {for i in 0..ring.len(){let a=ring[i];let b=ring[(i+1)%ring.len()];*edges.entry((a.min(b),a.max(b))).or_insert(0)+=1;}}
        }
        assert!((signed.abs()-12.).abs()<1e-8,"holed extrusion expected volume 12, got {}",signed.abs());
        assert!(edges.values().all(|&count|count==2),"hole boundary must have inner walls and closed caps");
    }
}
#[test]
fn nonfinite_distance_is_rejected_atomically() {
    for distance in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY] {let mut scene=Scene::new();scene.mesh=cube();let before=serde_json::to_value(&scene).unwrap();assert!(PushPullTool::extrude_face(&mut scene,1,distance).is_err());assert_eq!(serde_json::to_value(&scene).unwrap(),before);}
}
#[test]
fn embedded_pocket_and_through_cut_have_analytic_removed_volume() {
    for (distance,expected) in [(-0.5,0.875),(-1.,0.75)] {
        let mut scene=Scene::new();scene.mesh=cube();
        let inside=[[0.25,0.25,1.],[0.75,0.25,1.],[0.75,0.75,1.],[0.25,0.75,1.]].map(|p|scene.mesh.add_vertex(DVec3::from_array(p)));
        scene.mesh.faces[1].holes.push(inside.to_vec());scene.mesh.add_face(inside.to_vec(),vec![]).unwrap();
        PushPullTool::extrude_face(&mut scene,6,distance).unwrap();
        let ring_volume=|ring:&Vec<usize>| {let a=scene.mesh.vertices[ring[0]].position;(1..ring.len()-1).map(|i|a.dot(scene.mesh.vertices[ring[i]].position.cross(scene.mesh.vertices[ring[i+1]].position))/6.).sum::<f64>()};
        let actual=scene.mesh.faces.iter().map(|f|{let v=ring_volume(&f.vertices);v-f.holes.iter().map(|h|v.signum()*ring_volume(h).abs()).sum::<f64>()}).sum::<f64>().abs();
        assert!((actual-expected).abs()<1e-8,"expected pocket/through-cut volume {expected}, got {actual}");
        // A through-cut must leave no cap anywhere inside its column.
        if distance==-1. {
            let cap=scene.mesh.faces.iter().any(|f| {
                let inside=f.vertices.iter().all(|&v| {let p=scene.mesh.vertices[v].position;p.x>=0.25&&p.x<=0.75&&p.y>=0.25&&p.y<=0.75});
                let planar=f.vertices.iter().all(|&v|scene.mesh.vertices[v].position.z.abs()<1e-8)
                    ||f.vertices.iter().all(|&v|(scene.mesh.vertices[v].position.z-1.).abs()<1e-8);
                f.holes.is_empty()&&inside&&planar
            });
            assert!(!cap,"through cut retained cap");
        }
    }
}
#[test]
fn attached_edit_preserves_unrelated_solid_and_loose_wire_metadata() {
    let mut scene=Scene::new();scene.mesh=cube();
    let wire_a=scene.mesh.add_vertex(DVec3::new(10.,0.,0.));let wire_b=scene.mesh.add_vertex(DVec3::new(11.,0.,0.));
    let edge=scene.mesh.add_edge(wire_a,wire_b).unwrap();scene.mesh.edges[edge].attrs.insert("audit_wire".into(),serde_json::json!(37));
    let unrelated=cube();for f in &unrelated.faces {let ids=f.vertices.iter().map(|&v|scene.mesh.add_vertex(unrelated.vertices[v].position+DVec3::new(20.,0.,0.))).collect();let face=scene.mesh.add_face(ids,vec![]).unwrap();scene.mesh.faces[face].attrs.insert("audit_other".into(),serde_json::json!(41));}
    PushPullTool::extrude_face(&mut scene,1,-0.5).unwrap();
    assert!(scene.mesh.edges.iter().any(|e|e.attrs.get("audit_wire")==Some(&serde_json::json!(37))&&scene.mesh.vertices[e.v0].position.x==10.&&scene.mesh.vertices[e.v1].position.x==11.),"unrelated wire disappeared");
    assert_eq!(scene.mesh.faces.iter().filter(|f|f.attrs.get("audit_other")==Some(&serde_json::json!(41))).count(),6,"unrelated solid geometry/attributes changed");
}
