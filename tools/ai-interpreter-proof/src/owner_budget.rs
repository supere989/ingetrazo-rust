//! Shared explicit owner payload admission; excludes VM/live application mesh.
use std::{cell::RefCell,rc::Rc};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Kind{History=0,Staging=1,Preparation=2,Registry=3}
#[derive(Clone,Debug,PartialEq,Eq)]pub struct Snapshot{pub history:usize,pub staging:usize,pub preparation:usize,pub registry:usize,pub peak:usize,pub limit:Option<usize>}
impl Snapshot{pub fn total(&self)->usize{self.history+self.staging+self.preparation+self.registry}}
#[derive(Clone)]pub struct OwnerBudget(Rc<RefCell<State>>);
struct State{bytes:[usize;4],peak:usize,limit:Option<usize>}
pub struct Lease{budget:OwnerBudget,kind:Kind,bytes:usize}
impl OwnerBudget{
 pub fn new(limit:Option<usize>)->Self{Self(Rc::new(RefCell::new(State{bytes:[0;4],peak:0,limit})))}
 pub fn set_limit(&self,limit:Option<usize>){self.0.borrow_mut().limit=limit;}
 pub fn snapshot(&self)->Snapshot{let s=self.0.borrow();Snapshot{history:s.bytes[0],staging:s.bytes[1],preparation:s.bytes[2],registry:s.bytes[3],peak:s.peak,limit:s.limit}}
 pub fn reserve(&self,kind:Kind,bytes:usize)->Result<Lease,String>{let mut lease=Lease{budget:self.clone(),kind,bytes:0};lease.resize(bytes)?;Ok(lease)}
}
impl Lease{
 /// Transfers accounting category without changing total admitted bytes.
 pub fn reclassify(&mut self,kind:Kind)->Result<(),String>{if self.kind==kind{return Ok(());}let mut s=self.budget.0.borrow_mut();let next=s.bytes[kind as usize].checked_add(self.bytes).ok_or("Lease category overflow")?;s.bytes[self.kind as usize]-=self.bytes;s.bytes[kind as usize]=next;self.kind=kind;Ok(())}
 pub fn bytes(&self)->usize{self.bytes}
 pub fn merge(&mut self,mut other:Lease)->Result<(),String>{if self.kind!=other.kind||!Rc::ptr_eq(&self.budget.0,&other.budget.0){return Err("Foreign budget lease".into());}self.bytes=self.bytes.checked_add(other.bytes).ok_or("Lease extent overflow")?;other.bytes=0;Ok(())}
 pub fn resize(&mut self,bytes:usize)->Result<(),String>{let mut s=self.budget.0.borrow_mut();let total=s.bytes.iter().try_fold(0usize,|n,&x|n.checked_add(x)).ok_or("Owner budget extent overflow")?;if bytes>self.bytes{let extra=bytes-self.bytes;let next=total.checked_add(extra).ok_or("Owner budget extent overflow")?;if s.limit.is_some_and(|limit|next>limit){return Err("Shared owner payload admission denied".into());}s.bytes[self.kind as usize]+=extra;s.peak=s.peak.max(next);}else{s.bytes[self.kind as usize]-=self.bytes-bytes;}self.bytes=bytes;Ok(())}
}
impl Drop for Lease{fn drop(&mut self){self.budget.0.borrow_mut().bytes[self.kind as usize]-=self.bytes;}}

impl std::fmt::Debug for Lease{fn fmt(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.debug_struct("Lease").field("kind",&self.kind).field("bytes",&self.bytes).finish()}}
