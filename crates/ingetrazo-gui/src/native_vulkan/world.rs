//! Bounded opaque world-space Vulkan pass. Full style/texture/section parity remains open.
use ash::{vk,Device};
use egui_ash_renderer::MemoryObserver;
use std::sync::Arc;
use super::{budget::VulkanBudget,resources::{Resources,BufferId},VulkanContext};
struct Image {device:Device,budget:Arc<VulkanBudget>,handle:vk::Image,memory:vk::DeviceMemory,view:vk::ImageView}
impl Image {
 fn new(context:&VulkanContext,budget:Arc<VulkanBudget>,extent:vk::Extent2D,format:vk::Format,usage:vk::ImageUsageFlags,aspect:vk::ImageAspectFlags)->Result<Self,String> {
  Self::new_samples(context,budget,extent,format,usage,aspect,vk::SampleCountFlags::TYPE_1)
 }
 fn new_samples(context:&VulkanContext,budget:Arc<VulkanBudget>,extent:vk::Extent2D,format:vk::Format,usage:vk::ImageUsageFlags,aspect:vk::ImageAspectFlags,samples:vk::SampleCountFlags)->Result<Self,String> {
  Self::new_with_view(context,budget,extent,format,usage,aspect,samples,|device,info|unsafe {device.create_image_view(info,None)})
 }
 fn new_with_view(context:&VulkanContext,budget:Arc<VulkanBudget>,extent:vk::Extent2D,format:vk::Format,usage:vk::ImageUsageFlags,aspect:vk::ImageAspectFlags,samples:vk::SampleCountFlags,create_view:impl FnOnce(&Device,&vk::ImageViewCreateInfo)->Result<vk::ImageView,vk::Result>)->Result<Self,String> {
  let device=context.device.clone();let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).extent(vk::Extent3D{width:extent.width,height:extent.height,depth:1}).mip_levels(1).array_layers(1).format(format).tiling(vk::ImageTiling::OPTIMAL).usage(usage).samples(samples);
  let handle=unsafe {device.create_image(&info,None)}.map_err(|e|e.to_string())?;let requirements=unsafe {device.get_image_memory_requirements(handle)};
  let mut bound_memory=None;
  let result=(|| {
   let properties=unsafe {context.instance.get_physical_device_memory_properties(context.physical_device)};
   let ty=(0..properties.memory_type_count).find(|&i|requirements.memory_type_bits&(1<<i)!=0&&properties.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)).ok_or("No device-local image memory")?;
   let ticket=budget.reserve(properties.memory_types[ty as usize].heap_index,requirements.size).map_err(|e|e.to_string())?;
   let info=vk::MemoryAllocateInfo::default().allocation_size(requirements.size).memory_type_index(ty);
   let memory=match unsafe {device.allocate_memory(&info,None)} {Ok(m)=>m,Err(e)=>{budget.cancel(ticket);return Err(e.to_string());}};
   if let Err(e)=unsafe {device.bind_image_memory(handle,memory,0)} {unsafe {device.free_memory(memory,None)};budget.cancel(ticket);return Err(e.to_string());}
   budget.commit(ticket,memory);
   bound_memory=Some(memory);
   let range=vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1);
   match create_view(&device,&vk::ImageViewCreateInfo::default().image(handle).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(range)) {
    Ok(view)=>Ok((memory,view)),Err(e)=>Err(e.to_string())
   }
  })();
  match result {Ok((memory,view))=>Ok(Self{device,budget,handle,memory,view}),Err(e)=>{unsafe {device.destroy_image(handle,None)};if let Some(memory)=bound_memory {unsafe {device.free_memory(memory,None)};budget.freed(memory);}Err(e)}}
 }
}
impl Drop for Image {fn drop(&mut self){unsafe {self.device.destroy_image_view(self.view,None);self.device.destroy_image(self.handle,None);self.device.free_memory(self.memory,None)};self.budget.freed(self.memory);}}
struct Target {device:Device,pool:vk::DescriptorPool,_color:Image,_msaa:Option<Image>,_depth:Image,framebuffer:vk::Framebuffer,sampler:vk::Sampler,set:vk::DescriptorSet,extent:vk::Extent2D,texture:Option<egui::TextureId>}
impl Drop for Target {fn drop(&mut self){unsafe {self.device.destroy_framebuffer(self.framebuffer,None);self.device.destroy_sampler(self.sampler,None);let _=self.device.free_descriptor_sets(self.pool,&[self.set]);}}}
#[derive(Default,Clone,Copy,Debug)]pub struct ContourMetrics {pub additional_position_bytes:u64,pub batches:usize,pub cold_build_ms:f64,pub compact_preprocess_ms:f64,pub warm_cache_ms:f64,pub cache_hits:u64,pub failed_prepared_chunks:usize,pub rollbacks:u64,pub edge_buffer_uploads:u64,pub failed_prepared_edge_bytes:u64}
#[derive(Clone,Copy)]struct ContourBatch {vertices:BufferId,indices:BufferId,triangles:u32,compact:bool,stride:u32,owned:bool}
struct Contours {device:Device,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pool:vk::DescriptorPool,pipeline:vk::Pipeline,sets:Vec<vk::DescriptorSet>,limit:u64}
impl Contours {
 fn new(context:&VulkanContext,render_pass:vk::RenderPass,samples:vk::SampleCountFlags)->Result<Self,String>{
  let limits=unsafe{context.instance.get_physical_device_properties(context.physical_device)}.limits;
  if limits.max_per_stage_descriptor_storage_buffers<2||limits.max_descriptor_set_storage_buffers<2{return Err("Section contours require two readonly vertex storage buffers".into());}
  let device=context.device.clone();let mut result=Self{device,layout:vk::PipelineLayout::null(),descriptor_layout:vk::DescriptorSetLayout::null(),pool:vk::DescriptorPool::null(),pipeline:vk::Pipeline::null(),sets:Vec::new(),limit:limits.max_storage_buffer_range as u64};
  let bindings=[0,1].map(|binding|vk::DescriptorSetLayoutBinding::default().binding(binding).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::VERTEX));
  unsafe {
   result.descriptor_layout=result.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None).map_err(|e|e.to_string())?;
   let layouts=[result.descriptor_layout];let push=[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT).size(128)];
   result.layout=result.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&layouts).push_constant_ranges(&push),None).map_err(|e|e.to_string())?;

  }
  result.pipeline=make_pipeline(&result.device,render_pass,result.layout,WorldPass::Contour,samples,false)?;Ok(result)
 }
 fn replace_batches(&mut self,resources:&Resources,batches:&[ContourBatch])->Result<(),String>{
  // Prepare a replacement pool before touching descriptors used by the old model.
  if batches.is_empty(){unsafe{if self.pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(self.pool,None);}}self.pool=vk::DescriptorPool::null();self.sets.clear();return Ok(());}
  let count=u32::try_from(batches.len()).map_err(|_|"Too many contour batches")?;
  let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::STORAGE_BUFFER,descriptor_count:count.checked_mul(2).ok_or("Too many contour descriptors")?}];
  let pool=unsafe{self.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(count).pool_sizes(&sizes),None)}.map_err(|e|e.to_string())?;
  let prepare=(||->Result<Vec<vk::DescriptorSet>,String>{
   let layouts=vec![self.descriptor_layout;batches.len()];let sets=unsafe{self.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts))}.map_err(|e|e.to_string())?;
   for (batch,set) in batches.iter().zip(&sets){
    let vb=resources.logical_bytes(batch.vertices).ok_or("Missing contour vertices")?;let ib=resources.logical_bytes(batch.indices).ok_or("Missing contour indices")?;
    if vb>self.limit||ib>self.limit{return Err("Internal contour chunk exceeded storage limit".into());}
    let vertices=[vk::DescriptorBufferInfo::default().buffer(resources.buffer(batch.vertices).unwrap()).range(vb)];let indices=[vk::DescriptorBufferInfo::default().buffer(resources.buffer(batch.indices).unwrap()).range(ib)];
    let writes=[vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(0).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&vertices),vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(1).descriptor_type(vk::DescriptorType::STORAGE_BUFFER).buffer_info(&indices)];unsafe{self.device.update_descriptor_sets(&writes,&[]);}
   }Ok(sets)
  })();
  match prepare {Ok(sets)=>{unsafe{if self.pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(self.pool,None);}}self.pool=pool;self.sets=sets;Ok(())},Err(error)=>{unsafe{self.device.destroy_descriptor_pool(pool,None);}Err(error)}}
 }

}
impl Drop for Contours {fn drop(&mut self){unsafe{if self.pipeline!=vk::Pipeline::null(){self.device.destroy_pipeline(self.pipeline,None);}if self.layout!=vk::PipelineLayout::null(){self.device.destroy_pipeline_layout(self.layout,None);}if self.pool!=vk::DescriptorPool::null(){self.device.destroy_descriptor_pool(self.pool,None);}if self.descriptor_layout!=vk::DescriptorSetLayout::null(){self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}}
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
#[repr(C)]struct ColoredVertex {position:[f32;3],color:[f32;3]}
#[derive(Clone,Copy)]struct OpacityRun {first:u32,count:u32,opacity:f32,kind:u32}
pub struct World {textures:Option<super::textures::Textures>,texture_enabled:bool,has_texture_sources:bool,show_edges:bool,edge_vertices:Option<BufferId>,edge_count:u32,style_mode:ingetrazo_render::style::VisualStyle,style_front:[f32;4],style_back:[f32;4],default_back_runs:Vec<(u32,u32)>,consolidated_opaque_count:usize,paint_pipelines:[vk::Pipeline;10],ordinary_runs:Vec<(u32,u32,bool)>,transparent_vertices:Option<BufferId>,opacity_runs:Vec<OpacityRun>,transparent_pipeline:vk::Pipeline,probe_pressure_after:Option<usize>,contour_metrics:ContourMetrics,contours:Option<Contours>,contour_batches:Vec<ContourBatch>,background:[f32;4],eye:[f32;3],width_factor:f32,display_scale:f32,edge_color:[f32;4],device:Device,budget:Arc<VulkanBudget>,resources:Resources,render_pass:vk::RenderPass,pipeline:vk::Pipeline,mask_pipeline:vk::Pipeline,cap_pipeline:vk::Pipeline,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pool:vk::DescriptorPool,depth_format:vk::Format,samples:vk::SampleCountFlags,target:Option<Target>,vertices:Option<BufferId>,indices:Option<BufferId>,count:u32,cap_vertices:Option<BufferId>,cap_signature:Option<([[f32;3];4],[f32;4])>,cap_key:Option<((u64,u64,u64),[f32;4],bool,[f32;4])>,revision:Option<(u64,u64,u64)>,mvp:[f32;16],clip:[f32;4],clip_enabled:u32,pub uploads:u64}
impl World {
 pub fn new(context:&VulkanContext,budget:Arc<VulkanBudget>)->Result<Self,String> {
  let device=context.device.clone();
  let depth_format=[vk::Format::D24_UNORM_S8_UINT,vk::Format::D32_SFLOAT_S8_UINT].into_iter().find(|f|unsafe {context.instance.get_physical_device_format_properties(context.physical_device,*f)}.optimal_tiling_features.contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)).ok_or("No supported depth/stencil format")?;
  let limits=unsafe {context.instance.get_physical_device_properties(context.physical_device)}.limits;
  let counts=limits.framebuffer_color_sample_counts&limits.framebuffer_depth_sample_counts&limits.framebuffer_stencil_sample_counts;
  let color_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,vk::Format::R8G8B8A8_UNORM,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World color image format unavailable: {e}"))?.sample_counts;
  let depth_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,depth_format,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World depth/stencil image format unavailable: {e}"))?.sample_counts;
  let resolve_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,vk::Format::R8G8B8A8_UNORM,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::COLOR_ATTACHMENT|vk::ImageUsageFlags::SAMPLED|vk::ImageUsageFlags::TRANSFER_SRC,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World sampled resolve format unavailable: {e}"))?.sample_counts;
  if !resolve_samples.contains(vk::SampleCountFlags::TYPE_1){return Err("World resolve image does not support one sample".into());}
  let samples=select_sample_count(counts,color_samples,depth_samples)?;
  eprintln!("Native world MSAA: {:?} (original default4; unsupported fallback remains explicit)",samples);
  let multisampled=samples!=vk::SampleCountFlags::TYPE_1;
  let mut attachments=vec![vk::AttachmentDescription::default().format(vk::Format::R8G8B8A8_UNORM).samples(samples).load_op(vk::AttachmentLoadOp::CLEAR).store_op(if multisampled{vk::AttachmentStoreOp::DONT_CARE}else{vk::AttachmentStoreOp::STORE}).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(if multisampled{vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}else{vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL})];
  if multisampled {attachments.push(vk::AttachmentDescription::default().format(vk::Format::R8G8B8A8_UNORM).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::DONT_CARE).store_op(vk::AttachmentStoreOp::STORE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL));}
  let depth_index=attachments.len() as u32;
  attachments.push(vk::AttachmentDescription::default().format(depth_format).samples(samples).load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::DONT_CARE).stencil_load_op(vk::AttachmentLoadOp::CLEAR).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL));
  let colors=[vk::AttachmentReference{attachment:0,layout:vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}];let depth=vk::AttachmentReference{attachment:depth_index,layout:vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL};
  let resolve=[vk::AttachmentReference{attachment:1,layout:vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}];
  let subpass=vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colors).depth_stencil_attachment(&depth);
  let subpasses=[if multisampled{subpass.resolve_attachments(&resolve)}else{subpass}];
  let dependencies=[vk::SubpassDependency::default().src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0).src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT|vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS).src_access_mask(vk::AccessFlags::SHADER_READ).dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE|vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE),vk::SubpassDependency::default().src_subpass(0).dst_subpass(vk::SUBPASS_EXTERNAL).src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT).dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)];
  let render_pass=unsafe {device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses).dependencies(&dependencies),None)}.map_err(|e|e.to_string())?;
  let push=[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT).size(128)];let layout=match unsafe {device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push),None)} {Ok(layout)=>layout,Err(e)=>{unsafe {device.destroy_render_pass(render_pass,None)};return Err(e.to_string());}};
  let pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Surface,samples,false) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let mask_pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Mask,samples,false) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let cap_pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Cap,samples,false) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let transparent_pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Transparent,samples,false){Ok(p)=>p,Err(e)=>{unsafe{device.destroy_pipeline(cap_pipeline,None);device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let descriptor_layout=match egui_ash_renderer::vulkan::create_vulkan_descriptor_set_layout(&device) {Ok(l)=>l,Err(e)=>{unsafe {device.destroy_pipeline(transparent_pipeline,None);device.destroy_pipeline(cap_pipeline,None);device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::COMBINED_IMAGE_SAMPLER,descriptor_count:4}];let pool=match unsafe {device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(4).pool_sizes(&sizes),None)} {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_descriptor_set_layout(descriptor_layout,None);device.destroy_pipeline(transparent_pipeline,None);device.destroy_pipeline(cap_pipeline,None);device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  let clip_distance=unsafe{context.instance.get_physical_device_features(context.physical_device)}.shader_clip_distance!=0&&std::env::var("INGETRAZO_FORCE_MANUAL_EDGE_CLIP").as_deref()!=Ok("1");
  eprintln!("Native world edge pre-raster clip: shaderClipDistance={clip_distance}; unsupported/forced fallback is direct GPU endpoint clipping");
  let mut world=Self{textures:None,texture_enabled:false,has_texture_sources:false,show_edges:true,edge_vertices:None,edge_count:0,style_mode:ingetrazo_render::style::VisualStyle::ShadedWithTextures,style_front:[1.;4],style_back:[0.62,0.70,0.78,1.],default_back_runs:vec![],consolidated_opaque_count:0,paint_pipelines:[vk::Pipeline::null();10],ordinary_runs:Vec::new(),transparent_vertices:None,opacity_runs:Vec::new(),transparent_pipeline,probe_pressure_after:None,contour_metrics:ContourMetrics::default(),contours:None,contour_batches:Vec::new(),background:[0.90,0.91,0.92,1.],eye:[0.;3],width_factor:0.,display_scale:1.,edge_color:[0.;4],device,budget:budget.clone(),resources:Resources::new(context,budget),render_pass,pipeline,mask_pipeline,cap_pipeline,layout,descriptor_layout,pool,depth_format,samples,target:None,vertices:None,indices:None,count:0,cap_vertices:None,cap_signature:None,cap_key:None,revision:None,mvp:[0.0;16],clip:[0.;4],clip_enabled:0,uploads:0};world.contours=Some(Contours::new(context,render_pass,samples)?);world.textures=Some(super::textures::Textures::new(context,render_pass,samples,world.budget.clone())?);for (i,pass)in [WorldPass::FrontSurface,WorldPass::TransparentFront,WorldPass::TransparentBack,WorldPass::OpaqueBack,WorldPass::DefaultBack,WorldPass::XRaySurface,WorldPass::XRayFront,WorldPass::Edge,WorldPass::DepthOnly,WorldPass::HiddenEdge].into_iter().enumerate(){world.paint_pipelines[i]=make_pipeline(&world.device,render_pass,layout,pass,samples,clip_distance)?;}Ok(world)
 }
 /// Must be called after the previous UI/draw fence completes.
 pub fn resize(&mut self,context:&VulkanContext,renderer:&mut egui_ash_renderer::Renderer,extent:vk::Extent2D)->Result<egui::TextureId,String> {
  if let Some(target)=&self.target {if target.extent==extent {if let Some(texture)=target.texture{return Ok(texture);}}}
  let mut target=self.create_target(context,extent)?;
  // UI primitives may already hold this TextureId. Replace its descriptor
  // only after the complete new target exists; preserve old drawable on error.
  let texture=if let Some(id)=self.target.as_ref().and_then(|t|t.texture){renderer.update_user_texture(id,target.set).map_err(|e|e.to_string())?;id}else{renderer.add_bottom_origin_encoded_opaque_texture(target.set)};target.texture=Some(texture);
  if let Some(old)=self.target.take(){drop(old);}
  self.target=Some(target);Ok(texture)
 }
 /// Headless target uses the identical image/render pass/pipeline owner without egui registration.
 /// Caller must complete the previous draw fence before replacing this target.
 pub fn resize_offscreen(&mut self,context:&VulkanContext,extent:vk::Extent2D)->Result<(),String> {
  if let Some(target)=&self.target {if target.texture.is_some(){return Err("Cannot replace a registered UI target without its renderer".into());}if target.extent==extent{return Ok(());}}
  let target=self.create_target(context,extent)?;self.target=Some(target);Ok(())
 }
 pub fn readback(&mut self,context:&VulkanContext)->Result<Vec<u8>,String> {
  let target=self.target.as_ref().ok_or("No world target for readback")?;
  // GPU attachment has GL-compatible bottom-origin MSAA locations. Only
  // diagnostic host readback changes row order; presentation flips sampler UV.
  let mut rgba=self.resources.readback_image(context,target._color.handle,target.extent)?;
  let row=target.extent.width as usize*4;for y in 0..target.extent.height as usize/2{let opposite=target.extent.height as usize-1-y;let (top,bottom)=rgba.split_at_mut(opposite*row);top[y*row..(y+1)*row].swap_with_slice(&mut bottom[..row]);}Ok(rgba)
 }
 fn create_target(&self,context:&VulkanContext,extent:vk::Extent2D)->Result<Target,String> {
  if extent.width==0||extent.height==0{return Err("Zero viewport extent".into());}
  let color=Image::new(context,self.budget.clone(),extent,vk::Format::R8G8B8A8_UNORM,vk::ImageUsageFlags::COLOR_ATTACHMENT|vk::ImageUsageFlags::SAMPLED|vk::ImageUsageFlags::TRANSFER_SRC,vk::ImageAspectFlags::COLOR)?;
  let depth=Image::new_samples(context,self.budget.clone(),extent,self.depth_format,vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,vk::ImageAspectFlags::DEPTH|vk::ImageAspectFlags::STENCIL,self.samples)?;
  let msaa=if self.samples!=vk::SampleCountFlags::TYPE_1 {Some(Image::new_samples(context,self.budget.clone(),extent,vk::Format::R8G8B8A8_UNORM,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageAspectFlags::COLOR,self.samples)?)}else{None};
  let sampler=unsafe {self.device.create_sampler(&vk::SamplerCreateInfo::default().mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR).address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE).address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),None)}.map_err(|e|e.to_string())?;
  let attachments=if let Some(msaa)=&msaa{vec![msaa.view,color.view,depth.view]}else{vec![color.view,depth.view]};let framebuffer=match unsafe {self.device.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(self.render_pass).attachments(&attachments).width(extent.width).height(extent.height).layers(1),None)} {Ok(f)=>f,Err(e)=>{unsafe {self.device.destroy_sampler(sampler,None)};return Err(e.to_string());}};
  let set=match egui_ash_renderer::vulkan::create_vulkan_descriptor_set(&self.device,self.descriptor_layout,self.pool,color.view,sampler) {Ok(set)=>set,Err(e)=>{unsafe {self.device.destroy_framebuffer(framebuffer,None);self.device.destroy_sampler(sampler,None);}return Err(e.to_string());}};
  Ok(Target{device:self.device.clone(),pool:self.pool,_color:color,_msaa:msaa,_depth:depth,framebuffer,sampler,set,extent,texture:None})
 }

 /// Bounded engineering override exercises chunking on real GPU resources; never raises hardware limits.
 pub fn set_contour_storage_limit_for_probe(&mut self,bytes:u64)->Result<(),String>{if self.revision.is_some(){return Err("Set contour probe limit before model upload".into());}if bytes<36{return Err("Contour limit must hold a triangle".into());}let contours=self.contours.as_mut().unwrap();contours.limit=contours.limit.min(bytes);Ok(())}
 /// Engineering-only dynamic pressure: actual admission is exhausted after this many new chunks.
 /// Used because an initial discrete staging peak can exceed all later compact chunks.
 pub fn set_contour_pressure_after_chunks_for_probe(&mut self,chunks:Option<usize>)->Result<(),String>{if chunks==Some(0){return Err("Pressure control must first allocate a chunk".into());}self.probe_pressure_after=chunks;Ok(())}
 pub fn contour_metrics(&self)->ContourMetrics{self.contour_metrics}
 pub fn contour_batch_count(&self)->usize{self.contour_batches.len()}
 pub fn set_display_scale(&mut self,scale:f32){self.display_scale=scale.max(0.01);}
 pub fn set_scene(&mut self,context:&VulkanContext,scene:&ingetrazo_core::scene::Scene,camera:&ingetrazo_core::camera::OrbitCamera,style:&ingetrazo_render::style::StyleConfig,fence:vk::Fence,completed_serial:u64,epoch:u64)->Result<(),String> {
  let previous=(self.mvp,self.clip,self.clip_enabled,self.eye,self.width_factor,self.edge_color,self.background,self.style_mode,self.style_front,self.style_back,self.show_edges);
  let result=self.prepare_scene(context,scene,camera,style,fence,completed_serial,epoch);
  if result.is_err(){(self.mvp,self.clip,self.clip_enabled,self.eye,self.width_factor,self.edge_color,self.background,self.style_mode,self.style_front,self.style_back,self.show_edges)=previous;}
  result
 }
 fn prepare_scene(&mut self,context:&VulkanContext,scene:&ingetrazo_core::scene::Scene,camera:&ingetrazo_core::camera::OrbitCamera,style:&ingetrazo_render::style::StyleConfig,fence:vk::Fence,completed_serial:u64,epoch:u64)->Result<(),String> {
  let started=std::time::Instant::now();
  self.show_edges=style.show_edges;self.style_mode=style.visual_style;self.style_front=style.front_color;self.style_back=style.back_color;
  self.background=style.background_color;self.background[3]=1.;
  self.eye=camera.eye().as_vec3().to_array();self.width_factor=(camera.fov_deg.to_radians()*0.5).tan() as f32*2.*1.25;self.edge_color=style.edge_color;self.edge_color[3]=1.;
  self.mvp=(camera.projection_matrix()*camera.view_matrix()).as_mat4().to_cols_array();
  self.clip_enabled=0;
  if scene.show_section_cuts {if let Some(plane)=scene.section_planes.iter().find(|p|p.active) {
   self.clip=[plane.normal.x as f32,plane.normal.y as f32,plane.normal.z as f32,-plane.normal.dot(plane.point) as f32];self.clip_enabled=1;
  }}
  let revision=(scene.version,scene.view_version,epoch);
  let cap_enabled=self.clip_enabled!=0&&scene.show_section_fill&&!matches!(style.visual_style,ingetrazo_render::style::VisualStyle::Wireframe|ingetrazo_render::style::VisualStyle::XRay);
  let cap_key=(revision,self.clip,cap_enabled,style.section_fill_color);
  let cap=if self.cap_key==Some(cap_key){self.cap_signature}else if cap_enabled {
   scene.section_planes.iter().position(|p|p.active).and_then(|index|scene.section_frame(index)).map(|frame|{let mut color=style.section_fill_color;color[3]=1.;(frame.map(|p|p.as_vec3().to_array()),color)})
  }else{None};
  let cap_changed=cap!=self.cap_signature;
  let has_texture_sources=if self.revision==Some(revision){self.has_texture_sources}else{scene_has_textures(scene)};
  let texture_enabled=has_texture_sources&&style.show_textures&&style.visual_style==ingetrazo_render::style::VisualStyle::ShadedWithTextures;
  if self.revision==Some(revision)&&self.texture_enabled==texture_enabled{
   if cap_changed {let next=self.upload_cap(context,cap)?;if let Some(old)=self.cap_vertices.take(){self.resources.retire(old)?;self.resources.collect(fence,completed_serial)?;}self.cap_vertices=next;self.cap_signature=cap;}
   self.cap_key=Some(cap_key);self.contour_metrics.cache_hits+=1;self.contour_metrics.warm_cache_ms=started.elapsed().as_secs_f64()*1000.;return Ok(());
  }
  if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("World resource replacement requires completed draw fence".into());}
  let mut plain=style.clone();plain.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;plain.show_textures=texture_enabled;plain.front_color=[0.96,0.95,0.925,1.];
  let data=ingetrazo_render::pipeline::build_world_surfaces(scene,camera,&plain);
  let hard_edges=ingetrazo_render::native_edges::build_hard_edges(scene);let edge_count:u32=hard_edges.positions.len().try_into().map_err(|_|"Too many GPU edge endpoints")?;
  let texture_buffers=data.textured_meshes;
  let transparent=data.transparent_mesh;
  let mesh=data.opaque_mesh;let (colored,runs,front_count)=pack_transparent_with_textures(&transparent,&mesh,&texture_buffers)?;let ordinary_runs=indexed_paint_runs(&mesh)?;let default_back_runs=mesh.source_default_back.iter().map(|&(first,count)|Ok((u32::try_from(first).map_err(|_|"Default-back range overflow")?,u32::try_from(count).map_err(|_|"Default-back count overflow")?))).collect::<Result<Vec<_>,String>>()?;
  let count:u32=mesh.indices.len().try_into().map_err(|_|"Too many GPU indices")?;
  let (vertices,indices)=if mesh.indices.is_empty(){(None,None)}else {
   let vertices=self.resources.upload(context,bytemuck::cast_slice(&mesh.vertices),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;
   let indices=match self.resources.upload(context,bytemuck::cast_slice(&mesh.indices),vk::BufferUsageFlags::INDEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false){Ok(i)=>i,Err(e)=>{self.resources.retire(vertices)?;self.resources.collect(fence,completed_serial)?;return Err(e);}};
   (Some(vertices),Some(indices))
  };
  let mut next_transparent=None;let mut next_edges=None;
  let mut batches=Vec::new();let limit=self.contours.as_ref().unwrap().limit;let mut preprocessing=0.;let mut next_cap=None;let mut next_textures=None;
  let prepare=(||->Result<(),String>{
   if !hard_edges.positions.is_empty(){next_edges=Some(self.resources.upload(context,bytemuck::cast_slice(&hard_edges.positions),vk::BufferUsageFlags::VERTEX_BUFFER,false)?);self.contour_metrics.edge_buffer_uploads+=1;}
   if let(Some(v),Some(i))=(vertices,indices){
    if self.resources.logical_bytes(v).unwrap()<=limit&&self.resources.logical_bytes(i).unwrap()<=limit{batches.push(ContourBatch{vertices:v,indices:i,triangles:count/3,compact:false,stride:12,owned:false});}
    else{upload_contour_chunks(context,&mut self.resources,&mesh,limit,&mut batches,&mut preprocessing,self.probe_pressure_after,&self.budget).map_err(|e|format!("Contour chunk upload failed: {e}"))?;}
   }
   if !colored.is_empty(){let v=self.resources.upload(context,bytemuck::cast_slice(&colored),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;next_transparent=Some(v);
    if front_count>0&&self.resources.logical_bytes(v).unwrap()<=limit {batches.push(ContourBatch{vertices:v,indices:v,triangles:(front_count/3)as u32,compact:true,stride:6,owned:false});}
    else if front_count>0 {upload_contour_chunks(context,&mut self.resources,&transparent,limit,&mut batches,&mut preprocessing,self.probe_pressure_after,&self.budget).map_err(|e|format!("Contour chunk upload failed: {e}"))?;}
   }
   if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Contour cache replacement requires a completed draw fence".into());}
   if cap_changed{next_cap=Some(self.upload_cap(context,cap)?);}
   let candidate=self.textures.as_mut().unwrap().prepare(context,&mut self.resources,&texture_buffers,&scene.embedded_textures,fence,completed_serial,limit)?;
   for &(buffer,triangles,stride)in &candidate.contours{batches.push(ContourBatch{vertices:buffer,indices:buffer,triangles,compact:true,stride,owned:false});}
   next_textures=Some(candidate);
   self.contours.as_mut().unwrap().replace_batches(&self.resources,&batches)
  })();
  if let Err(error)=prepare {if let Some(candidate)=next_textures.take(){self.textures.as_mut().unwrap().abort(&mut self.resources,candidate,fence,completed_serial)?;}self.contour_metrics.failed_prepared_edge_bytes=next_edges.and_then(|id|self.resources.logical_bytes(id)).unwrap_or(0);self.contour_metrics.failed_prepared_chunks=batches.iter().filter(|b|b.owned).count();self.contour_metrics.rollbacks+=1;for id in batches.iter().filter(|batch|batch.owned).map(|batch|batch.vertices).chain([vertices,indices].into_iter().flatten()).chain(next_cap.into_iter().flatten()).chain(next_transparent).chain(next_edges){self.resources.retire(id)?;}self.resources.collect(fence,completed_serial)?;return Err(error);}
  self.textures.as_mut().unwrap().publish(context,&mut self.resources,next_textures.unwrap(),fence,completed_serial)?;
  if let Some(next)=next_cap{if let Some(old)=self.cap_vertices.take(){self.resources.retire(old)?;}self.cap_vertices=next;self.cap_signature=cap;}self.cap_key=Some(cap_key);
  for old in self.contour_batches.iter().filter(|batch|batch.owned).map(|batch|batch.vertices).chain([self.vertices.take(),self.indices.take(),self.transparent_vertices.take(),self.edge_vertices.take()].into_iter().flatten()){self.resources.retire(old)?;}
  self.resources.collect(fence,completed_serial)?;self.contour_metrics.additional_position_bytes=batches.iter().filter(|b|b.owned).map(|b|self.resources.logical_bytes(b.vertices).unwrap()).sum();self.contour_metrics.batches=batches.len();self.contour_metrics.compact_preprocess_ms=preprocessing;self.contour_metrics.cold_build_ms=started.elapsed().as_secs_f64()*1000.;self.contour_batches=batches;
  self.edge_vertices=next_edges;self.edge_count=edge_count;self.transparent_vertices=next_transparent;self.opacity_runs=runs;self.ordinary_runs=ordinary_runs;self.default_back_runs=default_back_runs;self.consolidated_opaque_count=mesh.source_consolidated_opaque_count;self.vertices=vertices;self.indices=indices;self.count=count;self.revision=Some(revision);self.texture_enabled=texture_enabled;self.has_texture_sources=has_texture_sources;self.uploads+=1;Ok(())
 }
 fn upload_cap(&mut self,context:&VulkanContext,cap:Option<([[f32;3];4],[f32;4])>)->Result<Option<BufferId>,String>{
  if let Some((frame,color))=cap{let vertices=[0,1,2,0,2,3].map(|i|ingetrazo_render::buffer::Vertex::new(frame[i],[0.,0.,1.],color,[0.,0.]));Ok(Some(self.resources.upload(context,bytemuck::cast_slice(&vertices),vk::BufferUsageFlags::VERTEX_BUFFER,false)?))}else{Ok(None)}
 }
 /// Cached source opacity buckets; diagnostic observation does not rebuild buffers.
 pub fn transparent_runs(&self)->Vec<(f32,u32,u32)>{self.opacity_runs.iter().filter(|r|r.kind!=3).map(|r|(r.opacity,r.first,r.count)).collect()}
 pub fn paint_runs(&self)->Vec<(u32,f32,u32,u32)>{self.opacity_runs.iter().map(|r|(r.kind,r.opacity,r.first,r.count)).collect()}
 pub fn set_texture_pressure_for_probe(&mut self,enabled:bool){self.textures.as_mut().unwrap().set_pressure_for_probe(enabled);}
 pub fn texture_stats(&self)->(super::textures::Stats,&super::texture_cache::TextureStats){let textures=self.textures.as_ref().unwrap();(textures.stats,textures.cache_stats())}
 pub fn edge_geometry_bytes(&self)->u64{self.edge_vertices.and_then(|id|self.resources.logical_bytes(id)).unwrap_or(0)}
 pub fn edge_endpoint_count(&self)->u32{self.edge_count}
 pub fn style_state(&self)->(ingetrazo_render::style::VisualStyle,[f32;4],[f32;4]){(self.style_mode,self.style_front,self.style_back)}
 pub fn consolidated_opaque_count(&self)->usize{self.consolidated_opaque_count}
 pub fn ordinary_paint_runs(&self)->&[(u32,u32,bool)]{&self.ordinary_runs}
 pub fn transparent_surface_bytes(&self)->u64{self.transparent_vertices.and_then(|id|self.resources.logical_bytes(id)).unwrap_or(0)}
 fn push_face_style(&self,command:vk::CommandBuffer,mode:u32,opacity:f32){unsafe {
  let state=[opacity.to_bits(),mode];self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,84,bytemuck::cast_slice(&state));
 }}
 pub fn record(&mut self,command:vk::CommandBuffer,serial:u64)->Result<(),String> {
  let Some(target)=&self.target else{return Ok(());};
  let mut clears=vec![vk::ClearValue{color:vk::ClearColorValue{float32:self.background}}];
  if self.samples!=vk::SampleCountFlags::TYPE_1{clears.push(vk::ClearValue::default());}
  clears.push(vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.0,stencil:0}});
  unsafe {
   self.device.cmd_begin_render_pass(command,&vk::RenderPassBeginInfo::default().render_pass(self.render_pass).framebuffer(target.framebuffer).render_area(vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}).clear_values(&clears),vk::SubpassContents::INLINE);
   self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.pipeline);
   self.device.cmd_set_viewport(command,0,&[vk::Viewport{x:0.0,y:0.0,width:target.extent.width as f32,height:target.extent.height as f32,min_depth:0.0,max_depth:1.0}]);self.device.cmd_set_scissor(command,0,&[vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}]);
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,bytemuck::cast_slice(&self.mvp));
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,64,bytemuck::cast_slice(&self.clip));
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&self.clip_enabled));
   let colors=[self.style_front,self.style_back];self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,96,bytemuck::cast_slice(&colors));
   let wire=self.style_mode==ingetrazo_render::style::VisualStyle::Wireframe;
   let xray=self.style_mode==ingetrazo_render::style::VisualStyle::XRay;
   let material=matches!(self.style_mode,ingetrazo_render::style::VisualStyle::ShadedWithTextures|ingetrazo_render::style::VisualStyle::XRay);
   let mode=match self.style_mode{ingetrazo_render::style::VisualStyle::HiddenLine=>1u32,ingetrazo_render::style::VisualStyle::Monochrome=>2,_=>0};
   self.push_face_style(command,mode,if xray{0.6}else{1.});
   if !wire {if let (Some(vertices),Some(indices))=(self.vertices,self.indices){self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);let ordinary=self.consolidated_opaque_count>0;
    for &(first,count,cull) in &self.ordinary_runs {if cull&&!ordinary{continue;}self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,if xray{self.paint_pipelines[if cull{6}else{5}]}else if cull{self.paint_pipelines[0]}else{self.pipeline});self.device.cmd_draw_indexed(command,count,1,first,0,0);}self.resources.mark_used(vertices,serial)?;self.resources.mark_used(indices,serial)?;}}
   if !wire&&self.texture_enabled{self.textures.as_mut().unwrap().record(&mut self.resources,command,serial,super::textures::Phase::Opaque,&self.mvp,&self.clip,self.clip_enabled,target.extent.height)?;}
   // Original separate default-back tint, shaded/textured opaque faces only.
   if self.style_mode==ingetrazo_render::style::VisualStyle::ShadedWithTextures {if let (Some(vertices),Some(indices))=(self.vertices,self.indices){self.push_face_style(command,3,1.);self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.paint_pipelines[4]);self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);for &(first,count)in &self.default_back_runs{self.device.cmd_draw_indexed(command,count,1,first,0,0);}}}
   self.push_face_style(command,0,1.);
   // Original opaque back overrides win over front copies before stencil fill.
   if material {if let Some(vertices)=self.transparent_vertices {self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.paint_pipelines[3]);self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);for run in self.opacity_runs.iter().filter(|r|r.kind==3){self.device.cmd_draw(command,run.count,1,run.first,0);}self.resources.mark_used(vertices,serial)?;}}
   if !wire&&self.texture_enabled{self.textures.as_mut().unwrap().record(&mut self.resources,command,serial,super::textures::Phase::BackOpaque,&self.mvp,&self.clip,self.clip_enabled,target.extent.height)?;}
   if self.clip_enabled!=0 {
    if let Some(cap)=self.cap_vertices {
     if let (Some(vertices),Some(indices))=(self.vertices,self.indices) {
     self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.mask_pipeline);
     self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);
     self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);
     for &(first,count,cull)in &self.ordinary_runs{if !cull{self.device.cmd_draw_indexed(command,count,1,first,0,0);}}
     }
     if self.texture_enabled{self.textures.as_mut().unwrap().record(&mut self.resources,command,serial,super::textures::Phase::Mask,&self.mvp,&self.clip,self.clip_enabled,target.extent.height)?;}
     self.push_face_style(command,0,1.);
     self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.cap_pipeline);
     let disabled=0u32;self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&disabled));
     self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(cap).unwrap()],&[0]);self.device.cmd_draw(command,6,1,0,0);self.resources.mark_used(cap,serial)?;
    }
   }
   // Original viewport.py1800-1845: both color sides precede both texture sides.
   for back in [false,true]{
    if material {if let Some(vertices)=self.transparent_vertices {
     self.push_face_style(command,0,1.);
     self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&self.clip_enabled));
     self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);
     for run in self.opacity_runs.iter().filter(|r|r.kind!=3&&(r.kind==2)==back){self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,match run.kind{1=>self.paint_pipelines[1],2=>self.paint_pipelines[2],_=>self.transparent_pipeline});self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,84,bytemuck::bytes_of(&run.opacity));self.device.cmd_draw(command,run.count,1,run.first,0);}
     self.resources.mark_used(vertices,serial)?;
    }}

   }
   if self.texture_enabled{for phase in [super::textures::Phase::TransparentFront,super::textures::Phase::TransparentBack]{self.textures.as_mut().unwrap().record(&mut self.resources,command,serial,phase,&self.mvp,&self.clip,self.clip_enabled,target.extent.height)?;}}
   if self.show_edges||wire {if let Some(vertices)=self.edge_vertices {
    if xray {
     // Source _xray_face_depth excludes colored transparent/front-only glass.
     // Reuse cached opaque runs; no scene copy, per-frame sort or CPU rebuild.
     self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&self.clip_enabled));
     if let(Some(v),Some(i))=(self.vertices,self.indices){self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.paint_pipelines[8]);self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(v).unwrap()],&[0]);self.device.cmd_bind_index_buffer(command,self.resources.buffer(i).unwrap(),0,vk::IndexType::UINT32);for &(first,count,cull)in &self.ordinary_runs{if !cull{self.device.cmd_draw_indexed(command,count,1,first,0,0);}}self.resources.mark_used(v,serial)?;self.resources.mark_used(i,serial)?;}
     let faded=[0,1,2].map(|i|self.edge_color[i]+(self.background[i]-self.edge_color[i])*0.6);let faded=[faded[0],faded[1],faded[2],1.];self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.paint_pipelines[9]);self.push_face_style(command,1,1.);self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,96,bytemuck::cast_slice(&faded));self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_draw(command,2,self.edge_count/2,0,0);
    }
    self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.paint_pipelines[7]);self.push_face_style(command,1,1.);self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,96,bytemuck::cast_slice(&self.edge_color));self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&self.clip_enabled));self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_draw(command,2,self.edge_count/2,0,0);self.resources.mark_used(vertices,serial)?;
   }}
   if xray {self.device.cmd_clear_attachments(command,&[vk::ClearAttachment::default().aspect_mask(vk::ImageAspectFlags::DEPTH).clear_value(vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.,stencil:0}})],&[vk::ClearRect::default().rect(vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}).base_array_layer(0).layer_count(1)]);}
   if self.clip_enabled!=0 {
    let contours=self.contours.as_ref().unwrap();let mut push=[0u8;128];push[..64].copy_from_slice(bytemuck::cast_slice(&self.mvp));push[64..80].copy_from_slice(bytemuck::cast_slice(&self.clip));push[96..108].copy_from_slice(bytemuck::cast_slice(&self.eye));
    let factor=self.width_factor*self.display_scale/target.extent.height as f32;push[108..112].copy_from_slice(bytemuck::bytes_of(&factor));push[112..128].copy_from_slice(bytemuck::cast_slice(&self.edge_color));
    self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,contours.pipeline);self.device.cmd_push_constants(command,contours.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);
    for (batch,set) in self.contour_batches.iter().zip(&contours.sets){
     let mode=[batch.stride,u32::from(batch.compact)];self.device.cmd_push_constants(command,contours.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,84,bytemuck::cast_slice(&mode));
     self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,contours.layout,0,&[*set],&[]);self.device.cmd_draw(command,6,batch.triangles,0,0);self.resources.mark_used(batch.vertices,serial)?;if batch.indices!=batch.vertices{self.resources.mark_used(batch.indices,serial)?;}
    }
   }
   self.device.cmd_end_render_pass(command);
  }
  Ok(())
 }
}
impl Drop for World {fn drop(&mut self){let _=unsafe {self.device.device_wait_idle()};drop(self.target.take());drop(self.contours.take());drop(self.textures.take());unsafe {for pipeline in self.paint_pipelines{self.device.destroy_pipeline(pipeline,None);}self.device.destroy_pipeline(self.transparent_pipeline,None);self.device.destroy_pipeline(self.cap_pipeline,None);self.device.destroy_pipeline(self.mask_pipeline,None);self.device.destroy_pipeline(self.pipeline,None);self.device.destroy_pipeline_layout(self.layout,None);self.device.destroy_render_pass(self.render_pass,None);self.device.destroy_descriptor_pool(self.pool,None);self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}
#[derive(Clone,Copy)]enum WorldPass {Surface,Mask,Cap,Contour,Transparent,FrontSurface,TransparentFront,TransparentBack,OpaqueBack,DefaultBack,XRaySurface,XRayFront,Edge,DepthOnly,HiddenEdge}
fn make_pipeline(device:&Device,render_pass:vk::RenderPass,layout:vk::PipelineLayout,pass:WorldPass,samples:vk::SampleCountFlags,clip_distance:bool)->Result<vk::Pipeline,String> {
 let code=|bytes:&[u8]|ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string());
 let vertex_code=code(if matches!(pass,WorldPass::Edge|WorldPass::HiddenEdge){if clip_distance{include_bytes!("shaders/edge.vert.spv")}else{include_bytes!("shaders/edge_manual.vert.spv")}}else if matches!(pass,WorldPass::Contour){include_bytes!("shaders/contour.vert.spv")}else{include_bytes!("shaders/world.vert.spv")})?;
 let fragment_code=code(if matches!(pass,WorldPass::Edge|WorldPass::HiddenEdge){include_bytes!("shaders/edge.frag.spv")}else if matches!(pass,WorldPass::Contour){include_bytes!("shaders/contour.frag.spv")}else if matches!(pass,WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack){include_bytes!("shaders/transparent.frag.spv")}else{include_bytes!("shaders/world.frag.spv")})?;
 let vertex=unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&vertex_code),None)}.map_err(|e|e.to_string())?;
 let fragment=match unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&fragment_code),None)} {Ok(m)=>m,Err(e)=>{unsafe {device.destroy_shader_module(vertex,None)};return Err(e.to_string());}};
 let stages=[vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vertex).name(c"main"),vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main")];
 let binding=[vk::VertexInputBindingDescription{binding:0,stride:48,input_rate:vk::VertexInputRate::VERTEX}];let attrs=[vk::VertexInputAttributeDescription{location:0,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:0},vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:12},vk::VertexInputAttributeDescription{location:2,binding:0,format:vk::Format::R32G32B32A32_SFLOAT,offset:24},vk::VertexInputAttributeDescription{location:3,binding:0,format:vk::Format::R32G32_SFLOAT,offset:40}];
 let colored_binding=[vk::VertexInputBindingDescription{binding:0,stride:24,input_rate:vk::VertexInputRate::VERTEX}];let colored_attrs=[attrs[0],vk::VertexInputAttributeDescription{location:2,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:12}];
 let edge_binding=[vk::VertexInputBindingDescription{binding:0,stride:24,input_rate:vk::VertexInputRate::INSTANCE}];let edge_attrs=[attrs[0],vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:12}];
 let input=if matches!(pass,WorldPass::Edge|WorldPass::HiddenEdge){vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&edge_binding).vertex_attribute_descriptions(&edge_attrs)}else if matches!(pass,WorldPass::Contour){vk::PipelineVertexInputStateCreateInfo::default()}else if matches!(pass,WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::OpaqueBack){vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&colored_binding).vertex_attribute_descriptions(&colored_attrs)}else{vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs)};let assembly=vk::PipelineInputAssemblyStateCreateInfo::default().topology(if matches!(pass,WorldPass::Edge|WorldPass::HiddenEdge){vk::PrimitiveTopology::LINE_LIST}else{vk::PrimitiveTopology::TRIANGLE_LIST});let viewport=vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);let raster=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.0).front_face(vk::FrontFace::CLOCKWISE).cull_mode(if matches!(pass,WorldPass::Mask|WorldPass::TransparentBack|WorldPass::OpaqueBack|WorldPass::DefaultBack){vk::CullModeFlags::FRONT}else if matches!(pass,WorldPass::FrontSurface|WorldPass::TransparentFront|WorldPass::XRayFront){vk::CullModeFlags::BACK}else{vk::CullModeFlags::NONE}).depth_bias_enable(matches!(pass,WorldPass::Cap|WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::FrontSurface|WorldPass::Surface|WorldPass::DefaultBack|WorldPass::XRaySurface|WorldPass::XRayFront|WorldPass::DepthOnly)).depth_bias_constant_factor(if matches!(pass,WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::FrontSurface|WorldPass::Surface|WorldPass::DefaultBack|WorldPass::XRaySurface|WorldPass::XRayFront|WorldPass::DepthOnly){1.0}else{2.0}).depth_bias_slope_factor(if matches!(pass,WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::FrontSurface|WorldPass::Surface|WorldPass::DefaultBack|WorldPass::XRaySurface|WorldPass::XRayFront|WorldPass::DepthOnly){1.0}else{2.0});let multisample=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);let stencil=vk::StencilOpState::default().fail_op(vk::StencilOp::KEEP).depth_fail_op(vk::StencilOp::KEEP).pass_op(if matches!(pass,WorldPass::Mask){vk::StencilOp::REPLACE}else{vk::StencilOp::KEEP}).compare_op(if matches!(pass,WorldPass::Cap){vk::CompareOp::EQUAL}else{vk::CompareOp::ALWAYS}).compare_mask(0xff).write_mask(if matches!(pass,WorldPass::Mask){0xff}else{0}).reference(1);
 let depth=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(!matches!(pass,WorldPass::Mask|WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::XRaySurface|WorldPass::XRayFront|WorldPass::HiddenEdge)).depth_compare_op(if matches!(pass,WorldPass::HiddenEdge){vk::CompareOp::GREATER}else{vk::CompareOp::LESS_OR_EQUAL}).stencil_test_enable(matches!(pass,WorldPass::Mask|WorldPass::Cap)).front(stencil).back(stencil);let colors=[vk::PipelineColorBlendAttachmentState::default().blend_enable(matches!(pass,WorldPass::Transparent|WorldPass::TransparentFront|WorldPass::TransparentBack|WorldPass::XRaySurface|WorldPass::XRayFront)).src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).color_blend_op(vk::BlendOp::ADD).src_alpha_blend_factor(vk::BlendFactor::ZERO).dst_alpha_blend_factor(vk::BlendFactor::ONE).alpha_blend_op(vk::BlendOp::ADD).color_write_mask(if matches!(pass,WorldPass::Mask|WorldPass::DepthOnly){vk::ColorComponentFlags::empty()}else{vk::ColorComponentFlags::RGBA})];let blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR]);
 let result=unsafe {device.create_graphics_pipelines(vk::PipelineCache::null(),&[vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&input).input_assembly_state(&assembly).viewport_state(&viewport).rasterization_state(&raster).multisample_state(&multisample).depth_stencil_state(&depth).color_blend_state(&blend).dynamic_state(&dynamic).layout(layout).render_pass(render_pass)],None)};
 unsafe {device.destroy_shader_module(vertex,None);device.destroy_shader_module(fragment,None);}
 match result {Ok(p)=>Ok(p[0]),Err((pipelines,e))=>{release_partial_pipelines(pipelines,|p|unsafe {device.destroy_pipeline(p,None)});Err(e.to_string())}}

}

