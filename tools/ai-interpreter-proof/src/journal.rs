//! Bounded append-only mesh transaction proof, not an all-document journal.
use glam::DVec3;
use ingetrazo_core::{math::PointKey,mesh::Mesh};
use std::collections::{HashMap};
pub struct MeshJournal {
 pub base_version:u64,
 pub base_vertices:usize,
 pub base_edges:usize,
 vertices:HashMap<PointKey,usize>,
 edges:HashMap<(usize,usize),(usize,usize,usize)>,
 base_positions:Vec<DVec3>,
 pub added_vertices:Vec<DVec3>,
 pub added_edges:Vec<(usize,usize)>,
}
impl MeshJournal {
 pub fn begin(mesh:&Mesh,version:u64)->Self {
  Self {base_version:version,base_vertices:mesh.vertices.len(),base_edges:mesh.edges.len(),
   vertices:mesh.vertices.iter().enumerate().map(|(id,v)|(PointKey::from_vec3(v.position),id)).collect(),
   edges:mesh.edges.iter().enumerate().map(|(id,e)|(ordered(e.v0,e.v1),(id,e.v0,e.v1))).collect(),base_positions:mesh.vertices.iter().map(|v|v.position).collect(),added_vertices:vec![],added_edges:vec![]}
 }
 pub fn add_edge(&mut self,a:DVec3,b:DVec3)->Result<(usize,DVec3,DVec3),String>{
  let a=a.as_vec3().as_dvec3();let b=b.as_vec3().as_dvec3();
  if !a.is_finite()||!b.is_finite(){return Err("Nonfinite endpoint".into());}
  let ka=PointKey::from_vec3(a);let kb=PointKey::from_vec3(b);
  if ka==kb{return Err("Coincident endpoints".into());}
  let v0=self.vertex(ka,a);let v1=self.vertex(kb,b);
  let key=ordered(v0,v1);
  let endpoints=if let Some(&existing)=self.edges.get(&key){existing}else{let id=self.base_edges+self.added_edges.len();self.edges.insert(key,(id,v0,v1));self.added_edges.push((v0,v1));(id,v0,v1)};
  Ok((endpoints.0,self.position(endpoints.1),self.position(endpoints.2)))
 }
 fn position(&self,id:usize)->DVec3{if id<self.base_vertices{self.base_positions[id]}else{self.added_vertices[id-self.base_vertices]}}
 fn vertex(&mut self,key:PointKey,p:DVec3)->usize {
  if let Some(&id)=self.vertices.get(&key){return id;}
  let id=self.base_vertices+self.added_vertices.len();self.vertices.insert(key,id);self.added_vertices.push(p);id
 }
 /// UI owner admission precedes every mutation. Version must cover all writers.
 /// This proof supports only appended unpainted edges: richer writes are refused.
 pub fn publish(self,mesh:&mut Mesh,version:u64)->Result<bool,String>{
  if version!=self.base_version||mesh.vertices.len()!=self.base_vertices||mesh.edges.len()!=self.base_edges{return Err("Stale transaction base".into());}
  if self.added_edges.is_empty(){return Ok(false);}
  for p in self.added_vertices {mesh.add_vertex(p);}
  for (a,b) in self.added_edges {mesh.add_edge(a,b).expect("validated append endpoints");}
  Ok(true)
 }
}
fn ordered(a:usize,b:usize)->(usize,usize){if a<b{(a,b)}else{(b,a)}}
#[cfg(test)] mod tests{
 use super::*;
 #[test]fn discarded_journal_never_mutates_live_mesh(){let mesh=Mesh::new();let before=mesh.clone();let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();drop(j);assert_eq!(mesh,before);}
 #[test]fn stale_publish_is_a_complete_refusal(){let mut mesh=Mesh::new();let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();let before=mesh.clone();assert!(j.publish(&mut mesh,2).is_err());assert_eq!(mesh,before);}
}
