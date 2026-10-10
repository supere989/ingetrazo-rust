//! Cold resize preparation controls; no fake Vulkan return values.
use std::{collections::BTreeMap,sync::{Arc,Mutex}};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Stage {StartupCommand,StartupSwapchain,StartupSync,StartupRenderer,BeforeCreate,ImageView(usize),RenderPass,Framebuffer(usize),EguiPipeline,CadPipeline,Sync,BeforeCommit}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,serde::Serialize)]
pub enum Kind {CommandBuffer,AcquireSemaphore,GraphicsFence,Swapchain,ImageView,RenderPass,Framebuffer,EguiPipeline,CadPipeline,CadPool,Semaphore,PresentFence}
#[derive(Clone,Default,Debug,PartialEq,Eq,serde::Serialize)]
pub struct Audit {pub created:BTreeMap<Kind,u64>,pub destroyed:BTreeMap<Kind,u64>}
#[derive(Clone,Default)]
pub(crate) struct Control {pub cancel:Option<Stage>,pub audit:Option<Arc<Mutex<Audit>>>}
impl Control {
 pub fn checkpoint(&self,stage:Stage)->Result<(),String>{if self.cancel==Some(stage){Err(format!("Engineering resize cancellation at {stage:?}; actual unpublished allocations discarded"))}else{Ok(())}}
 pub fn created(&self,kind:Kind,count:u64){if count>0{if let Some(audit)=&self.audit{*audit.lock().unwrap().created.entry(kind).or_default()+=count;}}}
 pub fn destroyed(&self,kind:Kind,count:u64){if count>0{if let Some(audit)=&self.audit{*audit.lock().unwrap().destroyed.entry(kind).or_default()+=count;}}}
}
pub(crate) struct Tracked<T>{pub value:Option<T>,control:Control,kinds:Vec<(Kind,u64)>}
impl<T> Tracked<T>{
 pub fn new(value:T,control:&Control,kinds:Vec<(Kind,u64)>)->Self{for(k,n)in &kinds{control.created(*k,*n);}Self{value:Some(value),control:control.clone(),kinds}}
 pub fn take(&mut self)->T{self.value.take().unwrap()}
}
impl<T> Drop for Tracked<T>{fn drop(&mut self){if let Some(value)=self.value.take(){drop(value);for(k,n)in &self.kinds{self.control.destroyed(*k,*n);}}}}

#[cfg(test)] mod tests {
 use super::*;
 struct Owner(Arc<Mutex<u64>>);
 impl Drop for Owner{fn drop(&mut self){*self.0.lock().unwrap()+=1;}}
 #[test] fn unused_owner_is_destroyed_and_published_owner_is_transferred(){
  let audit=Arc::new(Mutex::new(Audit::default()));let drops=Arc::new(Mutex::new(0));
  let control=Control{audit:Some(audit.clone()),cancel:None};
  {let _unused=Tracked::new(Owner(drops.clone()),&control,vec![(Kind::EguiPipeline,1)]);}
  assert_eq!(*drops.lock().unwrap(),1);{let a=audit.lock().unwrap();assert_eq!(a.created,a.destroyed);}
  let published={let mut candidate=Tracked::new(Owner(drops.clone()),&control,vec![(Kind::CadPipeline,1)]);candidate.take()};
  assert_eq!(*drops.lock().unwrap(),1);assert_eq!(audit.lock().unwrap().destroyed.get(&Kind::CadPipeline),None);
  drop(published);assert_eq!(*drops.lock().unwrap(),2);
 }
 #[test] fn cancellation_is_explicit_and_stage_specific(){
  let control=Control{cancel:Some(Stage::Framebuffer(1)),audit:None};
  assert!(control.checkpoint(Stage::RenderPass).is_ok());
  assert!(control.checkpoint(Stage::Framebuffer(1)).unwrap_err().starts_with("Engineering resize cancellation"));
 }
}

/// Driver result is retained until admission policy is decided, never string-matched.
#[derive(Debug)] pub(crate) enum PrepareError{Vulkan(ash::vk::Result),Other(String)}
impl PrepareError{pub fn device_lost(&self)->bool{matches!(self,Self::Vulkan(ash::vk::Result::ERROR_DEVICE_LOST))}}
impl From<ash::vk::Result> for PrepareError{fn from(value:ash::vk::Result)->Self{Self::Vulkan(value)}}
impl From<&str> for PrepareError{fn from(value:&str)->Self{Self::Other(value.into())}}
impl From<String> for PrepareError{fn from(value:String)->Self{Self::Other(value)}}
impl std::fmt::Display for PrepareError{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{match self{Self::Vulkan(v)=>write!(f,"{v}"),Self::Other(v)=>write!(f,"{v}")}}}
impl std::error::Error for PrepareError{}
#[cfg(test)]mod error_tests{
 use super::*;
 #[test]fn typed_loss_refuses_but_memory_pressure_and_opaque_text_do_not(){
  assert!(PrepareError::from(ash::vk::Result::ERROR_DEVICE_LOST).device_lost());
  for error in [PrepareError::from(ash::vk::Result::ERROR_OUT_OF_DEVICE_MEMORY),PrepareError::from(ash::vk::Result::ERROR_OUT_OF_HOST_MEMORY),PrepareError::Other("ERROR_DEVICE_LOST".into())]{assert!(!error.device_lost());}
 }
}
