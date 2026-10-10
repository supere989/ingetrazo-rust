//! Direct Vulkan buffers, bounded uploads and fence-retired destruction.
mod image_upload;
pub use image_upload::TextureImage;
use ash::{vk,Device};
use ash::vk::Handle;
use egui_ash_renderer::MemoryObserver;
use std::{collections::HashMap,sync::Arc};
use super::budget::VulkanBudget;
const UPLOAD_CHUNK:u64=8*1024*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]pub struct BufferId(u64);
struct Buffer {handle:vk::Buffer,usage:vk::BufferUsageFlags,memory:vk::DeviceMemory,logical_bytes:u64,allocation_bytes:u64,properties:vk::MemoryPropertyFlags,last_use:u64,retire_after:Option<u64>,persistent_map:Option<usize>}
#[derive(Default,Debug,serde::Serialize)]pub struct UploadStats {pub buffer_allocations:u64,pub allocation_bytes_total:u64,pub buffer_destructions:u64,pub upload_calls:u64,pub staging_allocations:u64,pub transfer_command_allocations:u64,pub transfer_fence_creations:u64,pub transfer_submissions:u64,pub bytes:u64,pub chunks:u64,pub staging_peak:u64,pub direct_mapped_bytes:u64,pub readback_bytes:u64,pub readback_staging_peak:u64,pub image_upload_calls:u64,pub image_bytes:u64,pub image_mips_generated:u64,pub upload_staging_maps:u64,pub upload_staging_unmaps:u64}
pub struct Resources {lifetime:Arc<super::lifetime::Lifetime>,device:Device,budget:Arc<VulkanBudget>,properties:vk::PhysicalDeviceMemoryProperties,next:u64,buffers:HashMap<BufferId,Buffer>,pub stats:UploadStats,image_staging:Option<BufferId>,image_transfer:Option<image_upload::Transfer>}
impl Resources {
 pub fn new(context:&super::VulkanContext,budget:Arc<VulkanBudget>)->Self {budget.bind_lifetime(context.lifetime.clone());Self {lifetime:context.lifetime.clone(),device:context.device.clone(),budget,properties:unsafe {context.instance.get_physical_device_memory_properties(context.physical_device)},next:1,buffers:HashMap::new(),stats:Default::default(),image_staging:None,image_transfer:None}}
 // One direct command/fence owner is reused only after each upload completes.
 fn transfer_handles(&mut self,context:&super::VulkanContext)->Result<(vk::CommandBuffer,vk::Fence),String>{
  self.lifetime.check()?;
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Upload context differs from resource owner".into());}
  if self.image_transfer.is_none(){self.image_transfer=Some(image_upload::Transfer::new(context)?);self.stats.transfer_command_allocations+=1;self.stats.transfer_fence_creations+=1;}
  Ok(self.image_transfer.as_ref().unwrap().handles())
 }
 // The synchronous transfer fence makes one bounded host allocation reusable.
 fn staging_buffer(&mut self,context:&super::VulkanContext,capacity:u64)->Result<BufferId,String>{
  self.lifetime.check()?;
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Upload context differs from resource owner".into());}
  if capacity==0||capacity>UPLOAD_CHUNK{return Err("Invalid bounded staging capacity".into());}
  if self.image_staging.and_then(|id|self.logical_bytes(id)).is_none_or(|n|n<capacity){
   if let Some(old)=self.image_staging.take(){self.destroy(old);}
   let id=self.allocate(capacity,vk::BufferUsageFlags::TRANSFER_SRC,vk::MemoryPropertyFlags::HOST_VISIBLE)?;
   let b=&self.buffers[&id];
   let mapped=match unsafe{self.device.map_memory(b.memory,0,b.allocation_bytes,vk::MemoryMapFlags::empty())}{Ok(p)=>p,Err(e)=>{self.destroy(id);return Err(e.to_string());}};
   self.buffers.get_mut(&id).unwrap().persistent_map=Some(mapped as usize);
   self.stats.upload_staging_maps+=1;
   self.stats.staging_allocations+=1;
   self.stats.staging_peak=self.stats.staging_peak.max(self.buffers[&id].allocation_bytes);
   self.image_staging=Some(id);
  }
  Ok(self.image_staging.unwrap())
 }
 pub fn check_submission(&self)->Result<(),String>{self.lifetime.check()}
 pub fn drain_graphics(&self)->bool{self.lifetime.drain_graphics()}
 pub(crate) fn retain_owner<T:std::any::Any+Send+Sync>(&self,owner:T){self.lifetime.retain(owner);}
 pub fn buffer(&self,id:BufferId)->Option<vk::Buffer> {self.buffers.get(&id).map(|b|b.handle)}
 pub fn logical_bytes(&self,id:BufferId)->Option<u64> {self.buffers.get(&id).map(|b|b.logical_bytes)}
 pub fn allocation_bytes(&self,id:BufferId)->Option<u64> {self.buffers.get(&id).map(|b|b.allocation_bytes)}
 fn allocate(&mut self,bytes:u64,usage:vk::BufferUsageFlags,required:vk::MemoryPropertyFlags)->Result<BufferId,String> {
  self.lifetime.check()?;
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
  self.stats.buffer_allocations+=1;self.stats.allocation_bytes_total+=requirements.size;let id=BufferId(self.next);self.next+=1;self.buffers.insert(id,Buffer {handle:buffer,usage,memory,logical_bytes:bytes,allocation_bytes:requirements.size,properties,last_use:0,retire_after:None,persistent_map:None});Ok(id)
 }
 fn destroy(&mut self,id:BufferId) {if self.lifetime.uncertain(){return;}if let Some(buffer)=self.buffers.remove(&id) {self.stats.buffer_destructions+=1;unsafe {if buffer.persistent_map.is_some(){self.device.unmap_memory(buffer.memory);self.stats.upload_staging_unmaps+=1;}self.device.destroy_buffer(buffer.handle,None);self.device.free_memory(buffer.memory,None)};self.budget.freed(buffer.memory);}}
 fn write(&self,id:BufferId,data:&[u8])->Result<(),String> {
  self.lifetime.check()?;
  let buffer=self.buffers.get(&id).ok_or("Unknown GPU upload buffer")?;
  if data.len() as u64>buffer.logical_bytes {return Err("Upload exceeds GPU buffer extent".into());}
  let ptr=if let Some(mapped)=buffer.persistent_map {mapped as *mut std::ffi::c_void}else{unsafe {self.device.map_memory(buffer.memory,0,buffer.allocation_bytes,vk::MemoryMapFlags::empty())}.map_err(|e|e.to_string())?};
  unsafe {std::ptr::copy_nonoverlapping(data.as_ptr(),ptr.cast::<u8>(),data.len());}
  let result=if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {unsafe {self.device.flush_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(buffer.memory).offset(0).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())}else{Ok(())};
  if buffer.persistent_map.is_none(){unsafe {self.device.unmap_memory(buffer.memory)}};result
 }
 /// Reserve an unpublished arena page; no initialization or transfer is implied.
 pub fn allocate_buffer(&mut self,bytes:u64,usage:vk::BufferUsageFlags)->Result<BufferId,String>{
  let supported=vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::INDEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER|vk::BufferUsageFlags::UNIFORM_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC|vk::BufferUsageFlags::TRANSFER_DST;
  if bytes==0||bytes%4!=0||usage.is_empty()||!supported.contains(usage){return Err("Invalid arena buffer extent or usage".into());}
  self.allocate(bytes,usage|vk::BufferUsageFlags::TRANSFER_DST,vk::MemoryPropertyFlags::DEVICE_LOCAL)
 }
 /// Transfer into a reserved, unpublished range, retaining the bounded staging owner.
 ///
 /// # Safety
 /// The caller owns this range exclusively: it must not overlap any published,
 /// in-flight or pending-retirement range. Publication must follow successful
 /// completion of every candidate transfer. Failed candidates stay unpublished.
 pub unsafe fn upload_fresh_range(&mut self,context:&super::VulkanContext,id:BufferId,offset:u64,data:&[u8])->Result<(),String>{
  self.lifetime.check()?;
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Upload context differs from resource owner".into());}
  let buffer=self.buffers.get(&id).ok_or("Unknown arena buffer")?;
  let end=offset.checked_add(data.len()as u64).ok_or("Arena range overflow")?;
  if data.is_empty()||offset%4!=0||data.len()%4!=0||end>buffer.logical_bytes||buffer.retire_after.is_some()||!buffer.usage.contains(vk::BufferUsageFlags::TRANSFER_DST){return Err("Invalid or retired arena upload range".into());}
  let staging=self.staging_buffer(context,UPLOAD_CHUNK.min(data.len()as u64))?;
  self.transfer_chunks_at(context,id,staging,offset,data)?;
  self.stats.upload_calls+=1;self.stats.bytes+=data.len()as u64;Ok(())
 }
 /// Returns only after transfer completion; no scene serialization or IPC on upload path.
 pub fn upload(&mut self,context:&super::VulkanContext,data:&[u8],usage:vk::BufferUsageFlags,force_staging:bool)->Result<BufferId,String> {
  self.lifetime.check()?;
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Upload context differs from resource owner".into());}
  self.stats.upload_calls+=1;
  if data.len()%4!=0{return Err("Vulkan buffer upload must be a multiple of four bytes".into());}
  let destination=self.allocate(data.len() as u64,usage|vk::BufferUsageFlags::TRANSFER_DST,vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
  let result=(|| {
   if !force_staging&&self.buffers[&destination].properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE) {
    self.write(destination,data)?;self.stats.direct_mapped_bytes+=data.len() as u64;return Ok(());
   }
   let staging=self.staging_buffer(context,UPLOAD_CHUNK.min(data.len() as u64))?;
   self.transfer_chunks(context,destination,staging,data)
  })();
  if let Err(error)=result {self.destroy(destination);return Err(error);}
  self.stats.bytes+=data.len() as u64;Ok(destination)
 }
 /// Transactional bounded transfer batch; individual destination ownership is preserved.
 /// All input validation happens before allocations; one staging buffer and command/fence
 /// are retained across <=8MiB submissions and subsequent uploads.
 pub fn upload_many(&mut self,context:&super::VulkanContext,inputs:&[(&[u8],vk::BufferUsageFlags)])->Result<Vec<BufferId>,String>{
  self.lifetime.check()?;
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Upload context differs from resource owner".into());}
  let supported=vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::INDEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER|vk::BufferUsageFlags::UNIFORM_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC|vk::BufferUsageFlags::TRANSFER_DST;
  let mut total=0u64;
  for (data,usage) in inputs{
   if data.is_empty()||data.len()%4!=0{return Err("Batch upload requires nonempty four-byte-aligned buffers".into());}
   if usage.is_empty()||!supported.contains(*usage){return Err("Unsupported batch buffer transfer usage".into());}
   total=total.checked_add(data.len()as u64).ok_or("Batch byte count overflow")?;
  }
  if inputs.is_empty(){return Ok(Vec::new());}
  let mut destinations=Vec::new();
  let result=(||->Result<(),String>{
   for (data,usage) in inputs{destinations.push(self.allocate(data.len()as u64,*usage|vk::BufferUsageFlags::TRANSFER_DST,vk::MemoryPropertyFlags::DEVICE_LOCAL)?);}
   let id=self.staging_buffer(context,total.min(UPLOAD_CHUNK))?;
   self.transfer_many(context,&destinations,id,inputs)
  })();
  if let Err(error)=result{for id in destinations{self.destroy(id);}return Err(error);}
  self.stats.upload_calls+=inputs.len()as u64;self.stats.bytes+=total;Ok(destinations)
 }
 fn transfer_many(&mut self,context:&super::VulkanContext,destinations:&[BufferId],staging:BufferId,inputs:&[(&[u8],vk::BufferUsageFlags)])->Result<(),String>{
  let (command,fence)=self.transfer_handles(context)?;
  let mut submitted=false;
  let result=(||->Result<(),String>{
   let mut input=0usize;let mut offset=0usize;
   while input<inputs.len(){
    let mut pieces=Vec::new();let mut used=0usize;
    while input<inputs.len()&&used<UPLOAD_CHUNK as usize{
     let length=(inputs[input].0.len()-offset).min(UPLOAD_CHUNK as usize-used);
     pieces.push((input,offset,used,length));used+=length;offset+=length;
     if offset==inputs[input].0.len(){input+=1;offset=0;}
    }
    // Scatter directly into mapped staging: no additional packed CPU byte buffer.
    let host=&self.buffers[&staging];
    let pointer=host.persistent_map.ok_or("Transfer staging is not persistently mapped")? as *mut std::ffi::c_void;
    for &(index,start,target,length) in &pieces{unsafe{std::ptr::copy_nonoverlapping(inputs[index].0[start..start+length].as_ptr(),pointer.cast::<u8>().add(target),length)};}
    let flushed=if host.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT){Ok(())}else{unsafe{self.device.flush_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(host.memory).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())};
    flushed?;
    unsafe{
     self.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty()).map_err(|e|e.to_string())?;
     self.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e|e.to_string())?;
     let mut barriers=Vec::new();
     for &(index,start,source,length) in &pieces{
      let target=self.buffers[&destinations[index]].handle;
      self.device.cmd_copy_buffer(command,host.handle,target,&[vk::BufferCopy{src_offset:source as u64,dst_offset:start as u64,size:length as u64}]);
      barriers.push(vk::BufferMemoryBarrier::default().src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::MEMORY_READ).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).buffer(target).offset(start as u64).size(length as u64));
     }
     self.device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::TRANSFER,vk::PipelineStageFlags::ALL_COMMANDS,vk::DependencyFlags::empty(),&[],&barriers,&[]);
     self.device.end_command_buffer(command).map_err(|e|e.to_string())?;self.device.reset_fences(&[fence]).map_err(|e|e.to_string())?;
     self.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence).map_err(|e|{self.lifetime.quarantine();e.to_string()})?;
    }
    submitted=true;self.stats.transfer_submissions+=1;context.wait_transfer(fence).map_err(|e|e.to_string())?;submitted=false;self.stats.chunks+=1;
   }Ok(())
  })();
  if submitted||self.lifetime.uncertain(){self.lifetime.quarantine();self.lifetime.retain((context.command_pool,command,fence));return result;}
  result
 }
 fn transfer_chunks(&mut self,context:&super::VulkanContext,destination:BufferId,staging:BufferId,data:&[u8])->Result<(),String> {
  self.transfer_chunks_at(context,destination,staging,0,data)
 }
 fn transfer_chunks_at(&mut self,context:&super::VulkanContext,destination:BufferId,staging:BufferId,destination_offset:u64,data:&[u8])->Result<(),String> {
  let (command,fence)=self.transfer_handles(context)?;
  let mut submitted=false;
  let result=(|| {
   let mut offset=destination_offset;
   for chunk in data.chunks(UPLOAD_CHUNK as usize) {
    self.write(staging,chunk)?;
    unsafe {
     self.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty()).map_err(|e|e.to_string())?;
     self.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e|e.to_string())?;
     self.device.cmd_copy_buffer(command,self.buffers[&staging].handle,self.buffers[&destination].handle,&[vk::BufferCopy {src_offset:0,dst_offset:offset,size:chunk.len() as u64}]);
     let access=vk::AccessFlags::MEMORY_READ;
     let stages=vk::PipelineStageFlags::ALL_COMMANDS;
     let barrier=vk::BufferMemoryBarrier::default().src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(access).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).buffer(self.buffers[&destination].handle).offset(offset).size(chunk.len() as u64);
     self.device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::TRANSFER,stages,vk::DependencyFlags::empty(),&[],&[barrier],&[]);
     self.device.end_command_buffer(command).map_err(|e|e.to_string())?;
     self.device.reset_fences(&[fence]).map_err(|e|e.to_string())?;
     self.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence).map_err(|e|{self.lifetime.quarantine();e.to_string()})?;submitted=true;self.stats.transfer_submissions+=1;
     context.wait_transfer(fence).map_err(|e|e.to_string())?;submitted=false;
    }
    self.stats.chunks+=1;offset+=chunk.len() as u64;
   }
   Ok(())
  })();
  // A failed wait must not allow a staging resource to be freed while still in use.
  if submitted||self.lifetime.uncertain() {self.lifetime.quarantine();self.lifetime.retain((context.command_pool,command,fence));return result;}
  result
 }
 /// Actual GPU-to-host copy; works for unmappable discrete device-local memory.
 pub fn readback_buffer(&mut self,context:&super::VulkanContext,id:BufferId)->Result<Vec<u8>,String> {
  let source=self.buffers.get(&id).ok_or("Unknown GPU readback buffer")?;if !source.usage.contains(vk::BufferUsageFlags::TRANSFER_SRC){return Err("GPU buffer was not created for readback".into());}let bytes=source.logical_bytes;let handle=source.handle;
  self.read_chunks(context,bytes,4,|device,command,destination,offset,length|unsafe {
   let barrier=vk::BufferMemoryBarrier::default().src_access_mask(vk::AccessFlags::MEMORY_WRITE).dst_access_mask(vk::AccessFlags::TRANSFER_READ).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).buffer(handle).offset(offset).size(length);
   device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::ALL_COMMANDS,vk::PipelineStageFlags::TRANSFER,vk::DependencyFlags::empty(),&[],&[barrier],&[]);
   device.cmd_copy_buffer(command,handle,destination,&[vk::BufferCopy{src_offset:offset,dst_offset:0,size:length}]);
  })
 }
 /// Read a resolved RGBA8 image, restoring its sampled layout after every chunk.
 pub fn readback_image(&mut self,context:&super::VulkanContext,image:vk::Image,extent:vk::Extent2D)->Result<Vec<u8>,String> {
  let row=extent.width as u64*4;let bytes=row.checked_mul(extent.height as u64).ok_or("Readback image size overflow")?;
  self.read_chunks(context,bytes,row,|device,command,destination,offset,length|unsafe {
   let range=vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::COLOR).level_count(1).layer_count(1);
   let mut barrier=vk::ImageMemoryBarrier::default().image(image).subresource_range(range).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL).src_access_mask(vk::AccessFlags::MEMORY_WRITE|vk::AccessFlags::SHADER_READ).dst_access_mask(vk::AccessFlags::TRANSFER_READ);
   device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::ALL_COMMANDS,vk::PipelineStageFlags::TRANSFER,vk::DependencyFlags::empty(),&[],&[],&[barrier]);
   let region=vk::BufferImageCopy::default().image_subresource(vk::ImageSubresourceLayers::default().aspect_mask(vk::ImageAspectFlags::COLOR).layer_count(1)).image_offset(vk::Offset3D{x:0,y:(offset/row)as i32,z:0}).image_extent(vk::Extent3D{width:extent.width,height:(length/row)as u32,depth:1});
   device.cmd_copy_image_to_buffer(command,image,vk::ImageLayout::TRANSFER_SRC_OPTIMAL,destination,&[region]);
   barrier.old_layout=vk::ImageLayout::TRANSFER_SRC_OPTIMAL;barrier.new_layout=vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;barrier.src_access_mask=vk::AccessFlags::TRANSFER_READ;barrier.dst_access_mask=vk::AccessFlags::SHADER_READ;
   device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::TRANSFER,vk::PipelineStageFlags::FRAGMENT_SHADER,vk::DependencyFlags::empty(),&[],&[],&[barrier]);
  })
 }
 fn read_chunks(&mut self,context:&super::VulkanContext,bytes:u64,row:u64,mut record:impl FnMut(&Device,vk::CommandBuffer,vk::Buffer,u64,u64))->Result<Vec<u8>,String> {
  if bytes==0||row==0||row>UPLOAD_CHUNK||bytes%row!=0{return Err("Invalid GPU readback extent".into());}
  let chunk=(UPLOAD_CHUNK/row*row).min(bytes);
  let staging=self.allocate(chunk,vk::BufferUsageFlags::TRANSFER_DST,vk::MemoryPropertyFlags::HOST_VISIBLE)?;
  self.stats.readback_staging_peak=self.stats.readback_staging_peak.max(self.buffers[&staging].allocation_bytes);
  let info=vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1);
  let command=match unsafe {self.device.allocate_command_buffers(&info)} {Ok(c)=>c[0],Err(e)=>{self.destroy(staging);return Err(e.to_string());}};
  let fence=match unsafe {self.device.create_fence(&vk::FenceCreateInfo::default(),None)} {Ok(f)=>f,Err(e)=>{unsafe {self.device.free_command_buffers(context.command_pool,&[command])};self.destroy(staging);return Err(e.to_string());}};
  let mut submitted=false;
  let result=(|| {
   let mut output=Vec::with_capacity(usize::try_from(bytes).map_err(|_|"GPU readback does not fit host address space")?);let mut offset=0;
   while offset<bytes {
    let length=chunk.min(bytes-offset);let host=&self.buffers[&staging];
    unsafe {
     self.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty()).map_err(|e|e.to_string())?;
     self.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)).map_err(|e|e.to_string())?;
     record(&self.device,command,host.handle,offset,length);
     let barrier=vk::BufferMemoryBarrier::default().buffer(host.handle).offset(0).size(length).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).src_access_mask(vk::AccessFlags::TRANSFER_WRITE).dst_access_mask(vk::AccessFlags::HOST_READ);
     self.device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::TRANSFER,vk::PipelineStageFlags::HOST,vk::DependencyFlags::empty(),&[],&[barrier],&[]);
     self.device.end_command_buffer(command).map_err(|e|e.to_string())?;
     self.device.reset_fences(&[fence]).map_err(|e|e.to_string())?;
     self.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence).map_err(|e|{self.lifetime.quarantine();e.to_string()})?;submitted=true;
     context.wait_transfer(fence).map_err(|e|e.to_string())?;submitted=false;
    }
    let host=&self.buffers[&staging];let pointer=unsafe {self.device.map_memory(host.memory,0,host.allocation_bytes,vk::MemoryMapFlags::empty())}.map_err(|e|e.to_string())?;
    let read=(|| {
     if !host.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT){unsafe {self.device.invalidate_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(host.memory).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())?;}
     output.extend_from_slice(unsafe {std::slice::from_raw_parts(pointer.cast::<u8>(),length as usize)});Ok::<(),String>(())
    })();
    unsafe {self.device.unmap_memory(host.memory)};read?;offset+=length;
   }
   self.stats.readback_bytes+=bytes;Ok(output)
  })();
  if submitted||self.lifetime.uncertain() {self.lifetime.quarantine();self.lifetime.retain((context.command_pool,command,fence));return result;}
  unsafe {self.device.destroy_fence(fence,None);self.device.free_command_buffers(context.command_pool,&[command]);}
  self.destroy(staging);result
 }
 /// Diagnostic comparison after completed upload, for host-visible device-local heaps.
 pub fn verify_host_visible(&self,id:BufferId,expected:&[u8])->Result<(),String> {
  self.lifetime.check()?;
  let buffer=self.buffers.get(&id).ok_or("Unknown GPU buffer")?;
  if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_VISIBLE){return Err("Diagnostic requires host-visible device-local memory".into());}
  if expected.len() as u64!=buffer.logical_bytes{return Err("Diagnostic byte extent mismatch".into());}
  let ptr=if let Some(mapped)=buffer.persistent_map {mapped as *mut std::ffi::c_void}else{unsafe {self.device.map_memory(buffer.memory,0,buffer.allocation_bytes,vk::MemoryMapFlags::empty())}.map_err(|e|e.to_string())?};
  let result=(|| {if !buffer.properties.contains(vk::MemoryPropertyFlags::HOST_COHERENT) {unsafe {self.device.invalidate_mapped_memory_ranges(&[vk::MappedMemoryRange::default().memory(buffer.memory).size(vk::WHOLE_SIZE)])}.map_err(|e|e.to_string())?;}if unsafe {std::slice::from_raw_parts(ptr.cast::<u8>(),expected.len())}!=expected{return Err("GPU uploaded bytes differ".into());}Ok(())})();
  if buffer.persistent_map.is_none(){unsafe {self.device.unmap_memory(buffer.memory)}};result
 }
 pub fn mark_used(&mut self,id:BufferId,serial:u64)->Result<(),String> {self.lifetime.check()?;let b=self.buffers.get_mut(&id).ok_or("Unknown GPU buffer")?;if b.retire_after.is_some(){return Err("Cannot submit a retired GPU buffer".into());}b.last_use=b.last_use.max(serial);Ok(())}
 pub fn retire(&mut self,id:BufferId)->Result<(),String> {let b=self.buffers.get_mut(&id).ok_or("Unknown GPU buffer")?;b.retire_after=Some(b.last_use);self.budget.ledger.lock().unwrap().retire(b.memory.as_raw(),b.last_use)}
 /// Caller associates the completed fence with its monotonically increasing draw serial.
 pub fn collect(&mut self,fence:vk::Fence,serial:u64)->Result<usize,String> {
  if !unsafe {self.device.get_fence_status(fence)}.map_err(|e|e.to_string())? {return Ok(0);}
  let ready=self.buffers.iter().filter_map(|(&id,b)|b.retire_after.filter(|s|*s<=serial).map(|_|id)).collect::<Vec<_>>();let n=ready.len();for id in ready {self.destroy(id);}Ok(n)
 }
}
impl Drop for Resources {fn drop(&mut self){if !self.buffers.is_empty(){if !self.lifetime.drain_graphics(){eprintln!("Retaining quarantined GPU buffers and memory ledger");self.lifetime.retain(self.budget.clone());self.lifetime.retain(std::mem::take(&mut self.buffers));if let Some(transfer)=self.image_transfer.take(){self.lifetime.retain(transfer);}return;}let ids=self.buffers.keys().copied().collect::<Vec<_>>();for id in ids {self.destroy(id);}}}}
