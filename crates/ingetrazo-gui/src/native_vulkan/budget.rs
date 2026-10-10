use ash::{vk,Instance};
use ash::vk::Handle;
use std::sync::Mutex;
use crate::gpu_memory::MemoryBudget;

pub struct VulkanBudget {lifetime:Mutex<Option<std::sync::Arc<super::lifetime::Lifetime>>>,pub ledger:Mutex<MemoryBudget>,instance:Instance,physical:vk::PhysicalDevice,live_supported:bool}
impl VulkanBudget {
 pub fn new(instance:&Instance,physical:vk::PhysicalDevice)->Result<Self,Box<dyn std::error::Error>> {
  let properties=unsafe {instance.get_physical_device_memory_properties(physical)};
  let override_bytes=std::env::var("INGETRAZO_GPU_HEAP_LIMIT_BYTES").ok().map(|s|s.parse::<u64>()).transpose()?;
  let heaps=properties.memory_heaps[..properties.memory_heap_count as usize].iter().map(|h|(h.size,override_bytes.unwrap_or(h.size/10*8).min(h.size)));
  let extensions=unsafe {instance.enumerate_device_extension_properties(physical)}?;
  let live_supported=extensions.iter().any(|e|unsafe {std::ffi::CStr::from_ptr(e.extension_name.as_ptr())}==ash::ext::memory_budget::NAME);
  let this=Self {lifetime:Mutex::new(None),ledger:Mutex::new(MemoryBudget::new(heaps)),instance:instance.clone(),physical,live_supported};this.refresh();Ok(this)
 }
 pub fn bind_lifetime(&self,lifetime:std::sync::Arc<super::lifetime::Lifetime>){*self.lifetime.lock().unwrap_or_else(|p|p.into_inner())=Some(lifetime);}
 pub fn refresh(&self) {
  if !self.live_supported {return;}
  let mut budget=vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
  let mut properties=vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
  unsafe {self.instance.get_physical_device_memory_properties2(self.physical,&mut properties);}
  let count=properties.memory_properties.memory_heap_count as usize;
  let mut ledger=self.ledger.lock().unwrap_or_else(|p|p.into_inner());
  for i in 0..count {let _=ledger.update_live(i,budget.heap_budget[i]/10*9,budget.heap_usage[i]);}
 }
 pub fn diagnostics(&self)->String {let ledger=self.ledger.lock().unwrap_or_else(|p|p.into_inner());format!("GPU allocation budget: {} committed bytes; peakcommitted {}; peakadmitted {}; globalcap {:?}; heaps {:?}",ledger.total_committed(),ledger.peak_committed,ledger.peak_admitted,ledger.total_limit,ledger.heaps)}
}
impl egui_ash_renderer::MemoryObserver for VulkanBudget {
 fn can_destroy(&self)->bool{self.lifetime.lock().unwrap_or_else(|p|p.into_inner()).as_ref().is_none_or(|l|!l.uncertain())}
 fn gpu_wait_failed(&self){if let Some(l)=self.lifetime.lock().unwrap_or_else(|p|p.into_inner()).as_ref(){l.quarantine();}}
 fn reserve(&self,heap:u32,bytes:u64)->egui_ash_renderer::RendererResult<u64> {if !self.can_destroy(){return Err(egui_ash_renderer::RendererError::Allocator("GPU ownership quarantined".into()));}self.refresh();self.ledger.lock().unwrap_or_else(|p|p.into_inner()).reserve(heap as usize,bytes).map_err(egui_ash_renderer::RendererError::Allocator)}
 fn commit(&self,ticket:u64,memory:vk::DeviceMemory) {if let Err(error)=self.ledger.lock().unwrap_or_else(|p|p.into_inner()).commit(ticket,memory.as_raw()){log::error!("GPU budget commit: {error}");}}
 fn cancel(&self,ticket:u64) {if let Err(error)=self.ledger.lock().unwrap_or_else(|p|p.into_inner()).cancel(ticket){log::error!("GPU budget rollback: {error}");}}
 fn freed(&self,memory:vk::DeviceMemory) {if let Err(error)=self.ledger.lock().unwrap_or_else(|p|p.into_inner()).freed(memory.as_raw()){log::error!("GPU budget free: {error}");}}
}
