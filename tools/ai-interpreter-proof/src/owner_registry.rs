//! Persistent private source registry. No public mutable state escapes.
use crate::{journal::SourceKey,owner_budget::{OwnerBudget,Lease,Kind}};
use ingetrazo_core::mesh::{Mesh,MeshVertex,MeshEdge};
use glam::DVec3;
use uuid::Uuid;
use std::{cell::RefCell,collections::HashMap,rc::Rc};
#[derive(Clone,Copy,Debug)]pub struct Metrics{pub cold_vertex_rows:usize,pub cold_edge_rows:usize,pub lookup_calls:usize,pub publication_rows:usize}
struct State{document:Uuid,state:Uuid,version:u64,valid:bool,positions:Vec<DVec3>,edge_extent:usize,vertices:HashMap<SourceKey,usize>,edges:HashMap<(usize,usize),(usize,usize,usize)>,charge:Lease,budget:OwnerBudget,metrics:Metrics}
pub struct Registry(Rc<RefCell<State>>);
#[derive(Clone)]pub struct View{inner:Rc<RefCell<State>>,document:Uuid,state:Uuid,version:u64}
pub struct Change{inner:Rc<RefCell<State>>,document:Uuid,state:Uuid,version:u64,vertex_extent:usize,edge_extent:usize,vertices:Vec<(SourceKey,usize,DVec3)>,edges:Vec<((usize,usize),(usize,usize,usize))>,grow:Lease,bytes:usize,truncate:bool}
impl Registry{
 pub fn new(mesh:&Mesh,budget:OwnerBudget,document:Uuid,state:Uuid,version:u64)->Result<Self,String>{
  let charge=budget.reserve(Kind::Registry,registry_bytes(mesh.vertices.len(),mesh.edges.len())?)?;
  let mut positions=Vec::new();positions.try_reserve_exact(mesh.vertices.len()).map_err(|_|"Registry positions refused")?;
  let mut vertices=HashMap::new();vertices.try_reserve(mesh.vertices.len()).map_err(|_|"Registry vertices refused")?;
  for (id,v) in mesh.vertices.iter().enumerate(){positions.push(v.position);if v.position.is_finite()&&(v.position*10000.).is_finite(){vertices.insert(SourceKey::new(v.position),id);}}
  let mut edges=HashMap::new();edges.try_reserve(mesh.edges.len()).map_err(|_|"Registry edges refused")?;
  for (id,e) in mesh.edges.iter().enumerate(){edges.insert(ordered(e.v0,e.v1),(id,e.v0,e.v1));}
  Ok(Self(Rc::new(RefCell::new(State{document,state,version,valid:true,positions,edge_extent:mesh.edges.len(),vertices,edges,charge,budget,metrics:Metrics{cold_vertex_rows:mesh.vertices.len(),cold_edge_rows:mesh.edges.len(),lookup_calls:0,publication_rows:0}}))))
 }
 pub fn view(&self)->View{let s=self.0.borrow();View{inner:self.0.clone(),document:s.document,state:s.state,version:s.version}}
 pub fn invalidate(&self){self.0.borrow_mut().valid=false;}
 pub fn metrics(&self)->Metrics{self.0.borrow().metrics}
 pub fn identity(&self)->usize{Rc::as_ptr(&self.0)as usize}
 pub fn prepare_append(&self,vertices:&[MeshVertex],edges:&[MeshEdge])->Result<Change,String>{
  let mut s=self.0.borrow_mut();let nv=s.positions.len().checked_add(vertices.len()).ok_or("Registry vertex extent overflow")?;let base_edges=s.edge_extent;
  let ne=base_edges.checked_add(edges.len()).ok_or("Registry edge extent overflow")?;
  let bytes=registry_bytes(nv,ne)?.max(s.charge.bytes());let grow=s.budget.reserve(Kind::Registry,bytes.saturating_sub(s.charge.bytes()))?;
  let mut v=Vec::new();v.try_reserve_exact(vertices.len()).map_err(|_|"Registry pending vertices refused")?;
  for (i,x) in vertices.iter().enumerate(){v.push((SourceKey::new(x.position),s.positions.len()+i,x.position));}
  let mut e=Vec::new();e.try_reserve_exact(edges.len()).map_err(|_|"Registry pending edges refused")?;
  for (i,x) in edges.iter().enumerate(){e.push((ordered(x.v0,x.v1),(base_edges+i,x.v0,x.v1)));}
  s.positions.try_reserve_exact(vertices.len()).map_err(|_|"Registry position publication refused")?;s.vertices.try_reserve(v.len()).map_err(|_|"Registry vertex publication refused")?;s.edges.try_reserve(e.len()).map_err(|_|"Registry edge publication refused")?;
  Ok(Change{inner:self.0.clone(),document:s.document,state:s.state,version:s.version,vertex_extent:nv,edge_extent:ne,vertices:v,edges:e,grow,bytes,truncate:false})
 }
 pub fn prepare_truncate(&self,vertex_extent:usize,edge_extent:usize,vertices:&[MeshVertex],edges:&[MeshEdge])->Result<Change,String>{
  let s=self.0.borrow();let bytes=s.charge.bytes();let grow=s.budget.reserve(Kind::Registry,0)?;
  let mut v=Vec::new();v.try_reserve_exact(vertices.len()).map_err(|_|"Registry removed vertices refused")?;
  for (i,x) in vertices.iter().enumerate(){v.push((SourceKey::new(x.position),vertex_extent+i,x.position));}
  let mut e=Vec::new();e.try_reserve_exact(edges.len()).map_err(|_|"Registry removed edges refused")?;
  for (i,x) in edges.iter().enumerate(){e.push((ordered(x.v0,x.v1),(edge_extent+i,x.v0,x.v1)));}
  if edge_extent.checked_add(edges.len())!=Some(s.edge_extent){return Err("Registry edge truncate extent mismatch".into());}
  if vertex_extent.checked_add(vertices.len())!=Some(s.positions.len()){return Err("Registry truncate extent mismatch".into());}
  for (key,id,_) in &v{if s.vertices.get(key)!=Some(id){return Err("Registry removed vertex identity mismatch".into());}}
  for (key,value) in &e{if s.edges.get(key)!=Some(value){return Err("Registry removed edge identity mismatch".into());}}
  Ok(Change{inner:self.0.clone(),document:s.document,state:s.state,version:s.version,vertex_extent,edge_extent,vertices:v,edges:e,grow,bytes,truncate:true})
 }
}
impl View{pub fn lookup_calls(&self)->usize{self.inner.borrow().metrics.lookup_calls}
 pub fn check(&self)->Result<(),String>{let s=self.inner.borrow();if !s.valid||s.document!=self.document||s.state!=self.state||s.version!=self.version{return Err("Stale source registry view".into());}Ok(())}
 pub fn extents(&self)->(usize,usize){let s=self.inner.borrow();(s.positions.len(),s.edge_extent)}
 pub fn vertex(&self,key:&SourceKey)->Option<usize>{let mut s=self.inner.borrow_mut();s.metrics.lookup_calls+=1;s.vertices.get(key).copied()}
 pub fn edge(&self,key:&(usize,usize))->Option<(usize,usize,usize)>{let mut s=self.inner.borrow_mut();s.metrics.lookup_calls+=1;s.edges.get(key).copied()}
 pub fn position(&self,id:usize)->DVec3{self.inner.borrow().positions[id]}
}
impl Change{
 pub fn apply(self,state:Uuid,version:u64)->Result<(),String>{
  let mut s=self.inner.borrow_mut();if !s.valid||s.document!=self.document||s.state!=self.state||s.version!=self.version{return Err("Stale prepared registry change".into());}
  let touched=self.vertices.len().checked_add(self.edges.len()).ok_or("Registry metrics extent overflow")?;
  if self.truncate{for (key,_,_) in self.vertices{s.vertices.remove(&key);}for (key,_) in self.edges{s.edges.remove(&key);}s.positions.truncate(self.vertex_extent);}
  else{for (key,id,p) in self.vertices{s.vertices.insert(key,id);s.positions.push(p);}for (key,value) in self.edges{s.edges.insert(key,value);}}
  s.charge.merge(self.grow).expect("private same-ledger registry lease");s.charge.resize(self.bytes).expect("already reserved registry payload");s.edge_extent=self.edge_extent;s.state=state;s.version=version;s.metrics.publication_rows=s.metrics.publication_rows.saturating_add(touched);Ok(())
 }
}
fn registry_bytes(vertices:usize,edges:usize)->Result<usize,String>{
 // Conservative logical typed slots, not physical allocator accounting.
 vertices.checked_mul(1024).and_then(|n|edges.checked_mul(256).and_then(|e|n.checked_add(e))).ok_or_else(||"Registry payload extent overflow".into())
}
fn ordered(a:usize,b:usize)->(usize,usize){if a<b{(a,b)}else{(b,a)}}
