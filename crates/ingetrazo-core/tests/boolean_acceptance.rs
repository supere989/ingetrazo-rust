//! Independent analytic boundary-volume oracle, intentionally avoids Mesh::volume.
use glam::DVec3;
use ingetrazo_core::{mesh::Mesh, solids::{perform_solid_op, SolidOp}};
fn cube(x: f64) -> Mesh {
    let mut m=Mesh::new();
    for p in [[x,0.,0.],[x+1.,0.,0.],[x+1.,1.,0.],[x,1.,0.],[x,0.,1.],[x+1.,0.,1.],[x+1.,1.,1.],[x,1.,1.]] {m.add_vertex(DVec3::from_array(p));}
    for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]] {m.add_face(f.to_vec(),vec![]).unwrap();} m
}
fn independent_volume(m:&Mesh)->f64 {
    m.faces.iter().map(|f| {
        assert!(f.holes.is_empty(),"Oracle corpus has no holes");
        let a=m.vertices[f.vertices[0]].position;
        (1..f.vertices.len()-1).map(|i| a.dot(m.vertices[f.vertices[i]].position.cross(m.vertices[f.vertices[i+1]].position))/6.).sum::<f64>()
    }).sum::<f64>().abs()
}
fn oriented_closed_boundary(m:&Mesh) {
    use std::collections::HashMap;
    let mut edges=HashMap::new();
    for f in &m.faces { for i in 0..f.vertices.len() {
        let a=f.vertices[i];let b=f.vertices[(i+1)%f.vertices.len()];
        let entry=edges.entry((a.min(b),a.max(b))).or_insert((0,0));entry.0+=1;entry.1+=if a<b {1}else{-1};
    }}
    assert!(edges.values().all(|&(count,orientation)| count==2 && orientation==0),"Independent boundary edge incidence/orientation check failed");
}
#[test]
fn overlapping_boxes_have_analytic_boolean_volumes_and_closed_boundaries() {
    for (op, expected) in [(SolidOp::Union,1.5),(SolidOp::Subtract,0.5),(SolidOp::Intersect,0.5)] {
        let mesh=perform_solid_op(op,&cube(0.),&cube(0.5)).unwrap();
        let actual=independent_volume(&mesh);
        assert!((actual-expected).abs()<1e-8,"{op:?}: expected volume {expected}, got {actual}");
        oriented_closed_boundary(&mesh);
    }
}
#[test]
fn oracle_rejects_old_copy_and_concatenate_placeholders() {
    assert!((independent_volume(&cube(0.))-0.5).abs()>1e-8);
    let mut concatenated=cube(0.);
    let second=cube(0.5);
    for f in &second.faces {let ids=f.vertices.iter().map(|&i|concatenated.add_vertex(second.vertices[i].position)).collect();concatenated.add_face(ids,vec![]).unwrap();}
    assert!((independent_volume(&concatenated)-1.5).abs()>1e-8);
}
