//! Retained-depth query compute owner. No production MSAA policy is inferred.
use super::{annotation_visibility::Query,budget::VulkanBudget,resources::{Resources,BufferId},VulkanContext};
use ash::{vk,Device};
use std::sync::Arc;
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
pub(crate) struct Parameters {
 pub inverse:[f32;16],pub eye:[f32;4],pub forward:[f32;4],pub extent_count_sample:[u32;4],
}
pub(crate) struct Evaluator {
 device:Device,resources:Resources,layout:vk::PipelineLayout,pipeline:vk::Pipeline,
 set_layout:vk::DescriptorSetLayout,pool:vk::DescriptorPool,set:vk::DescriptorSet,
 query:Option<BufferId>,output:Option<BufferId>,count:u32,multisampled:bool,cached_queries:Vec<Query>,uploads:u64,
}
impl Evaluator {
 pub(crate) fn new(context:&VulkanContext,budget:Arc<VulkanBudget>,multisampled:bool)->Result<Self,String> {
  let families=unsafe{context.instance.get_physical_device_queue_family_properties(context.physical_device)};
  if !families[context.graphics_q_index as usize].queue_flags.contains(vk::QueueFlags::COMPUTE){return Err("Annotation visibility requires compute-capable graphics queue".into());}
  let device=context.device.clone();
  let mut owner=Self{device,resources:Resources::new(context,budget),layout:vk::PipelineLayout::null(),pipeline:vk::Pipeline::null(),set_layout:vk::DescriptorSetLayout::null(),pool:vk::DescriptorPool::null(),set:vk::DescriptorSet::null(),query:None,output:None,count:0,multisampled,cached_queries:Vec::new(),uploads:0};
  let bindings=[vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
   vk::DescriptorSetLayoutBinding::default().binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE),
   vk::DescriptorSetLayoutBinding::default().binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::COMPUTE)];
  owner.set_layout=unsafe{owner.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None)}.map_err(|e|e.to_string())?;
  let push=[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::COMPUTE).size(std::mem::size_of::<Parameters>() as u32)];
  owner.layout=unsafe{owner.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&[owner.set_layout]).push_constant_ranges(&push),None)}.map_err(|e|e.to_string())?;
  let bytes:&[u8]=if multisampled{include_bytes!("shaders/annotation_visibility.comp.spv")}else{include_bytes!("shaders/annotation_visibility_single.comp.spv")};
  let code=ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string())?;
  let module=unsafe{owner.device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code),None)}.map_err(|e|e.to_string())?;
  let info=vk::ComputePipelineCreateInfo::default().layout(owner.layout).stage(vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::COMPUTE).module(module).name(c"main"));
  let pipeline=unsafe{owner.device.create_compute_pipelines(vk::PipelineCache::null(),&[info],None)};
  unsafe{owner.device.destroy_shader_module(module,None);}
  owner.pipeline=match pipeline{Ok(p)=>p[0],Err((partial,e))=>{for p in partial{if p!=vk::Pipeline::null(){unsafe{owner.device.destroy_pipeline(p,None);}}}return Err(e.to_string());}};
  let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::SAMPLED_IMAGE,descriptor_count:1},vk::DescriptorPoolSize{ty:vk::DescriptorType::STORAGE_BUFFER,descriptor_count:2}];
  owner.pool=unsafe{owner.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&sizes),None)}.map_err(|e|e.to_string())?;
  owner.set=unsafe{owner.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(owner.pool).set_layouts(&[owner.set_layout]))}.map_err(|e|e.to_string())?[0];
  Ok(owner)
 }
 /// Previous use must have completed. Depth owner must outlive this set and GPU
 /// submission. Only World supplies the live view; no foreign raw-view API.
 pub(crate) fn prepare(&mut self,context:&VulkanContext,queries:&[Query],depth:vk::ImageView,fence:vk::Fence,completed:u64)->Result<(),String>{
  let bytes=bytemuck::cast_slice(queries);
  let limits=unsafe{context.instance.get_physical_device_properties(context.physical_device)}.limits;
  let max=limits.max_storage_buffer_range as usize;
  if queries.is_empty()||queries.len()>u32::MAX as usize||queries.len().div_ceil(64)>limits.max_compute_work_group_count[0] as usize||bytes.len()>max{return Err("Annotation query storage admission failed".into());}
  // Validate completion and resource owner before descriptor reuse/allocation.
  self.resources.publication_preflight(context,fence,completed,&[],&[])?;
  if self.cached_queries==queries {
   let image=[vk::DescriptorImageInfo::default().image_view(depth).image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
   unsafe{self.device.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(self.set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&image)],&[]);}
   return Ok(());
  }
  let mut cached=Vec::new();cached.try_reserve_exact(queries.len()).map_err(|_|"Annotation query cache admission failed")?;cached.extend_from_slice(queries);
  let query=self.resources.upload(context,bytes,vk::BufferUsageFlags::STORAGE_BUFFER,false)?;
  let output=match self.resources.allocate_buffer(queries.len() as u64*4,vk::BufferUsageFlags::STORAGE_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC){Ok(id)=>id,Err(e)=>{self.resources.retire(query)?;self.resources.collect(fence,completed)?;return Err(e);}};
  let old=[self.query,self.output].into_iter().flatten().collect::<Vec<_>>();
  let proof=match self.resources.publication_preflight(context,fence,completed,&old,&[query,output]){Ok(p)=>p,Err(e)=>{self.resources.retire(query)?;self.resources.retire(output)?;self.resources.collect(fence,completed)?;return Err(e);}};
  let image=[vk::DescriptorImageInfo::default().image_view(depth).image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
  let input=[vk::DescriptorBufferInfo::default().buffer(self.resources.buffer(query).unwrap()).range(bytes.len() as u64)];
  let result=[vk::DescriptorBufferInfo::default().buffer(self.resources.buffer(output).unwrap()).range(queries.len() as u64*4)];
  unsafe{self.device.update_descriptor_sets(&[
   vk::WriteDescriptorSet::default().dst_set(self.set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&image),
   vk::WriteDescriptorSet::default().dst_set(self.set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&input),
   vk::WriteDescriptorSet::default().dst_set(self.set).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&result)],&[]);}
  self.resources.retire_completed(proof);
  self.query=Some(query);self.output=Some(output);self.count=queries.len() as u32;self.cached_queries=cached;self.uploads+=1;Ok(())
 }
 pub(crate) fn record(&mut self,command:vk::CommandBuffer,serial:u64,mut p:Parameters,samples:u32)->Result<(),String>{
  if p.extent_count_sample[0]==0||p.extent_count_sample[1]==0||samples==0||p.extent_count_sample[3]>=samples||self.multisampled!=(samples>1){return Err("Annotation depth sample policy/extent mismatch".into());}
  if !p.inverse.iter().chain(p.eye.iter()).chain(p.forward.iter()).all(|x|x.is_finite()){return Err("Nonfinite annotation inverse snapshot".into());}
  let query=self.query.ok_or("Annotation queries not prepared")?;let output=self.output.ok_or("Annotation output not prepared")?;
  self.resources.mark_used(query,serial)?;self.resources.mark_used(output,serial)?;
  p.extent_count_sample[2]=self.count;
  unsafe{
   self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::COMPUTE,self.pipeline);
   self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::COMPUTE,self.layout,0,&[self.set],&[]);
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::COMPUTE,0,bytemuck::bytes_of(&p));
   self.device.cmd_dispatch(command,self.count.div_ceil(64),1,1);
   let barrier=[vk::BufferMemoryBarrier::default().src_access_mask(vk::AccessFlags::SHADER_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ|vk::AccessFlags::TRANSFER_READ).src_queue_family_index(vk::QUEUE_FAMILY_IGNORED).dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED).buffer(self.resources.buffer(output).unwrap()).offset(0).size(self.count as u64*4)];
   self.device.cmd_pipeline_barrier(command,vk::PipelineStageFlags::COMPUTE_SHADER,vk::PipelineStageFlags::VERTEX_SHADER|vk::PipelineStageFlags::FRAGMENT_SHADER|vk::PipelineStageFlags::TRANSFER,vk::DependencyFlags::empty(),&[],&barrier,&[]);
  }Ok(())
 }
 pub(crate) fn uploads(&self)->u64{self.uploads}
 pub(crate) fn output_buffer(&self)->Option<vk::Buffer>{self.output.and_then(|id|self.resources.buffer(id))}
 pub(crate) fn readback_for_probe(&mut self,context:&VulkanContext)->Result<Vec<u8>,String>{self.resources.readback_buffer(context,self.output.ok_or("Annotation output missing")?)}
}
impl Drop for Evaluator{
 fn drop(&mut self){
  if !self.resources.drain_graphics(){return;} // lifetime owns unknown device/resources; handles deliberately retained
  unsafe{
   if self.pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(self.pool,None);}
   if self.pipeline!=vk::Pipeline::null(){self.device.destroy_pipeline(self.pipeline,None);}
   if self.layout!=vk::PipelineLayout::null(){self.device.destroy_pipeline_layout(self.layout,None);}
   if self.set_layout!=vk::DescriptorSetLayout::null(){self.device.destroy_descriptor_set_layout(self.set_layout,None);}
  }
 }
}
