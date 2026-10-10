//! Bounded append-only mesh transaction proof, not an all-document journal.
use glam::DVec3;
use ingetrazo_core::{mesh::{Mesh,MeshVertex,MeshEdge}};
use std::collections::{HashMap};
use num_bigint::BigInt;
use num_traits::FromPrimitive;
#[derive(Clone,Eq,PartialEq,Hash)] struct SourceKey([BigInt;3]);
impl SourceKey{fn new(p:DVec3)->Self{Self(p.to_array().map(|x|BigInt::from_f64((x*10000.).round_ties_even()).expect("finite Qt coordinate")))}}
pub struct MeshJournal {
 pub base_version:u64,
 pub base_vertices:usize,
 pub base_edges:usize,
 vertices:HashMap<SourceKey,usize>,
 edges:HashMap<(usize,usize),(usize,usize,usize)>,
 base_positions:Vec<DVec3>,
 pub added_vertices:Vec<DVec3>,
 pub added_edges:Vec<(usize,usize)>,
}
impl MeshJournal {
 pub fn begin(mesh:&Mesh,version:u64)->Self {
  Self {base_version:version,base_vertices:mesh.vertices.len(),base_edges:mesh.edges.len(),
   vertices:mesh.vertices.iter().enumerate().filter(|(_,v)|v.position.is_finite()&&(v.position*10000.).is_finite()).map(|(id,v)|(SourceKey::new(v.position),id)).collect(),
   edges:mesh.edges.iter().enumerate().map(|(id,e)|(ordered(e.v0,e.v1),(id,e.v0,e.v1))).collect(),base_positions:mesh.vertices.iter().map(|v|v.position).collect(),added_vertices:vec![],added_edges:vec![]}
 }
 pub fn add_edge(&mut self,a:DVec3,b:DVec3)->Result<(usize,DVec3,DVec3),String>{
  let a=a.as_vec3().as_dvec3();let b=b.as_vec3().as_dvec3();
  if !a.is_finite()||!b.is_finite(){return Err("Nonfinite endpoint".into());}
  let ka=SourceKey::new(a);let kb=SourceKey::new(b);
  let v0=self.vertex(ka,a);let v1=self.vertex(kb,b);
  if v0==v1{return Err("degenerate edge: endpoints weld to one vertex".into());}
  let key=ordered(v0,v1);
  let endpoints=if let Some(&existing)=self.edges.get(&key){existing}else{let id=self.base_edges+self.added_edges.len();self.edges.insert(key,(id,v0,v1));self.added_edges.push((v0,v1));(id,v0,v1)};
  Ok((endpoints.0,self.position(endpoints.1),self.position(endpoints.2)))
 }
 fn position(&self,id:usize)->DVec3{if id<self.base_vertices{self.base_positions[id]}else{self.added_vertices[id-self.base_vertices]}}
 fn vertex(&mut self,key:SourceKey,p:DVec3)->usize {
  if let Some(&id)=self.vertices.get(&key){return id;}
  // Original registry probes neighbouring cells in dx/dy/dz order,
  // with Qt float32 subtraction and squared-length, strictly below step².
  for dx in -1..=1{for dy in -1..=1{for dz in -1..=1{
   if dx==0&&dy==0&&dz==0{continue;}
   let neighbour=SourceKey([&key.0[0]+dx,&key.0[1]+dy,&key.0[2]+dz]);
   if let Some(&id)=self.vertices.get(&neighbour){
    let d=self.position(id).as_vec3()-p.as_vec3();
    if (d.length_squared() as f64)<1e-8{return id;}
   }
  }}}
  let id=self.base_vertices+self.added_vertices.len();self.vertices.insert(key,id);self.added_vertices.push(p);id
 }
 /// UI owner admission precedes every mutation. Version must cover all writers.
 /// This proof supports only appended unpainted edges: richer writes are refused.
 pub fn publish(self,mesh:&mut Mesh,version:u64)->Result<bool,String>{Ok(self.publish_owned(mesh,version)?.is_some())}
 pub fn publish_owned(self,mesh:&mut Mesh,version:u64)->Result<Option<AppendUndo>,String>{
  if version!=self.base_version||mesh.vertices.len()!=self.base_vertices||mesh.edges.len()!=self.base_edges{return Err("Stale transaction base".into());}
  // Public vertex arrays can have duplicate keys or a stale private spatial
  // index. Never ask add_vertex to reinterpret already planned real IDs.
  validate_base(mesh)?;
  if mesh.vertices.iter().zip(&self.base_positions).any(|(v,p)|v.position!=*p)
   || mesh.edges.iter().enumerate().any(|(id,e)|self.edges.get(&ordered(e.v0,e.v1))!=Some(&(id,e.v0,e.v1))){return Err("Changed transaction base".into());}
  if self.added_edges.is_empty(){return Ok(None);}
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
  for (&id,ids) in &adjacency{if id>=self.base_vertices{vertices[id-self.base_vertices].edges.extend(ids);}}
  let command=AppendUndo{base_vertices:self.base_vertices,base_edges:self.base_edges,
   base_uids:mesh.vertices.iter().map(|v|v.uid).collect(),base_positions:self.base_positions,vertices:vertices.clone(),edges:edges.clone(),applied:true};
  mesh.vertices.extend(vertices);mesh.edges.extend(edges);
  for (id,ids) in adjacency{if id<self.base_vertices{mesh.vertices[id].edges.extend(ids);}}
  mesh.rebuild_spatial_index();
  Ok(Some(command))
 }
}
/// Only appended records are copied; no Scene or base Mesh snapshot.
pub struct AppendUndo{
 base_vertices:usize,base_edges:usize,base_uids:Vec<uuid::Uuid>,base_positions:Vec<DVec3>,
 vertices:Vec<MeshVertex>,edges:Vec<MeshEdge>,applied:bool,
}
impl AppendUndo{
 fn base_matches(&self,m:&Mesh)->bool{m.vertices.len()>=self.base_vertices&&m.vertices[..self.base_vertices].iter().zip(&self.base_uids).all(|(v,id)|v.uid==*id)&&m.vertices[..self.base_vertices].iter().zip(&self.base_positions).all(|(v,p)|v.position==*p)}
 pub fn undo(&mut self,m:&mut Mesh)->Result<(),String>{
  if !self.applied||!self.base_matches(m)||m.vertices.len()!=self.base_vertices+self.vertices.len()||m.edges.len()!=self.base_edges+self.edges.len()
   ||m.vertices[self.base_vertices..]!=self.vertices||m.edges[self.base_edges..]!=self.edges{return Err("Changed append command ownership".into());}
  validate_base(m)?;
  // No faces may acquire the appended vertices/edges behind this command.
  if m.faces.iter().any(|f|f.vertices.iter().chain(f.holes.iter().flatten()).any(|&v|v>=self.base_vertices))||m.edges[self.base_edges..].iter().any(|e|!e.faces.is_empty()){return Err("Append records acquired external owners".into());}
  m.edges.truncate(self.base_edges);m.vertices.truncate(self.base_vertices);
  for edge in &self.edges{for id in [edge.v0,edge.v1]{if id<self.base_vertices{m.vertices[id].edges.retain(|&e|e<self.base_edges);}}}
  m.rebuild_spatial_index();self.applied=false;Ok(())
 }
 pub fn redo(&mut self,m:&mut Mesh)->Result<(),String>{
  if self.applied||!self.base_matches(m)||m.vertices.len()!=self.base_vertices||m.edges.len()!=self.base_edges{return Err("Changed redo base".into());}
  validate_base(m)?;
  m.vertices.try_reserve(self.vertices.len()).map_err(|_|"Redo vertex allocation refused")?;
  m.edges.try_reserve(self.edges.len()).map_err(|_|"Redo edge allocation refused")?;
  let mut additions=HashMap::<usize,Vec<usize>>::new();
  for (i,e) in self.edges.iter().enumerate(){for id in [e.v0,e.v1]{if id<self.base_vertices{additions.entry(id).or_default().push(self.base_edges+i);}}}
  for (&id,edges) in &additions{m.vertices[id].edges.try_reserve(edges.len()).map_err(|_|"Redo adjacency allocation refused")?;}
  m.vertices.extend(self.vertices.iter().cloned());m.edges.extend(self.edges.iter().cloned());
  for (id,edges) in additions{m.vertices[id].edges.extend(edges);}
  m.rebuild_spatial_index();self.applied=true;Ok(())
 }
}
fn validate_base(mesh:&Mesh)->Result<(),String>{
 let mut incidence=vec![0u8;mesh.edges.len()];
 for (id,v) in mesh.vertices.iter().enumerate(){
  if !v.position.is_finite(){return Err("Malformed base vertex".into());}
  for &edge in &v.edges{
   let e=mesh.edges.get(edge).ok_or("Malformed base incidence")?;
   let bit=if e.v0==id{1}else if e.v1==id{2}else{return Err("Malformed base incidence".into());};
   if incidence[edge]&bit!=0{return Err("Duplicate base incidence".into());}
   incidence[edge]|=bit;
  }
 }
 for (id,e) in mesh.edges.iter().enumerate(){
  if e.v0==e.v1||e.v0>=mesh.vertices.len()||e.v1>=mesh.vertices.len()
   ||incidence[id]!=3||e.faces.iter().any(|&f|f>=mesh.faces.len()){return Err("Malformed base edge".into());}
 }
 for f in &mesh.faces{if f.vertices.iter().chain(f.holes.iter().flatten()).any(|&v|v>=mesh.vertices.len()){return Err("Malformed base face".into());}}
 Ok(())
}
fn ordered(a:usize,b:usize)->(usize,usize){if a<b{(a,b)}else{(b,a)}}
#[cfg(test)] mod tests{
 use super::*;
 #[test]fn append_history_preserves_uid_and_refuses_external_ownership(){
  let mut mesh=Mesh::new();let v=mesh.add_vertex(DVec3::ZERO);let base_uid=mesh.vertices[v].uid;
  let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();let mut c=j.publish_owned(&mut mesh,1).unwrap().unwrap();
  let expected=mesh.clone();for _ in 0..3{c.undo(&mut mesh).unwrap();assert_eq!(mesh.vertices.len(),1);assert_eq!(mesh.vertices[0].uid,base_uid);c.redo(&mut mesh).unwrap();assert_eq!(mesh,expected);}
  mesh.faces.push(ingetrazo_core::mesh::MeshFace::new(vec![0,1,0]));let old=mesh.clone();assert!(c.undo(&mut mesh).is_err());assert_eq!(mesh,old);
 }
 #[test]fn redo_changed_base_refuses_without_mutation(){
  let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::ZERO,DVec3::X).unwrap();let mut c=j.publish_owned(&mut mesh,1).unwrap().unwrap();c.undo(&mut mesh).unwrap();mesh.vertices[0].position=DVec3::Y;let old=mesh.clone();assert!(c.redo(&mut mesh).is_err());assert_eq!(mesh,old);
 }
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
 #[test]fn nonfinite_base_is_refused_without_key_panic(){
  let mut mesh=Mesh::new();mesh.vertices.push(MeshVertex::new(DVec3::splat(f64::NAN)));
  let mut j=MeshJournal::begin(&mesh,1);j.add_edge(DVec3::Y,DVec3::X).unwrap();
  assert!(j.publish(&mut mesh,1).is_err());assert_eq!(mesh.vertices.len(),1);assert!(mesh.edges.is_empty());assert!(mesh.vertices[0].position.x.is_nan());
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
