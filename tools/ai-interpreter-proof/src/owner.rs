//! Private append owner: state tokens replace per-command base snapshots.
use crate::journal::{MeshJournal,validate_base};
use crate::owner_budget::{OwnerBudget,Lease,Kind};
use crate::owner_registry::Registry;
use ingetrazo_core::mesh::{Mesh,MeshVertex,MeshEdge,PreparedSpatialIndex,PreparedSpatialDelta};
use glam::DVec3;use uuid::Uuid;use std::collections::{HashMap,HashSet};
pub struct Transaction{pub(crate) force_command:bool,document:Uuid,state:Uuid,pub(crate) journal:MeshJournal,charge:Lease,admission_failed:bool}
impl Transaction{pub fn registry_lookup_calls(&self)->usize{self.journal.registry.as_ref().unwrap().lookup_calls()} pub fn add_edge(&mut self,a:DVec3,b:DVec3)->Result<(),String>{
 if self.admission_failed{return Err("Transaction shared admission previously refused".into());}
 let next=staging_bound(0,0,self.journal.added_vertices.len().checked_add(2).ok_or("Staging extent overflow")?,self.journal.added_edges.len().checked_add(1).ok_or("Staging extent overflow")?)?;
 if let Err(error)=self.charge.resize(next){self.admission_failed=true;return Err(error);}
 let result=self.journal.add_edge(a,b).map(|_|());
 self.charge.resize(staging_bound(0,0,self.journal.added_vertices.len(),self.journal.added_edges.len())?)?;result
}}
struct Delta{before:Uuid,after:Uuid,base_vertices:usize,base_edges:usize,vertices:Vec<MeshVertex>,edges:Vec<MeshEdge>,bytes:usize,charge:Lease,spatial:PreparedSpatialDelta}
pub struct JournalOwner{mesh:Mesh,document:Uuid,state:Uuid,version:u64,undo:Vec<Delta>,redo:Vec<Delta>,index_entry_limit:Option<usize>,history_byte_limit:Option<usize>,undo_bytes:usize,redo_bytes:usize,append_vertex_limit:Option<usize>,append_edge_limit:Option<usize>,budget:OwnerBudget,registry:Registry}
/// Exclusive prepared publication. Dropping it discards the unpublished delta;
/// holding it prevents any owner mutation between admission and application.
pub struct PreparedAppend<'a>{owner:&'a mut JournalOwner,vertices:Vec<MeshVertex>,edges:Vec<MeshEdge>,additions:HashMap<usize,Vec<usize>>,delta:Delta,registry_change:crate::owner_registry::Change,base_vertices:usize,after:Uuid,next_version:u64,projected:usize,_preparation:Lease,_staging:Lease}
impl<'a> PreparedAppend<'a>{
 pub fn apply(self)->&'a mut JournalOwner {
  let Self{owner,vertices,edges,additions,delta,registry_change,base_vertices,after,next_version,projected,_preparation,_staging}=self;
  owner.mesh.vertices.extend(vertices);owner.mesh.edges.extend(edges);for(id,ids)in additions{if id<base_vertices{owner.mesh.vertices[id].edges.extend(ids);}}
  owner.mesh.install_spatial_append(&delta.spatial).expect("exclusive prepared append positions");owner.state=after;owner.version=next_version;registry_change.apply(after,next_version).expect("exclusive prepared registry append");owner.redo.clear();owner.redo_bytes=0;owner.undo_bytes=projected;owner.undo.push(delta);
  drop(_preparation);drop(_staging);owner
 }
}
impl JournalOwner{
 pub fn new(mesh:Mesh)->Result<Self,String>{Self::new_with_budget(mesh,OwnerBudget::new(None))}
 pub fn new_with_budget(mut mesh:Mesh,budget:OwnerBudget)->Result<Self,String>{let _preparation=budget.reserve(Kind::Preparation,preparation_bound(mesh.vertices.len(),mesh.edges.len())?)?;validate_base(&mesh)?;let positions=positions(&mesh.vertices,0)?;let index=Mesh::prepare_spatial_index(&positions).map_err(str::to_owned)?;mesh.install_prepared_spatial_index(index).map_err(str::to_owned)?;let document=Uuid::new_v4();let state=Uuid::new_v4();let registry=Registry::new(&mesh,budget.clone(),document,state,0)?;Ok(Self{mesh,document,state,version:0,undo:vec![],redo:vec![],index_entry_limit:None,history_byte_limit:None,undo_bytes:0,redo_bytes:0,append_vertex_limit:None,append_edge_limit:None,budget:budget.clone(),registry})}
 pub fn registry_metrics(&self)->crate::owner_registry::Metrics{self.registry.metrics()}
 pub fn registry_identity(&self)->usize{self.registry.identity()}
 pub fn budget(&self)->OwnerBudget{self.budget.clone()}
 pub fn mesh(&self)->&Mesh{&self.mesh}
 /// Consume a private newly constructed owner without cloning native geometry.
 pub(crate) fn into_mesh(self)->Mesh{self.mesh}
 pub(crate) fn face_storage(&mut self)->(&mut Vec<ingetrazo_core::mesh::MeshFace>,&mut Vec<MeshEdge>){(&mut self.mesh.faces,&mut self.mesh.edges)}
 pub fn history_counts(&self)->(usize,usize){(self.undo.len(),self.redo.len())}
 pub fn retained_baseline_entries(&self)->usize{0}
 /// Engineering/host admission quota, not a simulated allocator error.
 pub fn set_index_entry_limit(&mut self,limit:Option<usize>){self.index_entry_limit=limit;}
 /// Retained delta payload admission, excluding VM, scratch and history Vec slack.
 pub fn set_history_byte_limit(&mut self,limit:Option<usize>){self.history_byte_limit=limit;}
 pub fn history_payload_bytes(&self)->(usize,usize){(self.undo_bytes,self.redo_bytes)}
 pub fn set_append_limits(&mut self,vertices:Option<usize>,edges:Option<usize>){self.append_vertex_limit=vertices;self.append_edge_limit=edges;}
 pub fn begin(&self)->Transaction{self.try_begin().expect("legacy proof owner preparation")}
 pub fn try_begin(&self)->Result<Transaction,String>{let charge=self.budget.reserve(Kind::Staging,staging_bound(0,0,0,0)?)?;let mut journal=MeshJournal::from_registry(self.registry.view(),self.version)?;journal.set_append_limits(self.append_vertex_limit,self.append_edge_limit);Ok(Transaction{force_command:false,document:self.document,state:self.state,journal,charge,admission_failed:false})}
 fn index(&self,positions:&[DVec3])->Result<PreparedSpatialIndex,String>{if self.index_entry_limit.is_some_and(|n|positions.len()>n){return Err("Prepared-index entry admission denied".into());}Mesh::prepare_spatial_index(positions).map_err(str::to_owned)}
 pub fn replace(&mut self,mut mesh:Mesh)->Result<(),String>{let next_version=self.version.checked_add(1).ok_or("Owner revision overflow")?;let _preparation=self.budget.reserve(Kind::Preparation,preparation_bound(mesh.vertices.len(),mesh.edges.len())?)?;validate_base(&mesh)?;let positions=positions(&mesh.vertices,0)?;let index=self.index(&positions)?;mesh.install_prepared_spatial_index(index).map_err(str::to_owned)?;let document=Uuid::new_v4();let state=Uuid::new_v4();let registry=Registry::new(&mesh,self.budget.clone(),document,state,next_version)?;self.registry.invalidate();self.registry=registry;self.mesh=mesh;self.document=document;self.state=state;self.version=next_version;self.undo.clear();self.redo.clear();self.undo_bytes=0;self.redo_bytes=0;Ok(())}
 pub fn commit(&mut self,t:Transaction)->Result<bool,String>{let Some(prepared)=self.prepare(t)? else{return Ok(false)};prepared.apply();Ok(true)}
 pub fn prepare(&mut self,t:Transaction)->Result<Option<PreparedAppend<'_>>,String>{
  if t.document!=self.document||t.state!=self.state||t.journal.base_version!=self.version{return Err("Stale owner transaction".into());}
  if t.admission_failed{return Err("Transaction shared staging admission failed".into());}
  let next_version=self.version.checked_add(1).ok_or("Owner revision overflow")?;
  let force=t.force_command;let j=t.journal;let _staging=t.charge;if j.admission_failed{return Err("Transaction staging admission failed".into());}if !force&&j.added_vertices.is_empty()&&j.added_edges.is_empty(){return Ok(None);}
  let base_vertices=self.mesh.vertices.len();let base_edges=self.mesh.edges.len();
  let total=base_vertices.checked_add(j.added_vertices.len()).ok_or("Vertex extent overflow")?;
  let edge_total=base_edges.checked_add(j.added_edges.len()).ok_or("Edge extent overflow")?;
  let _preparation=self.budget.reserve(Kind::Preparation,preparation_bound(j.added_vertices.len(),j.added_edges.len())?)?;
  let mut history_charge=self.budget.reserve(Kind::History,delta_bound(j.added_vertices.len(),j.added_edges.len())?)?;
  let mut vertices=Vec::new();vertices.try_reserve(j.added_vertices.len()).map_err(|_|"Vertex preparation refused")?;for p in j.added_vertices{vertices.push(MeshVertex::new(p));}
  let mut edges=Vec::new();edges.try_reserve(j.added_edges.len()).map_err(|_|"Edge preparation refused")?;
  let mut additions=HashMap::<usize,Vec<usize>>::new();additions.try_reserve(j.added_edges.len().checked_mul(2).ok_or("Adjacency extent overflow")?).map_err(|_|"Adjacency preparation refused")?;
  for (offset,(a,b)) in j.added_edges.into_iter().enumerate(){if a>=total||b>=total||(!force&&a==b){return Err("Invalid planned endpoint".into());}let id=base_edges.checked_add(offset).ok_or("Edge extent overflow")?;for endpoint in [a,b].into_iter().take(if a==b{1}else{2}){let entries=additions.entry(endpoint).or_default();entries.try_reserve(1).map_err(|_|"Adjacency preparation refused")?;entries.push(id);}edges.push(MeshEdge::new(a,b));}
  for (&id,ids) in &additions{if id>=base_vertices{let adjacency=&mut vertices[id-base_vertices].edges;adjacency.try_reserve_exact(ids.len()).map_err(|_|"Prepared vertex adjacency refused")?;adjacency.extend(ids);}}
  if self.index_entry_limit.is_some_and(|n|total>n){return Err("Prepared-index entry admission denied".into());}let appended_positions=positions(&vertices,0)?;let spatial=self.mesh.prepare_spatial_append(&appended_positions).map_err(str::to_owned)?;
  // Delta copies concern only new records. No base UUID/position/edge arrays
  // survive the transaction, and no Scene/Mesh snapshot is retained.
  let after=Uuid::new_v4();let retained_vertices=copy_vertices(&vertices)?;let retained_edges=copy_append_edges(&edges)?;
  let bytes=delta_bytes(&retained_vertices,&retained_edges)?.checked_add(spatial.retained_bytes()).ok_or("Spatial delta byte overflow")?;
  history_charge.resize(bytes)?;
  let projected=self.undo_bytes.checked_add(bytes).ok_or("History payload extent overflow")?;
  if self.history_byte_limit.is_some_and(|limit|projected>limit){return Err("History payload admission denied".into());}
  let delta=Delta{before:self.state,after,base_vertices,base_edges,vertices:retained_vertices,edges:retained_edges,bytes,charge:history_charge,spatial};
  self.undo.try_reserve(1).map_err(|_|"History admission refused")?;self.mesh.vertices.try_reserve(vertices.len()).map_err(|_|"Vertex admission refused")?;self.mesh.edges.try_reserve(edges.len()).map_err(|_|"Edge admission refused")?;
  for (&id,ids) in &additions{if id<base_vertices{self.mesh.vertices[id].edges.try_reserve(ids.len()).map_err(|_|"Live adjacency admission refused")?;}}
  let registry_change=self.registry.prepare_append(&vertices,&edges)?;
  Ok(Some(PreparedAppend{owner:self,vertices,edges,additions,delta,registry_change,base_vertices,after,next_version,projected,_preparation,_staging}))
 }
 pub fn undo(&mut self)->Result<bool,String>{let Some(d)=self.undo.last()else{return Ok(false)};if self.state!=d.after{return Err("Stale undo state".into());}
  let next_version=self.version.checked_add(1).ok_or("Owner revision overflow")?;let _preparation=self.budget.reserve(Kind::Preparation,preparation_bound(d.vertices.len(),d.edges.len())?)?;if self.index_entry_limit.is_some_and(|n|d.base_vertices>n){return Err("Prepared-index entry admission denied".into());}self.redo.try_reserve(1).map_err(|_|"Redo history admission refused")?;
  let mut touched=HashSet::new();touched.try_reserve(d.edges.len().checked_mul(2).ok_or("Touched extent overflow")?).map_err(|_|"Touched endpoint admission refused")?;for e in &d.edges{for id in [e.v0,e.v1]{if id<d.base_vertices{touched.insert(id);}}}
  let registry_change=self.registry.prepare_truncate(d.base_vertices,d.base_edges,&d.vertices,&d.edges)?;
  self.mesh.restore_spatial_append(&d.spatial).map_err(str::to_owned)?;let d=self.undo.pop().unwrap();self.mesh.edges.truncate(d.base_edges);self.mesh.vertices.truncate(d.base_vertices);for id in touched{self.mesh.vertices[id].edges.retain(|&e|e<d.base_edges);}self.state=d.before;self.version=next_version;registry_change.apply(d.before,next_version).expect("private prepared registry undo");self.undo_bytes-=d.bytes;self.redo_bytes+=d.bytes;self.redo.push(d);Ok(true)
 }
 pub fn redo(&mut self)->Result<bool,String>{let Some(d)=self.redo.last()else{return Ok(false)};if self.state!=d.before{return Err("Stale redo state".into());}
  let next_version=self.version.checked_add(1).ok_or("Owner revision overflow")?;let total=self.mesh.vertices.len().checked_add(d.vertices.len()).ok_or("Vertex extent overflow")?;let _preparation=self.budget.reserve(Kind::Preparation,preparation_bound(d.vertices.len(),d.edges.len())?)?;if self.index_entry_limit.is_some_and(|n|total>n){return Err("Prepared-index entry admission denied".into());}self.mesh.reserve_spatial_append(&d.spatial).map_err(str::to_owned)?;
  let vertices=copy_vertices(&d.vertices)?;let edges=copy_append_edges(&d.edges)?;let mut additions=HashMap::<usize,Vec<usize>>::new();additions.try_reserve(edges.len().checked_mul(2).ok_or("Adjacency extent overflow")?).map_err(|_|"Redo adjacency map refused")?;for (offset,e) in edges.iter().enumerate(){for id in [e.v0,e.v1].into_iter().take(if e.v0==e.v1{1}else{2}){if id<d.base_vertices{let ids=additions.entry(id).or_default();ids.try_reserve(1).map_err(|_|"Redo prepared adjacency refused")?;ids.push(d.base_edges.checked_add(offset).ok_or("Redo edge extent overflow")?);}}}
  self.undo.try_reserve(1).map_err(|_|"Undo history admission refused")?;self.mesh.vertices.try_reserve(vertices.len()).map_err(|_|"Redo vertex admission refused")?;self.mesh.edges.try_reserve(edges.len()).map_err(|_|"Redo edge admission refused")?;for (&id,ids) in &additions{self.mesh.vertices[id].edges.try_reserve(ids.len()).map_err(|_|"Redo adjacency admission refused")?;}
  let registry_change=self.registry.prepare_append(&vertices,&edges)?;
  let d=self.redo.pop().unwrap();self.mesh.vertices.extend(vertices);self.mesh.edges.extend(edges);for (id,ids) in additions{self.mesh.vertices[id].edges.extend(ids);}self.mesh.install_spatial_append(&d.spatial).expect("private prepared redo positions");self.state=d.after;self.version=next_version;registry_change.apply(d.after,next_version).expect("private prepared registry redo");self.redo_bytes-=d.bytes;self.undo_bytes+=d.bytes;self.undo.push(d);Ok(true)
 }
}
// Explicit conservative payload slots, not allocator internals or VM memory.
// Includes coordinates/index slots, adjacency/key slots and preparation copies.
fn sized(count:usize,width:usize)->Result<usize,String>{count.checked_mul(width).ok_or_else(||"Owner payload extent overflow".into())}
fn add(a:usize,b:usize)->Result<usize,String>{a.checked_add(b).ok_or_else(||"Owner payload extent overflow".into())}
fn table_slots(entries:usize,width:usize)->Result<usize,String>{if entries==0{return Ok(0);}let slots=entries.checked_next_power_of_two().and_then(|n|n.checked_mul(2)).ok_or("Table payload extent overflow")?.max(4);add(64,sized(slots,width.checked_add(1).ok_or("Table slot extent overflow")?)?)}
fn preparation_bound(vertices:usize,edges:usize)->Result<usize,String>{
 let coordinates=sized(vertices,2*std::mem::size_of::<DVec3>()+2*std::mem::size_of::<MeshVertex>())?;
 let index=table_slots(vertices,std::mem::size_of::<([i64;3],usize)>())?;
 let endpoint_count=edges.checked_mul(2).ok_or("Endpoint extent overflow")?;
 let adjacency=table_slots(endpoint_count,std::mem::size_of::<(usize,Vec<usize>)>())?;
 let touched=table_slots(endpoint_count,std::mem::size_of::<usize>())?;
 let records=sized(edges,2*std::mem::size_of::<MeshEdge>()+16*std::mem::size_of::<usize>()+1)?;
 add(256,add(add(coordinates,index)?,add(add(adjacency,touched)?,records)?)?)
}
fn staging_bound(base_vertices:usize,base_edges:usize,vertices:usize,edges:usize)->Result<usize,String>{
 let nv=base_vertices.checked_add(vertices).ok_or("Staging vertex extent overflow")?;let ne=base_edges.checked_add(edges).ok_or("Staging edge extent overflow")?;
 // Finite f64 rounded registry coordinates fit <=1024 bits. Charge twice
 // that limb payload per component plus table slots and coordinate vectors.
 let limb_bytes=2*(f64::MAX_EXP as usize/8);
 let keys=add(table_slots(nv,std::mem::size_of::<([num_bigint::BigInt;3],usize)>())?,sized(nv,3*limb_bytes+std::mem::size_of::<DVec3>())?)?;
 let links=add(table_slots(ne,std::mem::size_of::<((usize,usize),(usize,usize,usize))>())?,sized(edges,std::mem::size_of::<(usize,usize)>())?)?;
 add(3*limb_bytes,add(keys,links)?)
}
fn delta_bound(vertices:usize,edges:usize)->Result<usize,String>{add(std::mem::size_of::<Delta>(),add(sized(vertices,std::mem::size_of::<MeshVertex>())?,sized(edges,std::mem::size_of::<MeshEdge>()+2*std::mem::size_of::<usize>())?)?)}
fn positions(vertices:&[MeshVertex],extra:usize)->Result<Vec<DVec3>,String>{let mut out=Vec::new();out.try_reserve_exact(vertices.len().checked_add(extra).ok_or("Position extent overflow")?).map_err(|_|"Position preparation refused")?;out.extend(vertices.iter().map(|v|v.position));Ok(out)}
// The append adapter owns only fresh unpainted edges, so copying metadata-free
// records needs no hidden String/Value/HashMap clone allocations.
fn copy_append_edges(source:&[MeshEdge])->Result<Vec<MeshEdge>,String>{
 let mut out=Vec::new();out.try_reserve_exact(source.len()).map_err(|_|"Delta edge admission refused")?;
 for e in source{if !e.attrs.is_empty()||!e.faces.is_empty()||e.layer.is_some()||e.soft||e.curve_id.is_some()||e.hidden{return Err("Non-append edge metadata refused".into());}out.push(MeshEdge::new(e.v0,e.v1));}Ok(out)
}
fn copy_vertices(source:&[MeshVertex])->Result<Vec<MeshVertex>,String>{
 let mut out=Vec::new();out.try_reserve_exact(source.len()).map_err(|_|"Delta vertex admission refused")?;
 for v in source{let mut edges=Vec::new();edges.try_reserve_exact(v.edges.len()).map_err(|_|"Delta adjacency admission refused")?;edges.extend_from_slice(&v.edges);out.push(MeshVertex{uid:v.uid,position:v.position,edges});}Ok(out)
}
fn delta_bytes(vertices:&Vec<MeshVertex>,edges:&Vec<MeshEdge>)->Result<usize,String>{
 let mut bytes=std::mem::size_of::<Delta>();
 for (count,width) in [(vertices.capacity(),std::mem::size_of::<MeshVertex>()),(edges.capacity(),std::mem::size_of::<MeshEdge>())]{bytes=bytes.checked_add(count.checked_mul(width).ok_or("Delta byte extent overflow")?).ok_or("Delta byte extent overflow")?;}
 for v in vertices{bytes=bytes.checked_add(v.edges.capacity().checked_mul(std::mem::size_of::<usize>()).ok_or("Delta adjacency byte extent overflow")?).ok_or("Delta byte extent overflow")?;}Ok(bytes)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn spatial_delta_native_collisions_restore_exact_predecessor_and_reject_stale(){
 let mut m=Mesh::new();m.vertices.push(MeshVertex::new(DVec3::ZERO));m.vertices.push(MeshVertex::new(DVec3::ZERO));m.rebuild_spatial_index();let points=[DVec3::ZERO,DVec3::X,DVec3::ZERO];let patch=m.prepare_spatial_append(&points).unwrap();for p in points{m.vertices.push(MeshVertex::new(p));}m.install_spatial_append(&patch).unwrap();assert_eq!(m.add_vertex(DVec3::ZERO),4);
 m.restore_spatial_append(&patch).unwrap();m.vertices.truncate(2);assert_eq!(m.add_vertex(DVec3::ZERO),1);assert!(m.restore_spatial_append(&patch).is_err());m.reserve_spatial_append(&patch).unwrap();for p in points{m.vertices.push(MeshVertex::new(p));}m.install_spatial_append(&patch).unwrap();assert_eq!(m.add_vertex(DVec3::ZERO),4);
 let before=m.clone();m.vertices[4].position=DVec3::Y;assert!(m.restore_spatial_append(&patch).is_err());m.vertices[4].position=DVec3::ZERO;assert_eq!(m,before);assert!(m.prepare_spatial_append(&[DVec3::NAN]).is_err());assert_eq!(m,before);
 }
 #[test]fn persistent_registry_stale_before_lookup_and_failed_prepare_preserves_view(){
 let mut o=JournalOwner::new(Mesh::new()).unwrap();
 for action in 0..4{let mut held=o.try_begin().unwrap();match action{0=>{let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();},1=>{o.undo().unwrap();},2=>{o.redo().unwrap();},_=>{o.replace(Mesh::new()).unwrap();}}
 let calls=held.registry_lookup_calls();assert!(held.add_edge(DVec3::Y,DVec3::Z).unwrap_err().contains("Stale source registry"));assert_eq!(held.registry_lookup_calls(),calls);}
 let mut earlier=o.try_begin().unwrap();let mut candidate=o.try_begin().unwrap();candidate.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.set_index_entry_limit(Some(0));assert!(o.commit(candidate).is_err());assert_eq!(o.history_counts(),(0,0));earlier.add_edge(DVec3::Y,DVec3::Z).unwrap();o.set_index_entry_limit(None);assert!(o.commit(earlier).unwrap());
 }
 #[test]fn persistent_registry_duplicate_vertex_last_wins_after_undo(){
 let mut m=Mesh::new();m.vertices.push(MeshVertex::new(DVec3::ZERO));m.vertices.push(MeshVertex::new(DVec3::ZERO));let mut o=JournalOwner::new(m).unwrap();
 for endpoint in [DVec3::X,DVec3::Y]{let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,endpoint).unwrap();o.commit(t).unwrap();assert_eq!(o.mesh().edges[0].v0,1);o.undo().unwrap();assert_eq!(o.mesh().vertices.len(),2);}
 assert_eq!(o.registry_metrics().cold_vertex_rows,2);
 }
 #[test]fn persistent_registry_lifetime_stale_views_and_duplicate_base_edge_ids(){
  let mut mesh=Mesh::new();let a=mesh.add_vertex(DVec3::ZERO);let b=mesh.add_vertex(DVec3::X);mesh.add_edge(a,b);let mut duplicate=MeshEdge::new(a,b);duplicate.layer=Some("opaque".into());mesh.vertices[a].edges.push(1);mesh.vertices[b].edges.push(1);mesh.edges.push(duplicate);
  let mut o=JournalOwner::new(mesh).unwrap();let budget=o.budget();let mut t=o.try_begin().unwrap();t.add_edge(DVec3::Y,DVec3::Z).unwrap();o.commit(t).unwrap();assert_eq!(o.mesh().edges[2].v0,2);assert_eq!(o.mesh().edges[2].v1,3);o.undo().unwrap();o.redo().unwrap();let mut duplicate=o.try_begin().unwrap();duplicate.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(!o.commit(duplicate).unwrap());
  let held=o.try_begin().unwrap();let mut stale=o.try_begin().unwrap();let retained=budget.snapshot().registry;assert!(retained>0);o.replace(Mesh::new()).unwrap();assert_eq!(budget.snapshot().registry,retained);assert!(stale.add_edge(DVec3::ZERO,DVec3::X).unwrap_err().contains("Stale source registry"));assert!(o.commit(stale).is_err());drop(held);assert_eq!(budget.snapshot().registry,0);
 }
 #[test]fn persistent_registry_begin_no_base_scan_and_overlay_lookup_order(){
  let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);let mut o=JournalOwner::new(mesh).unwrap();let identity=o.registry_identity();let scans=o.registry_metrics().cold_vertex_rows;
  for _ in 0..100{let t=o.try_begin().unwrap();drop(t);}assert_eq!(o.registry_metrics().cold_vertex_rows,scans);assert_eq!(o.registry_identity(),identity);
  let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();t.add_edge(DVec3::X,DVec3::Y).unwrap();o.commit(t).unwrap();assert_eq!(o.mesh().vertices.len(),3);assert_eq!(o.mesh().edges[1].v0,1);assert_eq!(o.mesh().edges[1].v1,2);assert_eq!(o.registry_metrics().publication_rows,4);o.undo().unwrap();assert_eq!(o.mesh().vertices.len(),1);o.redo().unwrap();let mut repeat=o.try_begin().unwrap();repeat.add_edge(DVec3::X,DVec3::Y).unwrap();assert!(!o.commit(repeat).unwrap());assert_eq!(o.registry_identity(),identity);
 }

 #[test]fn shared_budget_two_owners_and_existing_redo_survive_refused_commit(){
  let budget=OwnerBudget::new(None);let mut a=JournalOwner::new_with_budget(Mesh::new(),budget.clone()).unwrap();let mut b=JournalOwner::new_with_budget(Mesh::new(),budget.clone()).unwrap();
  for o in [&mut a,&mut b]{let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();}
  assert_eq!(budget.snapshot().history,a.history_payload_bytes().0+b.history_payload_bytes().0);
  a.undo().unwrap();let old_a=a.mesh().clone();let old_b=b.mesh().clone();let history=budget.snapshot().history;assert_eq!(history,a.history_payload_bytes().1+b.history_payload_bytes().0);
  let mut candidate=a.try_begin().unwrap();candidate.add_edge(DVec3::Y,DVec3::Z).unwrap();let before=budget.snapshot();assert!(before.staging>0);budget.set_limit(Some(before.total()));
  assert!(a.commit(candidate).unwrap_err().contains("Shared owner payload admission"));assert_eq!(a.mesh(),&old_a);assert_eq!(b.mesh(),&old_b);assert_eq!(a.history_counts(),(0,1));assert_eq!(b.history_counts(),(1,0));assert_eq!((budget.snapshot().history,budget.snapshot().staging,budget.snapshot().preparation),(history,0,0));
  budget.set_limit(None);a.redo().unwrap();assert_eq!(budget.snapshot().history,history);drop(a);assert_eq!(budget.snapshot().history,b.history_payload_bytes().0);drop(b);assert_eq!(budget.snapshot().total(),0);
 }

 #[test]fn shared_budget_constructor_and_preparation_refusals_preserve_owner(){
  let budget=OwnerBudget::new(Some(0));assert!(JournalOwner::new_with_budget(Mesh::new(),budget.clone()).is_err());assert_eq!(budget.snapshot().total(),0);
  budget.set_limit(None);let mut o=JournalOwner::new_with_budget(Mesh::new(),budget.clone()).unwrap();let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();let saved=o.mesh().clone();let history=o.history_counts();let before=budget.snapshot();assert!(before.history>0);assert_eq!((before.staging,before.preparation),(0,0));
  let valid=o.try_begin().unwrap();budget.set_limit(Some(0));assert!(o.undo().is_err());assert_eq!(o.mesh(),&saved);assert_eq!(o.history_counts(),history);assert!(o.replace(Mesh::new()).is_err());assert_eq!(o.mesh(),&saved);assert_eq!(o.history_counts(),history);assert!(!o.commit(valid).unwrap());assert_eq!(budget.snapshot().history,before.history);assert_eq!(budget.snapshot().staging,0);
  budget.set_limit(None);o.undo().unwrap();let undone=o.mesh().clone();let retained=budget.snapshot().history;budget.set_limit(Some(retained));assert!(o.redo().is_err());assert_eq!(o.mesh(),&undone);assert_eq!(o.history_counts(),(0,1));assert_eq!((budget.snapshot().staging,budget.snapshot().preparation),(0,0));budget.set_limit(None);o.redo().unwrap();assert_eq!(o.mesh(),&saved);o.replace(Mesh::new()).unwrap();assert_eq!(budget.snapshot().total(),0);
 }
 #[test]fn shared_budget_staging_and_commit_refusals_release_candidate_only(){
  let mut o=JournalOwner::new(Mesh::new()).unwrap();let budget=o.budget();let t=o.try_begin().unwrap();let first=budget.snapshot().staging;assert!(first>0);let second=o.try_begin().unwrap();assert_eq!(budget.snapshot().staging,first*2);drop(second);assert_eq!(budget.snapshot().staging,first);drop(t);assert_eq!(budget.snapshot().total(),0);
  let mut t=o.try_begin().unwrap();budget.set_limit(Some(budget.snapshot().total()));assert!(t.add_edge(DVec3::ZERO,DVec3::X).is_err());assert!(o.commit(t).is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(budget.snapshot().total(),0);
  budget.set_limit(None);let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();budget.set_limit(Some(budget.snapshot().total()));assert!(o.commit(t).is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,0));assert_eq!(budget.snapshot().total(),0);budget.set_limit(None);let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(o.commit(t).unwrap());assert_eq!(budget.snapshot().history,o.history_payload_bytes().0);drop(o);assert_eq!(budget.snapshot().total(),0);
 }

 #[test]fn caught_staging_refusal_cannot_publish_partial_vertices(){let mut o=JournalOwner::new(Mesh::new()).unwrap();o.set_append_limits(Some(1),None);let mut t=o.try_begin().unwrap();assert!(t.add_edge(DVec3::ZERO,DVec3::X).is_err());assert!(t.add_edge(DVec3::Y,DVec3::Z).unwrap_err().contains("previously refused"));assert!(o.commit(t).unwrap_err().contains("staging admission failed"));assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,0));o.set_append_limits(None,Some(0));let mut t=o.try_begin().unwrap();assert!(t.add_edge(DVec3::ZERO,DVec3::X).is_err());assert!(o.commit(t).is_err());assert!(o.mesh().vertices.is_empty());o.set_append_limits(None,None);let mut t=o.try_begin().unwrap();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();assert_eq!(o.mesh().edges.len(),1);o.set_append_limits(Some(0),Some(0));let mut duplicate=o.try_begin().unwrap();duplicate.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(!o.commit(duplicate).unwrap());assert_eq!(o.history_counts(),(1,0));}

 #[test]fn history_admission_preserves_redo_and_retry_without_payload_growth(){
  let mut o=JournalOwner::new(Mesh::new()).unwrap();let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();
  let first=o.history_payload_bytes();assert!(first.0>0);assert_eq!(first.1,0);let saved=o.mesh().clone();
  o.undo().unwrap();assert_eq!(o.history_payload_bytes(),(0,first.0));o.set_history_byte_limit(Some(0));
  let mut branch=o.begin();branch.add_edge(DVec3::Y,DVec3::Z).unwrap();assert!(o.commit(branch).unwrap_err().contains("payload admission"));
  assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,1));assert_eq!(o.history_payload_bytes(),(0,first.0));
  // Existing history moves between stacks without requiring a new admission.
  o.redo().unwrap();assert_eq!(o.mesh(),&saved);assert_eq!(o.history_payload_bytes(),first);
  o.set_history_byte_limit(Some(first.0));let mut t=o.begin();t.add_edge(DVec3::Y,DVec3::Z).unwrap();assert!(o.commit(t).is_err());assert_eq!(o.mesh(),&saved);assert_eq!(o.history_payload_bytes(),first);
  o.set_history_byte_limit(None);let mut t=o.begin();t.add_edge(DVec3::Y,DVec3::Z).unwrap();o.commit(t).unwrap();assert!(o.history_payload_bytes().0>first.0);
  o.replace(Mesh::new()).unwrap();assert_eq!(o.history_payload_bytes(),(0,0));
 }

 #[test]fn quota_failure_preserves_mesh_history_and_retry(){let mut o=JournalOwner::new(Mesh::new()).unwrap();o.set_index_entry_limit(Some(1));let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(o.commit(t).is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,0));o.set_index_entry_limit(None);let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();assert!(o.commit(t).unwrap());let old=o.mesh().clone();o.undo().unwrap();o.set_index_entry_limit(Some(1));assert!(o.redo().is_err());assert!(o.mesh().vertices.is_empty());assert_eq!(o.history_counts(),(0,1));o.set_index_entry_limit(None);o.redo().unwrap();assert_eq!(o.mesh(),&old);}
 #[test]fn failed_undo_and_replacement_keep_published_state(){let mut m=Mesh::new();for p in [DVec3::ZERO,DVec3::X,DVec3::Y]{m.add_vertex(p);}let mut o=JournalOwner::new(m).unwrap();let mut t=o.begin();t.add_edge(DVec3::Y,DVec3::Z).unwrap();o.commit(t).unwrap();let before=o.mesh().clone();o.set_index_entry_limit(Some(1));assert!(o.undo().is_err());assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),(1,0));assert!(o.replace(before.clone()).is_err());assert_eq!(o.mesh(),&before);assert_eq!(o.history_counts(),(1,0));}
 #[test]fn prepared_index_refuses_stale_positions_without_lookup_change(){let mut mesh=Mesh::new();mesh.add_vertex(DVec3::ZERO);let index=Mesh::prepare_spatial_index(&[DVec3::X]).unwrap();assert!(mesh.install_prepared_spatial_index(index).is_err());assert_eq!(mesh.add_vertex(DVec3::ZERO),0);assert_eq!(mesh.vertices.len(),1);}
 #[test]fn stale_transaction_after_replace_undo_redo_refuses(){let mut o=JournalOwner::new(Mesh::new()).unwrap();let mut stale=o.begin();stale.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.replace(Mesh::new()).unwrap();assert!(o.commit(stale).is_err());let mut t=o.begin();t.add_edge(DVec3::ZERO,DVec3::X).unwrap();o.commit(t).unwrap();let stale=o.begin();o.undo().unwrap();assert!(o.commit(stale).is_err());let stale=o.begin();o.redo().unwrap();assert!(o.commit(stale).is_err());assert_eq!(o.history_counts(),(1,0));}
}

