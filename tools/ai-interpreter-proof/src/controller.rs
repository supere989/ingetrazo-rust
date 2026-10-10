//! Standalone opaque VM + document controller; no production script tool.
use crate::{owner::{JournalOwner,Transaction},session};
use rustpython_vm::{Interpreter,InterpreterBuilder,scope::Scope,compiler::Mode,pymodule};
use ingetrazo_core::mesh::Mesh;use glam::DVec3;use std::cell::RefCell;
struct StageGuard;
impl Drop for StageGuard{fn drop(&mut self){STAGED.with(|slot|{drop(slot.borrow_mut().take());});}}
thread_local!{static STAGED:RefCell<Option<Transaction>>=const{RefCell::new(None)};}
#[pymodule]mod native_owner{
 use rustpython_vm::{PyResult,VirtualMachine};
 #[cfg(test)] #[pyfunction] fn host_panic(){panic!("engineering native callback unwind");}
 #[pyfunction]fn edge(ax:f64,ay:f64,az:f64,bx:f64,by:f64,bz:f64,vm:&VirtualMachine)->PyResult<()>{vm.host_budget_tick()?;super::STAGED.with(|slot|{let mut slot=slot.borrow_mut();let t=slot.as_mut().ok_or_else(||vm.new_runtime_error("No active transaction owner"))?;t.add_edge(super::DVec3::new(ax,ay,az),super::DVec3::new(bx,by,bz)).map_err(|e|vm.new_value_error(e))})}
}
/// Raw Python exception retained without invoking script-defined formatting.
pub enum Failure { Python(rustpython_vm::builtins::PyBaseExceptionRef), Preparation(String) }
pub struct Execution{pub failure:Option<Failure>,pub changed:bool,pub error:bool,pub poisoned:bool,pub finished_before_reset:bool,pub events:Vec<&'static str>}
pub struct Controller{interpreter:Interpreter,scope:Scope,owner:JournalOwner,unwind_tainted:bool}
impl Controller{
 pub fn new(mesh:Mesh)->Result<Self,String>{let owner=JournalOwner::new(mesh)?;let builder=InterpreterBuilder::new();let module=native_owner::module_def(&builder.ctx);let interpreter=builder.add_native_module(module).interpreter();let scope=interpreter.enter(|vm|{let scope=vm.new_scope_with_builtins();vm.run_code_obj(vm.compile("import native_owner",Mode::Exec,"<setup>").unwrap(),scope.clone()).unwrap();scope});Ok(Self{interpreter,scope,owner,unwind_tainted:false})}
 pub fn mesh(&self)->&Mesh{self.owner.mesh()}
 pub fn history_counts(&self)->(usize,usize){self.owner.history_counts()}
 pub fn set_index_entry_limit(&mut self,limit:Option<usize>){self.owner.set_index_entry_limit(limit);}
 /// Interpreter, VM, scope and staged transaction are private. No public reset
 /// or native module registration API lets controller clients reset early.
 pub fn execute(&mut self,source:&str,fuel:u64)->Result<Execution,String>{
  if self.unwind_tainted{return Err("Controller execution did not finish; VM reuse refused".into());}
  if STAGED.with(|s|s.borrow().is_some()){return Err("An owner execution is already active".into());}
  let transaction=self.owner.begin();let owner=&mut self.owner;let scope=self.scope.clone();
  self.unwind_tainted=true;
  let result=self.interpreter.enter(|vm|{vm.host_budget_arm(fuel).map_err(str::to_owned)?;STAGED.with(|s|*s.borrow_mut()=Some(transaction));let _discard_on_unwind=StageGuard;let mut events=vec!["armed","staged"];
   let executed=session::execute(vm,&scope,source);let poisoned=vm.host_budget_poisoned();let transaction=STAGED.with(|s|s.borrow_mut().take()).expect("private staged execution owner");
   let (changed,failure)=match if poisoned{executed.and_then(|_|Err(vm.new_runtime_error("Poisoned execution cannot publish")))}else{executed}{Err(exception)=>{drop(transaction);events.push("discarded");(false,Some(Failure::Python(exception)))},Ok(())=>{match owner.commit(transaction){Ok(changed)=>{events.push("committed");(changed,None)},Err(reason)=>{events.push("discarded");(false,Some(Failure::Preparation(reason)))}}}};
   assert!(STAGED.with(|s|s.borrow().is_none()));vm.host_budget_reset_after_discard();events.push("reset");Ok(Execution{changed,error:failure.is_some(),failure,poisoned,finished_before_reset:true,events})
  });
  if result.is_ok(){self.unwind_tainted=false;}
  result
 }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn callback_unwind_discards_staged_owner_and_refuses_vm_reuse(){
  let mut c=Controller::new(Mesh::new()).unwrap();
  let caught=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||c.execute("native_owner.edge(0.,0.,0.,1.,0.,0.)\nnative_owner.host_panic()",2000)));
  assert!(caught.is_err());assert!(STAGED.with(|s|s.borrow().is_none()));
  assert!(c.mesh().vertices.is_empty());assert_eq!(c.history_counts(),(0,0));
  assert!(matches!(c.execute("pass",2000),Err(reason) if reason.contains("reuse refused")));
  let mut next=Controller::new(Mesh::new()).unwrap();assert!(!next.execute("pass",2000).unwrap().error);
 }
}
