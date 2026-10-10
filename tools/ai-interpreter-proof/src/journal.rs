//! Bounded append-only mesh transaction proof, not an all-document journal.
use glam::DVec3;
use ingetrazo_core::{math::PointKey,mesh::{Mesh,MeshVertex,MeshEdge}};
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
  // Public vertex arrays can have duplicate keys or a stale private spatial
  // index. Never ask add_vertex to reinterpret already planned real IDs.
  validate_base(mesh)?;
  if mesh.vertices.iter().zip(&self.base_positions).any(|(v,p)|v.position!=*p)
   || mesh.edges.iter().enumerate().any(|(id,e)|self.edges.get(&ordered(e.v0,e.v1))!=Some(&(id,e.v0,e.v1))){return Err("Changed transaction base".into());}
  if self.added_edges.is_empty(){return Ok(false);}
  let total=self.base_vertices.checked_add(self.added_vertices.len()).ok_or("Vertex extent overflow")?;
  let mut vertices:Vec<MeshVertex>=self.added_vertices.into_iter().map(MeshVertex::new).collect();
  let mut edges=Vec::new();
  edges.try_reserve(self.added_edges.len()).map_err(|_|"Edge preparation allocation refused")?;
  let mut adjacency=HashMap::<usize,Vec<usize>>::new();
  for (offset,(a,b)) in self.added_edges.into_iter().enumerate(){
   if a>=total||b>=total||a==b{return Err("Invalid planned endpoints".into());}
   let id=self.base_edges.checked_add(offset).ok_or("Edge extent overflow")?;
   adjacency.entry(a).or_default().push(id);adjacency.entry(b).or_default().push(id);
   edges.push(MeshEdge::new(a,b));
  }
  mesh.vertices.try_reserve(vertices.len()).map_err(|_|"Vertex publication allocation refused")?;
  mesh.edges.try_reserve(edges.len()).map_err(|_|"Edge publication allocation refused")?;
  // Reserve every live adjacency before any semantic publication.
  for (&id,ids) in &adjacency {
   let v=if id<self.base_vertices{&mut mesh.vertices[id]}else{&mut vertices[id-self.base_vertices]};
   v.edges.try_reserve(ids.len()).map_err(|_|"Adjacency allocation refused")?;
  }
  mesh.vertices.extend(vertices);mesh.edges.extend(edges);
  for (id,ids) in adjacency{mesh.vertices[id].edges.extend(ids);}
  mesh.rebuild_spatial_index();
  Ok(true)
 }
}
fn validate_base(mesh:&Mesh)->Result<(),String>{
 for (id,e) in mesh.edges.iter().enumerate(){
  if e.v0==e.v1||e.v0>=mesh.vertices.len()||e.v1>=mesh.vertices.len()
   ||!mesh.vertices[e.v0].edges.contains(&id)||!mesh.vertices[e.v1].edges.contains(&id)
   ||e.faces.iter().any(|&f|f>=mesh.faces.len()){return Err("Malformed base edge".into());}
 }
 for (id,v) in mesh.vertices.iter().enumerate(){
  if !v.position.is_finite()||v.edges.iter().any(|&e|mesh.edges.get(e).is_none_or(|e|!e.contains_vertex(id))){return Err("Malformed base vertex".into());}
 }
 for f in &mesh.faces{if f.vertices.iter().chain(f.holes.iter().flatten()).any(|&v|v>=mesh.vertices.len()){return Err("Malformed base face".into());}}
 Ok(())
}
fn ordered(a:usize,b:usize)->(usize,usize){if a<b{(a,b)}else{(b,a)}}
#[cfg(test)] mod tests{
 use super::*;
 #[test]fn stale_private_index_cannot_redirect_appended_ids(){
  let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);
  mesh.vertices[0].position=DVec3::Y; // private index still maps zero -> 0
  mesh.vertices.push(MeshVertex::new(DVec3::Y)); // duplicate public key
  let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();
  j.publish(&mut mesh,1).unwrap();
  assert_eq!(mesh.vertices.len(),4);assert_eq!((mesh.edges[0].v0,mesh.edges[0].v1),(2,3));
  assert_eq!(mesh.vertices[2].position,DVec3::ZERO);assert_eq!(mesh.vertices[0].position,DVec3::Y);
  assert_eq!(mesh.vertices[2].edges,vec![0]);assert_eq!(mesh.vertices[3].edges,vec![0]);
 }
 #[test]fn malformed_base_refused_before_semantic_mutation(){
  let mut mesh=Mesh::new();mesh.vertices.push(MeshVertex::new(DVec3::ZERO));
  mesh.edges.push(MeshEdge::new(0,999));
  let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::Y,DVec3::X).unwrap();
  let old=mesh.clone();assert!(j.publish(&mut mesh,1).is_err());assert_eq!(mesh,old);
 }
 #[test]fn discarded_journal_never_mutates_live_mesh(){let mesh=Mesh::new();let before=mesh.clone();let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();drop(j);assert_eq!(mesh,before);}
 #[test]fn stale_publish_is_a_complete_refusal(){let mut mesh=Mesh::new();let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();let before=mesh.clone();assert!(j.publish(&mut mesh,2).is_err());assert_eq!(mesh,before);}
}