fn release_partial_pipelines(pipelines:Vec<vk::Pipeline>,mut destroy:impl FnMut(vk::Pipeline)) {
 for pipeline in pipelines {if pipeline!=vk::Pipeline::null(){destroy(pipeline);}}
}
/// Engineering probe: inject only view creation failure after actual allocation/binding.
/// The image and memory allocation still run through the production owner and ledger.
pub fn verify_image_view_failure_rollback(context:&VulkanContext,budget:Arc<VulkanBudget>)->Result<(),String> {
 let before=budget.ledger.lock().unwrap().total_committed();
 let result=Image::new_with_view(context,budget.clone(),vk::Extent2D{width:2,height:2},vk::Format::R8G8B8A8_UNORM,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageAspectFlags::COLOR,vk::SampleCountFlags::TYPE_1,|_,_|Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY));
 if result.is_ok(){return Err("Injected view failure unexpectedly succeeded".into());}
 let ledger=budget.ledger.lock().unwrap();
 if ledger.total_committed()!=before||ledger.heaps.iter().any(|h|h.reserved!=0){return Err("Image view failure leaked GPU allocation".into());}
 Ok(())
}
#[cfg(test)]mod lifecycle_regression {
 use super::*;use ash::vk::Handle;
 #[test]fn failed_pipeline_batch_releases_every_partial_non_null_handle(){
  let a=vk::Pipeline::from_raw(42);let b=vk::Pipeline::from_raw(43);let mut destroyed=vec![];
  release_partial_pipelines(vec![a,vk::Pipeline::null(),b],|p|destroyed.push(p));
  assert_eq!(destroyed,vec![a,b]);
 }
}

