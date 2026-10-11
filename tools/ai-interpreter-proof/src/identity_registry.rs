//! Private source-key -> genuine Vertex-reference authority prerequisite.
//! Public membership and native packed row numbers never enter this map.
//! This owner is not yet installed in editable Mesh; production remains disabled.
use crate::{journal::SourceKey,owner_budget::{Kind,Lease,OwnerBudget},vm_mesh::editable_mesh::Vertex};
use rustpython_vm::{PyRef,AsObject,Interpreter};
use std::rc::Rc;
pub(super) struct Runtime{interpreter:Rc<Interpreter>}
impl Runtime{pub(super) fn new(interpreter:Rc<Interpreter>)->Rc<Self>{Rc::new(Self{interpreter})}}
impl Drop for Runtime{fn drop(&mut self){self.interpreter.enter(|vm|{if vm.host_budget_poisoned(){vm.state.gc.collect_force(2);}});}}
/// The VM outlives every exported real Vertex handle. Native reference drops
/// precede final runtime collection; no per-handle collection or scope clearing.
#[derive(Clone)]pub struct VertexHandle{vertex:PyRef<Vertex>,runtime:Rc<Runtime>}
impl VertexHandle{pub(super) fn new(vertex:PyRef<Vertex>,runtime:Rc<Runtime>)->Self{Self{vertex,runtime}}pub fn same(&self,other:&Self)->bool{self.vertex.is(&other.vertex)}}
use std::collections::HashMap;
use glam::DVec3;
pub struct Registry { values:HashMap<SourceKey,VertexHandle>, revision:u64, identity:uuid::Uuid, budget:OwnerBudget, charge:Lease, runtime:Rc<Runtime> }
pub struct Rejected { pub reason:String, pub candidate:Replacement }
impl std::fmt::Debug for Rejected{fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.debug_struct("Rejected").field("reason",&self.reason).finish_non_exhaustive()}}
pub struct Replacement { values:HashMap<SourceKey,VertexHandle>, revision:u64, identity:uuid::Uuid, charge:Lease, runtime:Rc<Runtime> }
fn bytes(rows:usize)->Result<usize,String>{rows.checked_mul(std::mem::size_of::<(SourceKey,VertexHandle)>()+3*std::mem::size_of::<usize>()).ok_or_else(||"Identity registry extent overflow".into())}
impl Registry {
 pub(super) fn new(budget:OwnerBudget,runtime:Rc<Runtime>)->Result<Self,String>{let charge=budget.reserve(Kind::Registry,0)?;Ok(Self{values:HashMap::new(),revision:0,identity:uuid::Uuid::new_v4(),budget,charge,runtime})}
 pub fn slot(&self,position:DVec3)->Result<Option<VertexHandle>,String>{if !position.is_finite()||!(position*10000.).is_finite(){return Err("Identity registry nonfinite key".into());}Ok(self.values.get(&SourceKey::new(position)).cloned())}
 pub fn len(&self)->usize{self.values.len()}
 pub fn revision(&self)->u64{self.revision}
 /// Prepare a complete source-owned registry view before publication. Values
 /// are actual held object references; duplicate keys use source last-wins.
 /// BigInt payload clone and VM heap allocation are not allocator-bounded here.
 pub fn prepare(&self,rows:&[(DVec3,VertexHandle)])->Result<Replacement,String>{
  for(position,vertex)in rows{if !Rc::ptr_eq(&self.runtime,&vertex.runtime){return Err("Foreign Vertex runtime".into());}if !position.is_finite()||!(*position*10000.).is_finite(){return Err("Identity registry nonfinite key".into());}}
  let revision=self.revision.checked_add(1).ok_or("Identity registry revision overflow")?;
  let mut charge=self.budget.reserve(Kind::Preparation,bytes(rows.len())?)?;
  let mut values=HashMap::new();values.try_reserve(rows.len()).map_err(|_|"Identity registry map reservation refused")?;
  charge.resize(bytes(values.capacity())?)?;
  for (position,vertex)in rows {values.insert(SourceKey::new(*position),vertex.clone());}
  Ok(Replacement{values,revision,identity:self.identity,charge,runtime:self.runtime.clone()})
 }
 /// Replace only after all companion graph preparations succeed. Previous AND rejected
 /// references are returned to the caller for drop OUTSIDE native/VM locks.
 pub fn publish(&mut self,mut replacement:Replacement)->Result<Replacement,Rejected>{
  let reject=|reason:&str,candidate|Rejected{reason:reason.to_owned(),candidate};
  if replacement.identity!=self.identity{return Err(reject("Foreign identity registry replacement",replacement));}
  let Some(next)=self.revision.checked_add(1)else{return Err(reject("Identity registry revision overflow",replacement));};
  if replacement.revision!=next{return Err(reject("Stale identity registry replacement",replacement));}
  if let Err(error)=replacement.charge.reclassify(Kind::Registry){return Err(reject(&error,replacement));}
  std::mem::swap(&mut self.values,&mut replacement.values);
  std::mem::swap(&mut self.charge,&mut replacement.charge);
  let old=self.revision;self.revision=replacement.revision;replacement.revision=old;
  Ok(replacement)
 }
 pub fn contains_identity(&self,vertex:&VertexHandle)->bool{self.values.values().any(|v|v.same(vertex))}
}