/// Existing-field publication prerequisite. A raw Face.loop edit does not heal
/// radial records and a raw QVector edit does not re-key the source registry.
/// Command history is owned by the future VM controller; this ticket does not
/// impersonate append Undo and makes prior native append tokens stale.
pub struct PreparedFields<'a>{owner:&'a mut JournalOwner,positions:Vec<DVec3>,loops:Vec<(Vec<usize>,Vec<Vec<usize>>)>,spatial:PreparedSpatialIndex,registry:crate::owner_registry::PositionChange,after:Uuid,next:u64,_charge:Lease}
impl JournalOwner {
 pub fn prepare_fields(&mut self,positions:Vec<DVec3>,loops:Vec<(Vec<usize>,Vec<Vec<usize>>)>)->Result<PreparedFields<'_>,String>{
  if positions.len()!=self.mesh.vertices.len()||loops.len()!=self.mesh.faces.len(){return Err("Raw field projection requires unchanged membership".into());}
  let next=self.version.checked_add(1).ok_or("Owner revision overflow")?;
  for(outer,holes)in &loops{for &id in outer.iter().chain(holes.iter().flatten()){if id>=positions.len(){return Err("Raw field projection vertex identity outside membership".into());}}}
  let charge=self.budget.reserve(Kind::Preparation,preparation_bound(positions.len(),0)?)?;
  let spatial=self.index(&positions)?;
  let registry=self.registry.prepare_positions(&positions)?;
  Ok(PreparedFields{owner:self,positions,loops,spatial,registry,after:Uuid::new_v4(),next,_charge:charge})
 }
}
impl PreparedFields<'_>{pub fn apply(self){let Self{owner,positions,loops,spatial,registry,after,next,_charge}=self;for(v,p)in owner.mesh.vertices.iter_mut().zip(positions){v.position=p;}for(f,(outer,holes))in owner.mesh.faces.iter_mut().zip(loops){f.vertices=outer;f.holes=holes;}owner.mesh.install_prepared_spatial_index(spatial).expect("Exclusive prepared raw field positions");registry.apply(after,next);owner.state=after;owner.version=next;}}