fn select_sample_count(global:vk::SampleCountFlags,color:vk::SampleCountFlags,depth:vk::SampleCountFlags)->Result<vk::SampleCountFlags,String> {
 let common=global&color&depth;
 [vk::SampleCountFlags::TYPE_4,vk::SampleCountFlags::TYPE_2,vk::SampleCountFlags::TYPE_1].into_iter().find(|samples|common.contains(*samples)).ok_or("No compatible world color/depth/stencil sample count".into())
}
#[cfg(test)]mod sample_count_regression {
 use super::*;
 #[test]fn format_support_limits_global_msaa_without_allocating_invalid_images(){
  let all=vk::SampleCountFlags::TYPE_1|vk::SampleCountFlags::TYPE_2|vk::SampleCountFlags::TYPE_4;
  assert_eq!(select_sample_count(all,all,vk::SampleCountFlags::TYPE_1|vk::SampleCountFlags::TYPE_2).unwrap(),vk::SampleCountFlags::TYPE_2);
  assert_eq!(select_sample_count(all,vk::SampleCountFlags::TYPE_1,all).unwrap(),vk::SampleCountFlags::TYPE_1);
  assert!(select_sample_count(all,vk::SampleCountFlags::TYPE_4,vk::SampleCountFlags::TYPE_2).is_err());
 }
}

