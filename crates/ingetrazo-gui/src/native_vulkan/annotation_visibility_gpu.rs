//! Retained-depth query compute owner. No production MSAA policy is inferred.
use super::{annotation_visibility::Query,budget::VulkanBudget,resources::{Resources,BufferId,CompletedRetirement},VulkanContext};
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
 output_identity:Arc<()>,query:Option<BufferId>,output:Option<BufferId>,count:u32,multisampled:bool,cached_queries:Arc<[Query]>,uploads:u64,depth_identity:Option<Arc<()>>,
}
impl Evaluator {
 pub(crate) fn new(context:&VulkanContext,budget:Arc<VulkanBudget>,multisampled:bool)->Result<Self,String> {
  let families=unsafe{context.instance.get_physical_device_queue_family_properties(context.physical_device)};
  if !families[context.graphics_q_index as usize].queue_flags.contains(vk::QueueFlags::COMPUTE){return Err("Annotation visibility requires compute-capable graphics queue".into());}
  let device=context.device.clone();
  let mut owner=Self{device,resources:Resources::new(context,budget),layout:vk::PipelineLayout::null(),pipeline:vk::Pipeline::null(),set_layout:vk::DescriptorSetLayout::null(),pool:vk::DescriptorPool::null(),set:vk::DescriptorSet::null(),output_identity:Arc::new(()),query:None,output:None,count:0,multisampled,cached_queries:Arc::from([]),uploads:0,depth_identity:None};
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
 pub(crate) fn prepare(&mut self,context:&VulkanContext,queries:&[Query],depth:vk::ImageView,depth_identity:&Arc<()>,fence:vk::Fence,completed:u64)->Result<(),String>{
  self.stage(context,queries,depth,depth_identity,fence,completed)?.commit();
  Ok(())
 }
 /// Stage all query/output ownership without touching live descriptors or retiring
 /// the previous inventory. The overlay mesh must also be admitted before commit.
 pub(crate) fn stage<'a>(&'a mut self,context:&VulkanContext,queries:&[Query],depth:vk::ImageView,depth_identity:&Arc<()>,fence:vk::Fence,completed:u64)->Result<PreparedVisibility<'a>,String>{
  let cached=if self.cached_queries.as_ref()==queries{self.cached_queries.clone()}else{
   let mut cached=Vec::new();cached.try_reserve_exact(queries.len()).map_err(|_|"Annotation query cache admission failed")?;cached.extend_from_slice(queries);cached.into()
  };
  self.stage_shared(context,cached,depth,depth_identity,fence,completed)
 }
 /// Immutable production query snapshot; unchanged identity avoids scanning or
 /// copying query arrays on warm frames.
 pub(crate) fn stage_shared<'a>(&'a mut self,context:&VulkanContext,queries:Arc<[Query]>,depth:vk::ImageView,depth_identity:&Arc<()>,fence:vk::Fence,completed:u64)->Result<PreparedVisibility<'a>,String>{
  let bytes=bytemuck::cast_slice(queries.as_ref());
  let limits=unsafe{context.instance.get_physical_device_properties(context.physical_device)}.limits;
  let max=limits.max_storage_buffer_range as usize;
  if queries.is_empty()||queries.len()>u32::MAX as usize||queries.len().div_ceil(64)>limits.max_compute_work_group_count[0] as usize||bytes.len()>max{return Err("Annotation query storage admission failed".into());}
  self.resources.publication_preflight(context,fence,completed,&[],&[])?;
  if Arc::ptr_eq(&self.cached_queries,&queries) {
   return Ok(PreparedVisibility{owner:self,depth,depth_identity:depth_identity.clone(),candidate:None,fence,completed});
  }
  if self.query.is_some()!=self.output.is_some(){return Err("Inconsistent annotation buffer owner".into());}
  let old_ids=self.query.zip(self.output).map(|(q,o)|[q,o]);
  let old=old_ids.as_ref().map_or(&[][..],|ids|ids.as_slice());
  let identity=Arc::new(());
  let query=self.resources.upload(context,bytes,vk::BufferUsageFlags::STORAGE_BUFFER,false)?;
  let output=match self.resources.allocate_buffer(queries.len() as u64*4,vk::BufferUsageFlags::STORAGE_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC){Ok(id)=>id,Err(e)=>{self.resources.retire(query)?;self.resources.collect(fence,completed)?;return Err(e);}};
  let proof=match self.resources.publication_preflight(context,fence,completed,old,&[query,output]){Ok(p)=>p,Err(e)=>{self.resources.retire(query)?;self.resources.retire(output)?;self.resources.collect(fence,completed)?;return Err(e);}};
  let cached=queries;
  let query_buffer=self.resources.buffer(query).expect("preflight admitted query buffer");
  let output_buffer=self.resources.buffer(output).expect("preflight admitted output buffer");
  Ok(PreparedVisibility{owner:self,depth,depth_identity:depth_identity.clone(),candidate:Some(StagedBuffers{query,output,query_buffer,output_buffer,cached,identity,proof}),fence,completed})
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
 pub(crate) fn bind_overlay(&self,renderer:&mut super::cad2d::Renderer,context:&VulkanContext,fence:vk::Fence,completed:u64)->Result<(),String>{
  self.resources.publication_preflight(context,fence,completed,&[],&[])?;
  let output=self.output.ok_or("Annotation output not prepared")?;
  let buffer=self.resources.buffer(output).ok_or("Retained annotation output missing")?;
  renderer.bind_annotation_visibility(context,buffer,self.count,&self.output_identity,fence,completed)
 }

 pub(crate) fn readback_for_probe(&mut self,context:&VulkanContext)->Result<Vec<u8>,String>{self.resources.readback_buffer(context,self.output.ok_or("Annotation output missing")?)}
}
struct StagedBuffers {
 query:BufferId,output:BufferId,query_buffer:vk::Buffer,output_buffer:vk::Buffer,
 cached:Arc<[Query]>,identity:Arc<()>,proof:CompletedRetirement,
}
/// Borrowed RAII candidate: abandonment releases only unsubmitted candidate
/// ownership and leaves the old descriptor set, identities and counters intact.
pub(crate) struct PreparedVisibility<'a> {
 owner:&'a mut Evaluator,depth:vk::ImageView,depth_identity:Arc<()>,
 candidate:Option<StagedBuffers>,fence:vk::Fence,completed:u64,
}
impl PreparedVisibility<'_> {
 pub(crate) fn output(&self)->Result<(vk::Buffer,u32,&Arc<()>),String>{
  if let Some(c)=&self.candidate{return Ok((c.output_buffer,c.cached.len() as u32,&c.identity));}
  let id=self.owner.output.ok_or("Annotation output not prepared")?;
  Ok((self.owner.resources.buffer(id).ok_or("Retained annotation output missing")?,self.owner.count,&self.owner.output_identity))
 }
 /// Infallible publication after both query and geometry admission. Caller must
 /// not publish a mesh that can still fail before calling this method.
 pub(crate) fn commit(mut self){
  let image=[vk::DescriptorImageInfo::default().image_view(self.depth).image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)];
  if let Some(c)=self.candidate.take(){
   let input=[vk::DescriptorBufferInfo::default().buffer(c.query_buffer).range(c.cached.len() as u64*std::mem::size_of::<Query>() as u64)];
   let result=[vk::DescriptorBufferInfo::default().buffer(c.output_buffer).range(c.cached.len() as u64*4)];
   unsafe{self.owner.device.update_descriptor_sets(&[
    vk::WriteDescriptorSet::default().dst_set(self.owner.set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&image),
    vk::WriteDescriptorSet::default().dst_set(self.owner.set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&input),
    vk::WriteDescriptorSet::default().dst_set(self.owner.set).dst_binding(2).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&result)],&[]);}
   self.owner.resources.retire_completed(c.proof);
   self.owner.output_identity=c.identity;self.owner.query=Some(c.query);self.owner.output=Some(c.output);
   self.owner.count=c.cached.len() as u32;self.owner.cached_queries=c.cached;self.owner.uploads+=1;
  }else if !self.owner.depth_identity.as_ref().is_some_and(|old|Arc::ptr_eq(old,&self.depth_identity)){
   unsafe{self.owner.device.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(self.owner.set).dst_binding(0).descriptor_type(vk::DescriptorType::SAMPLED_IMAGE).image_info(&image)],&[]);}
  }
  self.owner.depth_identity=Some(self.depth_identity.clone());
 }
}
impl Drop for PreparedVisibility<'_>{
 fn drop(&mut self){
  if let Some(c)=self.candidate.take(){
   let cleanup=self.owner.resources.retire(c.query)
    .and_then(|_|self.owner.resources.retire(c.output))
    .and_then(|_|self.owner.resources.collect(self.fence,self.completed).map(|_|()));
   if let Err(error)=cleanup{eprintln!("Annotation candidate ownership retained after cleanup refusal: {error}");}
  }
 }
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
