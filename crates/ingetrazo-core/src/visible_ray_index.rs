//! Immutable visible triangles and a direct balanced ray broad phase.
use crate::{scene::Scene,math::{Ray,ray_triangle_intersect}};
use glam::DVec3;
#[derive(Clone,Copy)]
struct Triangle { vertices:[DVec3;3], lo:DVec3,hi:DVec3 }
struct Node { lo:DVec3,hi:DVec3,begin:usize,end:usize,children:Option<(usize,usize)> }
pub struct VisibleRayIndex {triangles:Vec<Triangle>,nodes:Vec<Node>}
impl VisibleRayIndex {
 pub fn new(scene:&Scene,max_bytes:usize)->Result<Self,String>{
  let mut triangles=Vec::new();let mut error=None;
  crate::screen_pick::visit_placements(scene,|mesh,world,_|{
   if error.is_some(){return;}
   for face in 0..mesh.faces.len(){
    if !scene.mesh_face_visible(mesh,face){continue;}
    for (a,b,c) in mesh.triangulate_face(face){
     let vertices=[a,b,c].map(|p|world.transform_point3(p));
     if vertices.iter().any(|v|!v.is_finite()){error=Some("Nonfinite visible collision geometry");return;}
     let per=std::mem::size_of::<Triangle>()+2*std::mem::size_of::<Node>();
     if triangles.len()>=max_bytes/per {error=Some("Visible collision index memory admission exceeded");return;}
     if triangles.len()==triangles.capacity(){
      let capacity=triangles.capacity().saturating_mul(2).max(64).min(max_bytes/per);
      if triangles.try_reserve_exact(capacity-triangles.len()).is_err(){error=Some("Visible collision triangle allocation failed");return;}
     }
     let lo=vertices.into_iter().fold(DVec3::splat(f64::INFINITY),DVec3::min);
     let hi=vertices.into_iter().fold(DVec3::splat(f64::NEG_INFINITY),DVec3::max);
     let pad=(hi-lo).max_element()*1e-9+lo.abs().max(hi.abs()).max_element()*f64::EPSILON*64.+1e-12;
     let lo=lo-DVec3::splat(pad);let hi=hi+DVec3::splat(pad);
     if !lo.is_finite()||!hi.is_finite(){error=Some("Collision bounds exceed finite range");return;}
     triangles.push(Triangle{vertices,lo,hi});
    }
   }
  });
  if let Some(e)=error{return Err(e.into());}
  let mut result=Self{triangles,nodes:Vec::new()};
  let reserve=result.triangles.len().checked_mul(2).ok_or("Collision node count overflow")?;
  result.nodes.try_reserve(reserve).map_err(|_|"Visible collision node allocation failed")?;
  if !result.triangles.is_empty(){result.build(0,result.triangles.len());}
  Ok(result)
 }
 fn build(&mut self,begin:usize,end:usize)->usize{
  let mut lo=DVec3::splat(f64::INFINITY);let mut hi=DVec3::splat(f64::NEG_INFINITY);
  for t in &self.triangles[begin..end]{lo=lo.min(t.lo);hi=hi.max(t.hi);}
  let id=self.nodes.len();self.nodes.push(Node{lo,hi,begin,end,children:None});
  if end-begin>8{
   let extent=hi-lo;let axis=if extent.x>=extent.y&&extent.x>=extent.z{0}else if extent.y>=extent.z{1}else{2};
   let mid=(end-begin)/2;
   self.triangles[begin..end].select_nth_unstable_by(mid,|a,b|(a.lo[axis]*0.5+a.hi[axis]*0.5).total_cmp(&(b.lo[axis]*0.5+b.hi[axis]*0.5)));
   let split=begin+mid;let left=self.build(begin,split);let right=self.build(split,end);
   self.nodes[id].children=Some((left,right));
  }
  id
 }
 pub fn triangle_count(&self)->usize{self.triangles.len()}
 /// Reuses triangulation and traverses nodes without allocations. Section state is authoritative at query.
 pub fn distance(&self,scene:&Scene,ray:&Ray)->Option<f64>{
  if !ray.origin.is_finite()||!ray.dir.is_finite()||self.nodes.is_empty(){return None;}
  let mut best=f64::INFINITY;self.query(0,scene,ray,&mut best);if best.is_finite(){Some(best)}else{None}
 }
 fn query(&self,id:usize,scene:&Scene,ray:&Ray,best:&mut f64){
  let node=&self.nodes[id];let mut near=0_f64;let mut far=*best;
  for axis in 0..3{
   let o=ray.origin[axis];let d=ray.dir[axis];
   if d==0.{if o<node.lo[axis]||o>node.hi[axis]{return;}}
   else{let a=(node.lo[axis]-o)/d;let b=(node.hi[axis]-o)/d;near=near.max(a.min(b));far=far.min(a.max(b));if near>far{return;}}
  }
  if let Some((left,right))=node.children{self.query(left,scene,ray,best);self.query(right,scene,ray,best);}
  else{for triangle in &self.triangles[node.begin..node.end]{
   let [a,b,c]=triangle.vertices;
   if let Some((t,p))=ray_triangle_intersect(ray,a,b,c){if t<*best&&scene.section_point_visible(p)&&(b-a).cross(c-a).normalize_or_zero().length_squared()>0.{*best=t;}}
  }}
 }
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn cached_distances_match_visible_picker_and_refuse_small_admission(){
  let mut scene=Scene::new();
  for z in [0.,-1.]{
   let vertices=[DVec3::new(-1.,-1.,z),DVec3::new(1.,-1.,z),DVec3::new(1.,1.,z),DVec3::new(-1.,1.,z)].map(|p|scene.mesh.add_vertex(p));
   scene.mesh.add_face(vertices.to_vec(),vec![]);
  }
  let index=VisibleRayIndex::new(&scene,1024*1024).unwrap();
  assert!(VisibleRayIndex::new(&scene,1).is_err());
  for x in [-1.1,-1.,-0.5,0.,0.5,1.,1.1]{
   let ray=Ray::new(DVec3::new(x,0.,3.),-DVec3::Z);
   assert_eq!(index.distance(&scene,&ray),crate::screen_pick::hover_face(&scene,&ray).map(|h|h.2));
  }
 }
 #[test]fn balanced_broad_phase_matches_direct_queries_over_many_faces(){
  let mut scene=Scene::new();
  for x in -10..10 {for y in -10..10 {
   let z=((x*x+y*y)%7)as f64/5.;
   let ids=[DVec3::new(x as f64,y as f64,z),DVec3::new(x as f64+0.8,y as f64,z),DVec3::new(x as f64+0.8,y as f64+0.8,z),DVec3::new(x as f64,y as f64+0.8,z)].map(|p|scene.mesh.add_vertex(p));
   scene.mesh.add_face(ids.to_vec(),vec![]);
  }}
  let index=VisibleRayIndex::new(&scene,1024*1024).unwrap();assert_eq!(index.triangle_count(),800);
  for i in 0..1000 {
   let x=((i*37)%220)as f64/10.-11.;let y=((i*73)%220)as f64/10.-11.;
   for direction in [-DVec3::Z,DVec3::new(0.2,-0.1,-1.),DVec3::X] {
    let ray=Ray::new(DVec3::new(x,y,4.),direction);
    assert_eq!(index.distance(&scene,&ray),crate::screen_pick::hover_face(&scene,&ray).map(|h|h.2));
   }
  }
 }

 #[test]fn nested_reflection_hidden_faces_and_live_section_follow_visible_picker(){
  use crate::{groups::Group,mesh::Mesh,section::SectionPlane};use glam::DMat4;
  let mut mesh=Mesh::new();
  let ids=[DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(1.,1.,0.),DVec3::new(-1.,1.,0.)].map(|p|mesh.add_vertex(p));
  mesh.add_face(ids.to_vec(),vec![]);
  let mut child=Group::with_mesh("floor",mesh);child.transform=DMat4::from_scale(DVec3::new(-2.,0.5,1.));
  let mut parent=Group::new("translated");parent.transform=DMat4::from_translation(DVec3::new(3.,2.,1.));parent.children.push(child);
  let mut scene=Scene::new();scene.groups.push(parent);
  let index=VisibleRayIndex::new(&scene,1024*1024).unwrap();
  for x in [0.,1.,2.,3.,4.,5.,6.] {let ray=Ray::new(DVec3::new(x,2.,4.),-DVec3::Z);assert_eq!(index.distance(&scene,&ray),crate::screen_pick::hover_face(&scene,&ray).map(|h|h.2));}
  scene.section_planes.push(SectionPlane::new(DVec3::new(3.,2.,0.5),DVec3::Z,"cut","A"));
  let ray=Ray::new(DVec3::new(3.,2.,4.),-DVec3::Z);assert_eq!(index.distance(&scene,&ray),None);
  scene.section_planes[0].flip();assert_eq!(index.distance(&scene,&ray),Some(3.));
  scene.groups[0].children[0].mesh.faces[0].attrs.insert("hidden".into(),serde_json::json!(true));
  assert_eq!(VisibleRayIndex::new(&scene,1024*1024).unwrap().distance(&scene,&ray),None);
  scene.groups[0].hidden=true;assert_eq!(VisibleRayIndex::new(&scene,1024*1024).unwrap().triangle_count(),0);
 }

}