// Only oversized opaque geometry needs an extra position copy. Transparent contours never need
// normal, color, or UV data. Temporary CPU storage is bounded to one <=8 MiB chunk.
fn upload_contour_chunks(context:&VulkanContext,resources:&mut Resources,mesh:&ingetrazo_render::buffer::MeshRenderBuffer,limit:u64,batches:&mut Vec<ContourBatch>,preprocess_ms:&mut f64,pressure_after:Option<usize>,budget:&Arc<VulkanBudget>)->Result<(),String>{
 let triangles=contour_chunk_triangles(limit)?;
 let mut chunks=mesh.indices.chunks(triangles*3).peekable();
 while chunks.peek().is_some(){
  let owned=batches.iter().filter(|batch|batch.owned).count();
  let mut positions=Vec::<Vec<[f32;3]>>::new();let mut bytes=0usize;
  while let Some(chunk)=chunks.peek(){
   let next=chunk.len()*12;
   if !positions.is_empty()&&bytes+next>8*1024*1024{break;}
   let started=std::time::Instant::now();let chunk=chunks.next().unwrap();
   positions.push(chunk.iter().map(|&index|mesh.vertices[index as usize].position).collect());
   *preprocess_ms+=started.elapsed().as_secs_f64()*1000.;bytes+=next;
   // Preserve the explicit engineering admission boundary after N completed chunks.
   if pressure_after==Some(owned+positions.len()){break;}
  }
  let inputs=positions.iter().map(|points|(bytemuck::cast_slice(points.as_slice()),vk::BufferUsageFlags::STORAGE_BUFFER)).collect::<Vec<_>>();
  let _pressure=if pressure_after==Some(owned){let mut ledger=budget.ledger.lock().unwrap();let previous=ledger.total_limit;ledger.total_limit=Some(ledger.total_committed());Some(ProbeBudgetRestore{budget:budget.clone(),previous})}else{None};
  let ids=resources.upload_many(context,&inputs)?;
  for (id,points) in ids.into_iter().zip(positions){batches.push(ContourBatch{vertices:id,indices:id,triangles:(points.len()/3)as u32,compact:true,stride:3,owned:true});}
 }Ok(())
}
fn contour_chunk_triangles(limit:u64)->Result<usize,String>{
 let triangles=limit.min(8*1024*1024)/36;if triangles==0{return Err("Device storage range cannot hold one contour triangle".into());}Ok(triangles as usize)
}
#[cfg(test)]mod contour_chunk_regression{use super::*;
 #[test]fn storage_chunks_fit_actual_range_and_preserve_complete_triangles(){assert!(contour_chunk_triangles(35).is_err());assert_eq!(contour_chunk_triangles(36).unwrap(),1);assert_eq!(contour_chunk_triangles(107).unwrap(),2);assert!(contour_chunk_triangles(u64::MAX).unwrap()*36<=8*1024*1024);}
}

