//! Direct Vulkan buffers, bounded uploads and fence-retired destruction.
use ash::{vk,Device};
use ash::vk::Handle;
use egui_ash_renderer::MemoryObserver;
use std::{collections::HashMap,sync::Arc};
use super::budget::VulkanBudget;
const UPLOAD_CHUNK:u64=8*1024*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]pub struct BufferId(u64);
struct Buffer {handle:vk::Buffer,memory:vk::DeviceMemory,logical_bytes:u64,allocation_bytes:u64,properties:vk::MemoryPropertyFlags,last_use:u64,retire_after:Option<u64>}
#[derive(Default,Debug)]pub struct UploadStats {pub bytes:u64,pub chunks:u64,pub staging_peak:u64,pub direct_mapped_bytes:u64}
pub struct Resources {device:Device,budget:Arc<VulkanBudget>,properties:vk::PhysicalDeviceMemoryProperties,next:u64,buffers:HashMap<BufferId,Buffer>,pub stats:UploadStats}
impl Resources {
 pub fn new(context:&super::VulkanContext,budget:Arc<VulkanBudget>)->Self {Self {device:context.device.clone(),budget,properties:unsafe {context.instance.get_physical_device_memory_properties(context.physical_device)},next:1,buffers:HashMap::new(),stats:Default::default()}}
 pub fn buffer(&self,id:BufferId)->Option<vk::Buffer> {self.buffers.get(&id).map(|b|b.handle)}
 pub fn allocation_bytes(&self,id:BufferId)->Option<u64> {self.buffers.get(&id).map(|b|b.allocation_bytes)}
 fn allocate(&mut self,bytes:u64,usage:vk::BufferUsageFlags,required:vk::MemoryPropertyFlags)->Result<BufferId,String> {
  if bytes==0 {return Err("Empty GPU buffer".into());}
  let info=vk::BufferCreateInfo::default().size(bytes).usage(usage).sharing_mode(vk::SharingMode::EXCLUSIVE);
  let buffer=unsafe {self.device.create_buffer(&info,None)}.map_err(|e|e.to_string())?;
  let requirements=unsafe {self.device.get_buffer_memory_requirements(buffer)};
  let result=(|| {
   let ty=(0..self.properties.memory_type_count).find(|&i|requirements.memory_type_bits&(1<<i)!=0&&self.properties.memory_types[i as usize].property_flags.contains(required)).ok_or("No compatible GPU buffer memory type")?;
   let heap=self.properties.memory_types[ty as usize].heap_index;
   let ticket=self.budget.reserve(heap,requirements.size).map_err(|e|e.to_string())?;
   let info=vk::MemoryAllocateInfo::default().allocation_size(requirements.size).memory_type_index(ty);
   let memory=match unsafe {self.device.allocate_memory(&info,None)} {Ok(memory)=>memory,Err(error)=>{self.budget.cancel(ticket);return Err(error.to_string());}};
   if let Err(error)=unsafe {self.device.bind_buffer_memory(buffer,memory,0)} {unsafe {self.device.free_memory(memory,None)};self.budget.cancel(ticket);return Err(error.to_string());}
   self.budget.commit(ticket,memory);
   Ok((memory,self.properties.memory_types[ty as usize].property_flags))
  })();
  let (memory,properties)=match result {Ok(memory)=>memory,Err(error)=>{unsafe {self.device.destroy_buffer(buffer,None)};return Err(error);}};
  let id=BufferId(self.next);self.next+=1;self.buffers.insert(id,Buffer {handle:buffer,memory,logical_bytes:bytes,allocation_bytes:requirements.size,properties,last_use:0,retire_after:None});Ok(id)
 }
 fn destroy(&mut self,id:BufferId) {if let Some(buffer)=self.buffers.remove(&id) {unsafe {self.device.destroy_buffer(buffer.handle,None);self.device.free_memory(buffer.memory,None)};self.budget.freed(buffer.memory);}}
 fn write(&self,id:BufferId,data:&[u8])->Result<(),String> {
  let buffer=self.buffers.get(&id).ok_or("Unknown GPU upload buffer")?;
  if data.len() as u64>buffer.logical_bytes {return Err("Upload exceeds GPU buffer extent".into());}
  let ptr=unsafe {self.device.map_memory(buffer.memory,0,buffer.allocation_bytes,vk::MemoryMapFlags::empty())}.map_err(|e|e.to_string())?;
  unsafe {std::ptr::copy_nonoverlapping(data.as_ptr(),ptr.cast::<u8>(),data.len());}
  let result=if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {unsafe {self.device.flush_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(buffer.memory).offset(0).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())}else{Ok(())};
  unsafe {self.device.unmap_memory(buffer.memory)};result
 }
 /// Returns only after transfer completion; no scene serialization or IPC on upload path.
 pub fn upload(&mut self,context:&super::VulkanContext,data:&[u8],usage:vk::BufferUsageFlags,force_staging:bool)->Result<BufferId,String> {
  if data.len()%4!=0{return Err("Vulkan buffer upload must be a multiple of four bytes".into());}
  let destination=self.allocate(data.len() as u64,usage|vk::BufferUsageFlags::TRANSFER_DST,vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
  let result=(|| {
   if !force_staging&&self.buffers[&destination].properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
    self.write(destination,data)?;self.stats.direct_mapped_bytes+=data.len() as u64;return Ok(());
   }
   let staging=self.allocate(UPLOAD_CHUNK.min(data.len() as u64),vk::BufferUsageFlags::TRANSFER_SRC,vk::MemoryPropertyFlags::HOST_VISIBLE)?;
   self.stats.staging_peak=self.stats.staging_peak.max(self.buffers[&staging].allocation_bytes);
   let transfer=self.transfer_chunks(context,destination,staging,data);self.destroy(staging);transfer
  })();
  if let Err(error)=result {self.destroy(destination);return Err(error);}
  self.stats.bytes+=data.len() as u64;Ok(destination)
 }
 fn transfer_chunks(&mut self,context:&super::VulkanContext,destination:BufferId,staging:BufferId,data:&[u8])->Result<(),String> {
  let info=vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
  let command=unsafe {self.device.allocate_command_buffers(&info)}.map_err(|e|e.to_string())?[0];
  let fence=match unsafe {self.device.create_fence(&vk::FenceCreateInfo::default(),None)} {Ok(f)=>f,Err(error)=>{unsafe {self.device.free_command_buffers(context.command_pool,&[command])};return Err(error.to_string());}};
  let mut submitted=false;
  let result=(|| {
   let mut offset=0u64;
   for chunk in data.chunks(UPLOAD_CHUNK as usize) {
    self.write(staging,chunk)?;
    unsafe {
     self.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty()).map_err(|e|e.to_string())?;
     self.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e|e.to_string())?;
     self.device.cmd_copy_buffer(command,self.buffers[&staging].handle,self.buffers[&destination].handle,&[vk::BufferCopy {src_offset:0,dst_offset:offset,size:chunk.len() as u64}]);
     let barrier=vk::BufferMemoryBarrier::default().src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::VERTEX_ATTRIBUTE_READ|vk::AccessFlags::INDEX_READ).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).buffer(self.buffers[&destination].handle).offset(offset).size(chunk.len() as u64);
     self.device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::TRANSFER,vk::PipelineStageFlags::VERTEX_INPUT,vk::DependencyFlags::empty(),&[],&[barrier],&[]);
     self.device.end_command_buffer(command).map_err(|e|e.to_string())?;
     self.device.reset_fences(&[fence]).map_err(|e|e.to_string())?;
     self.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence).map_err(|e|e.to_string())?;submitted=true;
     self.device.wait_for_fences(&[fence],true,u64::MAX).map_err(|e|e.to_string())?;submitted=false;
    }
    self.stats.chunks+=1;offset+=chunk.len() as u64;
   }
   Ok(())
  })();
  // A failed wait must not allow a staging resource to be freed while still in use.
  if submitted {let _=unsafe {self.device.device_wait_idle()};}
  unsafe {self.device.destroy_fence(fence,None);self.device.free_command_buffers(context.command_pool,&[command]);}
  result
 }
 /// Diagnostic comparison after completed upload, for host-visible device-local heaps.
 pub fn verify_host_visible(&self,id:BufferId,expected:&[u8])->Result<(),String> {
  let buffer=self.buffers.get(&id).ok_or("Unknown GPU buffer")?;
  if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE){return Err("Diagnostic requires host-visible device-local memory".into());}
  if expected.len() as u64!=buffer.logical_bytes{return Err("Diagnostic byte extent mismatch".into());}
  let ptr=unsafe {self.device.map_memory(buffer.memory,0,buffer.allocation_bytes,vk::MemoryMapFlags::empty())}.map_err(|e|e.to_string())?;
  let result=(|| {if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {unsafe {self.device.invalidate_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(buffer.memory).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())?;}if unsafe {std::slice::from_raw_parts(ptr.cast::<u8>(),expected.len())}!=expected{return Err("GPU uploaded bytes differ".into());}Ok(())})();
  unsafe {self.device.unmap_memory(buffer.memory)};result
 }
 pub fn mark_used(&mut self,id:BufferId,serial:u64)->Result<(),String> {let b=self.buffers.get_mut(&id).ok_or("Unknown GPU buffer")?;if b.retire_after.is_some(){return Err("Cannot submit a retired GPU buffer".into());}b.last_use=b.last_use.max(serial);Ok(())}
 pub fn retire(&mut self,id:BufferId)->Result<(),String> {let b=self.buffers.get_mut(&id).ok_or("Unknown GPU buffer")?;b.retire_after=Some(b.last_use);self.budget.ledger.lock().unwrap().retire(b.memory.as_raw(),b.last_use)}
 /// Caller associates the completed fence with its monotonically increasing draw serial.
 pub fn collect(&mut self,fence:vk::Fence,serial:u64)->Result<usize,String> {
  if !unsafe {self.device.get_fence_status(fence)}.map_err(|e|e.to_string())? {return Ok(0);}
  let ready=self.buffers.iter().filter_map(|(&id,b)|b.retire_after.filter(|s|*s<=serial).map(|_|id)).collect::<Vec<_>>();let n=ready.len();for id in ready {self.destroy(id);}Ok(n)
 }
}
impl Drop for Resources {fn drop(&mut self){if !self.buffers.is_empty(){let _=unsafe {self.device.device_wait_idle()};let ids=self.buffers.keys().copied().collect::<Vec<_>>();for id in ids {self.destroy(id);}}}}
