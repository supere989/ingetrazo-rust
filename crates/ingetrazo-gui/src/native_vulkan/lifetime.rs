//! Bounded graphics-queue completion and shared destructive-cleanup gate.
//! A failed wait is not evidence of completion. Unknown GPU ownership remains
//! charged and retained until process termination; automatic recovery is not claimed.
use ash::{vk,Device};
use std::sync::atomic::{AtomicBool,Ordering};
pub const WAIT_NS:u64=5_000_000_000;
pub struct Lifetime {device:Device,queue:vk::Queue,uncertain:AtomicBool,retained:std::sync::Mutex<Vec<Box<dyn std::any::Any+Send+Sync>>>}
impl Lifetime {
 pub fn new(device:Device,queue:vk::Queue)->Self{Self{device,queue,uncertain:AtomicBool::new(false),retained:std::sync::Mutex::new(Vec::new())}}
 pub fn retain<T:std::any::Any+Send+Sync>(&self,value:T){self.retained.lock().unwrap_or_else(|p|p.into_inner()).push(Box::new(value));}
 pub fn retained_owners(&self)->usize{self.retained.lock().unwrap_or_else(|p|p.into_inner()).len()}
 pub fn uncertain(&self)->bool{self.uncertain.load(Ordering::Acquire)}
 pub fn quarantine(&self){self.uncertain.store(true,Ordering::Release);}
 pub fn check(&self)->Result<(),String>{if self.uncertain(){Err("GPU completion unknown; resources quarantined, device recreation required".into())}else{Ok(())}}
 pub fn wait_many(&self,fences:&[vk::Fence],timeout:u64)->Result<(),vk::Result>{
  if fences.is_empty(){return Ok(());}
  if self.uncertain(){return Err(vk::Result::ERROR_UNKNOWN);}
  let result=unsafe{self.device.wait_for_fences(fences,true,timeout.min(WAIT_NS))};
  if result.is_err(){self.quarantine();}result
 }
 pub fn wait(&self,fence:vk::Fence,timeout:u64)->Result<(),vk::Result>{
  if self.uncertain(){return Err(vk::Result::ERROR_UNKNOWN);}
  let result=unsafe{self.device.wait_for_fences(&[fence],true,timeout.min(WAIT_NS))};
  if result.is_err(){self.quarantine();}result
 }
 /// Fence after prior graphics submissions. Does not prove WSI presentation completion.
 pub fn drain_graphics(&self)->bool{
  if self.uncertain(){return false;}
  let fence=match unsafe{self.device.create_fence(&vk::FenceCreateInfo::default(),None)}{Ok(f)=>f,Err(_)=>{self.quarantine();return false;}};
  let submitted=unsafe{self.device.queue_submit(self.queue,&[],fence)};
  if submitted.is_err(){self.quarantine();return false;}
  if self.wait(fence,WAIT_NS).is_err(){return false;}
  unsafe{self.device.destroy_fence(fence,None)};true
 }
}
