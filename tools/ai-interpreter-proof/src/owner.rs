//! Private append owner: state tokens replace per-command base snapshots.
use crate::journal::{MeshJournal,validate_base};
use ingetrazo_core::mesh::{Mesh,MeshVertex,MeshEdge,PreparedSpatialIndex};
use glam::DVec3;use uuid::Uuid;use std::collections::{HashMap,HashSet};
pub struct Transaction{document:Uuid,state:Uuid,journal:MeshJournal}
impl Transaction{pub fn add_edge(&mut self,a:DVec3,b:DVec3)->Result<(),String>{self.journal.add_edge(a,b).map(|_|())}}
struct Delta{before:Uuid,after:Uuid,base_vertices:usize,base_edges:usize,vertices:Vec<MeshVertex>,edges:Vec<MeshEdge>}
pub struct JournalOwner{mesh:Mesh,document:Uuid,state:Uuid,version:u64,undo:Vec<Delta>,redo:Vec<Delta>,index_entry_limit:Option<usize>}
impl JournalOwner{
 pub fn new(mut mesh:Mesh)->Result<Self,String>{validate_base(&mesh)?;let positions:Vec<_>=mesh.vertices.iter().map(|v|v.position).collect();let index=Mesh::prepare_spatial_index(&positions).map_err(str::to_owned)?;mesh.install_prepared_spatial_index(index).map_err(str::to_owned)?;Ok(Self{mesh,document:Uuid::new_v4(),state:Uuid::new_v4(),version:0,undo:vec![],redo:vec![],index_entry_limit:None})}
 pub fn mesh(&self)->&Mesh{&self.mesh}
 pub fn history_counts(&self)->(usize,usize){(self.undo.len(),self.redo.len())}
 pub fn retained_baseline_entries(&self)->usize{0}
 /// Engineering/host admission quota, not a simulated allocator error.
 pub fn set_index_entry_limit(&mut self,limit:Option<usize>){self.index_entry_limit=limit;}
 pub fn begin(&self)->Transaction{Transaction{document:self.document,state:self.state,journal:MeshJournal::begin(&self.mesh,self.version)}}
 fn index(&self,positions:&[DVec3])->Result<PreparedSpatialIndex,String>{if self.index_entry_limit.is_some_and(|n|positions.len()>n){return Err("Prepared-index entry admission denied".into());}Mesh::prepare_spatial_index(positions).map_err(str::to_owned)}
 pub fn replace(&mut self,mut mesh:Mesh)->Result<(),String>{validate_base(&mesh)?;let positions:Vec<_>=mesh.vertices.iter().map(|v|v.position).collect();let index=self.index(&positions)?;mesh.install_prepared_spatial_index(index).map_err(str::to_owned)?;self.mesh=mesh;self.document=Uuid::new_v4();self.state=Uuid::new_v4();self.version+=1;self.undo.clear();self.redo.clear();Ok(())}
 pub fn commit(&mut self,t:Transaction)->Result<bool,String>{
  if t.document!=self.document||t.state!=self.state||t.journal.base_version!=self.version{return Err("Stale owner transaction".into());}
  let j=t.journal;if j.added_vertices.is_empty()&&j.added_edges.is_empty(){return Ok(false);}
  let base_vertices=self.mesh.vertices.len();let base_edges=self.mesh.edges.len();
  let total=base_vertices.checked_add(j.added_vertices.len()).ok_or("Vertex extent overflow")?;
  let mut vertices=Vec::new();vertices.try_reserve(j.added_vertices.len()).map_err(|_|"Vertex preparation refused")?;for p in j.added_vertices{vertices.push(MeshVertex::new(p));}
  let mut edges=Vec::new();edges.try_reserve(j.added_edges.len()).map_err(|_|"Edge preparation refused")?;
  let mut additions=HashMap::<usize,Vec<usize>>::new();additions.try_reserve(j.added_edges.len().checked_mul(2).ok_or("Adjacency extent overflow")?).map_err(|_|"Adjacency preparation refused")?;
  for (offset,(a,b)) in j.added_edges.into_iter().enumerate(){if a>=total||b>=total||a==b{return Err("Invalid planned endpoint".into());}let id=base_edges.checked_add(offset).ok_or("Edge extent overflow")?;for endpoint in [a,b]{let entries=additions.entry(endpoint).or_default();entries.try_reserve(1).map_err(|_|"Adjacency preparation refused")?;entries.push(id);}edges.push(MeshEdge::new(a,b));}
  for (&id,ids) in &additions{if id>=base_vertices{vertices[id-base_vertices].edges.extend(ids);}}
  let mut positions=Vec::new();positions.try_reserve_exact(total).map_err(|_|"Index positions refused")?;positions.extend(self.mesh.vertices.iter().map(|v|v.position));positions.extend(vertices.iter().map(|v|v.position));let index=self.index(&positions)?;
  // Delta copies concern only new records. No base UUID/position/edge arrays
  // survive the transaction, and no Scene/Mesh snapshot is retained.
  let after=Uuid::new_v4();let delta=Delta{before:self.state,after,base_vertices,base_edges,vertices:vertices.clone(),edges:edges.clone()};
  self.undo.try_reserve(1).map_err(|_|"History admission refused")?;self.mesh.vertices.try_reserve(vertices.len()).map_err(|_|"Vertex admission refused")?;self.mesh.edges.try_reserve(edges.len()).map_err(|_|"Edge admission refused")?;
  for (&id,ids) in &additions{if id<base_vertices{self.mesh.vertices[id].edges.try_reserve(ids.len()).map_err(|_|"Live adjacency admission refused")?;}}
  self.mesh.vertices.extend(vertices);self.mesh.edges.extend(edges);for (id,ids) in additions{if id<base_vertices{self.mesh.vertices[id].edges.extend(ids);}}
  // Exact prepared positions plus exclusive private owner make this swap
  // infallible after publication; install performs no allocation.
  self.mesh.install_prepared_spatial_index(index).expect("private prepared append positions");self.state=after;self.version+=1;self.redo.clear();self.undo.push(delta);Ok(true)
 }
 pub fn undo(&mut self)->Result<bool,String>{let Some(d)=self.undo.last()else{return Ok(false)};if self.state!=d.after{return Err("Stale undo state".into());}
  let positions:Vec<_>=self.mesh.vertices[..d.base_vertices].iter().map(|v|v.position).collect();let index=self.index(&positions)?;self.redo.try_reserve(1).map_err(|_|"Redo history admission refused")?;
  let touched:HashSet<_>=d.edges.iter().flat_map(|e|[e.v0,e.v1]).filter(|&id|id<d.base_vertices).collect();
  let d=self.undo.pop().unwrap();self.mesh.edges.truncate(d.base_edges);self.mesh.vertices.truncate(d.base_vertices);for id in touched{self.mesh.vertices[id].edges.retain(|&e|e<d.base_edges);}self.mesh.install_prepared_spatial_index(index).expect("private prepared undo positions");self.state=d.before;self.version+=1;self.redo.push(d);Ok(true)
 }
 pub fn redo(&mut self)->Result<bool,String>{let Some(d)=self.redo.last()else{return Ok(false)};if self.state!=d.before{return Err("Stale redo state".into());}
  let mut positions:Vec<_>=self.mesh.vertices.iter().map(|v|v.position).collect();positions.extend(d.vertices.iter().map(|v|v.position));let index=self.index(&positions)?;
  let vertices=d.vertices.clone();let edges=d.edges.clone();let mut additions=HashMap::<usize,Vec<usize>>::new();for (offset,e) in edges.iter().enumerate(){for id in [e.v0,e.v1]{if id<d.base_vertices{additions.entry(id).or_default().push(d.base_edges+offset);}}}
  self.undo.try_reserve(1).map_err(|_|"Undo history admission refused")?;self.mesh.vertices.try_reserve(vertices.len()).map_err(|_|"Redo vertex admission refused")?;self.mesh.edges.try_reserve(edges.len()).map_err(|_|"Redo edge admission refused")?;for (&id,ids) in &additions{self.mesh.vertices[id].edges.try_reserve(ids.len()).map_err(|_|"Redo adjacency admission refused")?;}
  let d=self.redo.pop().unwrap();self.mesh.vertices.extend(vertices);self.mesh.edges.extend(edges);for (id,ids) in additions{self.mesh.vertices[id].edges.extend(ids);}self.mesh.install_prepared_spatial_index(index).expect("private prepared redo positions");self.state=d.after;self.version+=1;self.undo.push(d);Ok(true)
 }
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn quota_failure_preserves_mesh_history_and_retry(){let mut o=JournalOwner::new(Mesh::new()).unwrap();o.set_index_entry_limit(Some(1));let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(o.commit(t).is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,0));o.set_index_entry_limit(None);let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(o.commit(t).unwrap());let old=o.mesh().clone();o.undo().unwrap();o.set_index_entry_limit(Some(1));assert!(o.redo().is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,1));o.set_index_entry_limit(None);o.redo().unwrap();assert_eq!(o.mesh(),&old);}
 #[test]fn failed_undo_and_replacement_keep_published_state(){let mut m=Mesh::new();for p in [DVec3::ZERO,DVec3::X,DVec3::Y]{m.add_vertex(p);}let mut o=JournalOwner::new(m).unwrap();let mut t=o.begin();t.add_edge(DVec3::Y,DVec3::Z).unwrap();o.commit(t).unwrap();let before=o.mesh().clone();o.set_index_entry_limit(Some(1));assert!(o.undo().is_err());assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),(1,0));assert!(o.replace(before.clone()).is_err());assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),(1,0));}
 #[test]fn prepared_index_refuses_stale_positions_without_lookup_change(){let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);let index=Mesh::prepare_spatial_index(&[DVec3::X]).unwrap();assert!(mesh.install_prepared_spatial_index(index).is_err());assert_eq!(mesh.add_vertex(DVec3::ZERO),0);assert_eq!(mesh.vertices.len(),1);}
 #[test]fn stale_transaction_after_replace_undo_redo_refuses(){let mut o=JournalOwner::new(Mesh::new()).unwrap();let mut stale=o.begin();stale.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.replace(Mesh::new()).unwrap();assert!(o.commit(stale).is_err());let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();let stale=o.begin();o.undo().unwrap();assert!(o.commit(stale).is_err());let stale=o.begin();o.redo().unwrap();assert!(o.commit(stale).is_err());assert_eq!(o.history_counts(),(1,0));}
}