struct ProbeBudgetRestore{budget:Arc<VulkanBudget>,previous:Option<u64>}
impl Drop for ProbeBudgetRestore{fn drop(&mut self){self.budget.ledger.lock().unwrap().total_limit=self.previous;}}

// Original viewport.py _tcol_runs: sorted (round(opacity,3), front_cull) buckets,
// traversal order inside each bucket. This first untextured front-paint path has
// front_cull=false; back paint and textured runs remain separate acceptance gates.
// Packed positions/RGB are also borrowed by the contour SSBO (stride6).
fn opacity_bucket(alpha:f64)->Result<i32,String>{if !alpha.is_finite(){return Err("Nonfinite source opacity".into());}let rounded=format!("{alpha:.3}").parse::<f64>().map_err(|_|"Invalid source opacity")?;Ok((rounded*1000.).round()as i32)}
fn quantized_rgb(color:[f32;3])->[f32;3]{color.map(|c|(c.clamp(0.,1.)*64.).round_ties_even()/64.)}
#[cfg(test)]fn pack_transparent(mesh:&ingetrazo_render::buffer::MeshRenderBuffer,opaque:&ingetrazo_render::buffer::MeshRenderBuffer)->Result<(Vec<ColoredVertex>,Vec<OpacityRun>,usize),String>{
 pack_transparent_with_textures(mesh,opaque,&ingetrazo_render::buffer::TextureBuffers::default())
}
fn pack_transparent_with_textures(mesh:&ingetrazo_render::buffer::MeshRenderBuffer,opaque:&ingetrazo_render::buffer::MeshRenderBuffer,textures:&ingetrazo_render::buffer::TextureBuffers)->Result<(Vec<ColoredVertex>,Vec<OpacityRun>,usize),String>{
 let mut buckets=std::collections::BTreeMap::<(i32,bool),Vec<ColoredVertex>>::new();let mut span=0;let mut cull_span=0;let mut previous_alpha=None;let mut key=0;
 for (triangle_index,triangle)in mesh.indices.chunks_exact(3).enumerate(){let offset=triangle_index*3;
  while span+1<mesh.source_opacity_runs.len()&&mesh.source_opacity_runs[span+1].0<=offset{span+=1;}
  while cull_span<mesh.source_front_cull.len()&&mesh.source_front_cull[cull_span].0+mesh.source_front_cull[cull_span].1<=offset{cull_span+=1;}
  let cull=mesh.source_front_cull.get(cull_span).is_some_and(|r|r.0<=offset&&offset<r.0+r.1);
  let alpha=mesh.source_opacity_runs.get(span).filter(|r|r.0<=offset).map(|r|r.1).unwrap_or(mesh.vertices[triangle[0]as usize].color[3]as f64);
  if previous_alpha!=Some(alpha){key=opacity_bucket(alpha)?;previous_alpha=Some(alpha);}let bucket=buckets.entry((key,cull)).or_default();
  for &index in triangle{let v=&mesh.vertices[index as usize];bucket.push(ColoredVertex{position:v.position,color:quantized_rgb([v.color[0],v.color[1],v.color[2]])});}
 }
 let mut vertices=Vec::new();let mut runs=Vec::new();for ((opacity,cull),bucket)in buckets{append_paint_run(&mut vertices,&mut runs,bucket,opacity,u32::from(cull))?;}
 let front_count=vertices.len();let mut back_buckets=std::collections::BTreeMap::<i32,Vec<ColoredVertex>>::new();let mut opaque_back=Vec::new();
 let mut paints=Vec::new();
 for source in [opaque,mesh].into_iter().chain(textures.values()){for paint in &source.source_back_paints{paints.push((source,paint));}}
 paints.sort_by_key(|(_,paint)|paint.order);
 for (source,paint) in paints{let key=opacity_bucket(paint.opacity)?;let bucket=if paint.opacity<0.999{back_buckets.entry(key).or_default()}else{&mut opaque_back};let color=quantized_rgb(paint.color);for &index in &source.indices[paint.first..paint.first+paint.count]{bucket.push(ColoredVertex{position:source.vertices[index as usize].position,color});}}

 if !opaque_back.is_empty(){append_paint_run(&mut vertices,&mut runs,opaque_back,1000,3)?;}
 for (opacity,bucket)in back_buckets{append_paint_run(&mut vertices,&mut runs,bucket,opacity,2)?;}
 Ok((vertices,runs,front_count))
}
fn append_paint_run(vertices:&mut Vec<ColoredVertex>,runs:&mut Vec<OpacityRun>,bucket:Vec<ColoredVertex>,opacity:i32,kind:u32)->Result<(),String>{let first=vertices.len().try_into().map_err(|_|"Too many paint vertices")?;let count=bucket.len().try_into().map_err(|_|"Too many paint vertices")?;runs.push(OpacityRun{first,count,opacity:opacity as f32/1000.,kind});vertices.extend(bucket);Ok(())}
fn indexed_paint_runs(mesh:&ingetrazo_render::buffer::MeshRenderBuffer)->Result<Vec<(u32,u32,bool)>,String>{let mut result=Vec::new();let mut first=0;for &(start,count)in &mesh.source_front_cull{if start>first{result.push((first.try_into().map_err(|_|"Too many opaque paint indices")?,(start-first).try_into().map_err(|_|"Too many opaque paint indices")?,false));}result.push((start.try_into().map_err(|_|"Too many opaque paint indices")?,count.try_into().map_err(|_|"Too many opaque paint indices")?,true));first=start+count;}if first<mesh.indices.len(){result.push((first.try_into().map_err(|_|"Too many opaque paint indices")?,(mesh.indices.len()-first).try_into().map_err(|_|"Too many opaque paint indices")?,false));}Ok(result)}

fn scene_has_textures(scene:&ingetrazo_core::scene::Scene)->bool{
 fn mesh(mesh:&ingetrazo_core::mesh::Mesh)->bool{mesh.faces.iter().any(|f|f.attrs.get("texture").is_some_and(|v|v.is_object())||f.attrs.get("back").and_then(|v|v.get("texture")).is_some_and(|v|v.is_object()))}
 fn group(g:&ingetrazo_core::groups::Group)->bool{mesh(&g.mesh)||g.native_document.get("material").and_then(|v|v.get("texture")).is_some_and(|v|v.is_object())||g.children.iter().any(group)}
 mesh(&scene.mesh)||scene.groups.iter().any(group)
}
