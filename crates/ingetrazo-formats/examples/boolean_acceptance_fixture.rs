use glam::DVec3;
use ingetrazo_core::{scene::Scene,mesh::Mesh,solids::{perform_solid_op,SolidOp}};
fn cube(x:f64)->Mesh{let mut m=Mesh::new();for p in [[x,0.,0.],[x+1.,0.,0.],[x+1.,1.,0.],[x,1.,0.],[x,0.,1.],[x+1.,0.,1.],[x+1.,1.,1.],[x,1.,1.]]{m.add_vertex(DVec3::from_array(p));}for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]{m.add_face(f.to_vec(),vec![]).unwrap();}m}
fn main(){
 let directory=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
 for scenario in ["overlap","rotated","contained"] {
  let a=cube(0.);let mut b=cube(if scenario=="overlap" {0.5}else{0.});
  if scenario=="rotated" {for v in &mut b.vertices {v.position=glam::DMat3::from_rotation_z(30_f64.to_radians())*v.position+DVec3::new(0.5,0.2,0.);}}
  if scenario=="contained" {for v in &mut b.vertices {v.position=v.position*0.5+DVec3::splat(0.25);}}
  for (name,op) in [("union",SolidOp::Union),("subtract",SolidOp::Subtract),("intersect",SolidOp::Intersect)]{
   let mut scene=Scene::new();scene.mesh=perform_solid_op(op,&a,&b).unwrap();
   ingetrazo_formats::igz::save_igz(&scene,&directory.join(format!("{scenario}-{name}.igz"))).unwrap();
  }
 }
}
