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
#[derive(Default,Clone,Copy,Debug)]pub struct ContourMetrics {pub additional_position_bytes:u64,pub batches:usize,pub cold_build_ms:f64,pub compact_preprocess_ms:f64,pub warm_cache_ms:f64,pub cache_hits:u64,pub failed_prepared_chunks:usize,pub rollbacks:u64}
#[derive(Clone,Copy)]struct ContourBatch {vertices:BufferId,indices:BufferId,triangles:u32,compact:bool,owned:bool}
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
  result.pipeline=make_pipeline(&result.device,render_pass,result.layout,WorldPass::Contour,samples)?;Ok(result)
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
pub struct World {probe_pressure_after:Option<usize>,contour_metrics:ContourMetrics,contours:Option<Contours>,contour_batches:Vec<ContourBatch>,eye:[f32;3],width_factor:f32,display_scale:f32,edge_color:[f32;4],device:Device,budget:Arc<VulkanBudget>,resources:Resources,render_pass:vk::RenderPass,pipeline:vk::Pipeline,mask_pipeline:vk::Pipeline,cap_pipeline:vk::Pipeline,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pool:vk::DescriptorPool,depth_format:vk::Format,samples:vk::SampleCountFlags,target:Option<Target>,vertices:Option<BufferId>,indices:Option<BufferId>,count:u32,cap_vertices:Option<BufferId>,cap_signature:Option<([[f32;3];4],[f32;4])>,cap_key:Option<((u64,u64,u64),[f32;4],bool,[f32;4])>,revision:Option<(u64,u64,u64)>,surface_color:Option<[f32;4]>,mvp:[f32;16],clip:[f32;4],clip_enabled:u32,pub uploads:u64}
impl World {
 pub fn new(context:&VulkanContext,budget:Arc<VulkanBudget>)->Result<Self,String> {
  let device=context.device.clone();
  let depth_format=[vk::Format::D24_UNORM_S8_UINT,vk::Format::D32_SFLOAT_S8_UINT].into_iter().find(|f|unsafe {context.instance.get_physical_device_format_properties(context.physical_device,*f)}.optimal_tiling_features.contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)).ok_or("No supported depth/stencil format")?;
  let limits=unsafe {context.instance.get_physical_device_properties(context.physical_device)}.limits;
  let counts=limits.framebuffer_color_sample_counts&limits.framebuffer_depth_sample_counts&limits.framebuffer_stencil_sample_counts;
  let color_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,vk::Format::R8G8B8A8_SRGB,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World color image format unavailable: {e}"))?.sample_counts;
  let depth_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,depth_format,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World depth/stencil image format unavailable: {e}"))?.sample_counts;
  let resolve_samples=unsafe {context.instance.get_physical_device_image_format_properties(context.physical_device,vk::Format::R8G8B8A8_SRGB,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,vk::ImageUsageFlags::COLOR_ATTACHMENT|vk::ImageUsageFlags::SAMPLED|vk::ImageUsageFlags::TRANSFER_SRC,vk::ImageCreateFlags::empty())}.map_err(|e|format!("World sampled resolve format unavailable: {e}"))?.sample_counts;
  if !resolve_samples.contains(vk::SampleCountFlags::TYPE_1){return Err("World resolve image does not support one sample".into());}
  let samples=select_sample_count(counts,color_samples,depth_samples)?;
  eprintln!("Native world MSAA: {:?} (original default4; unsupported fallback remains explicit)",samples);
  let multisampled=samples!=vk::SampleCountFlags::TYPE_1;
  let mut attachments=vec![vk::AttachmentDescription::default().format(vk::Format::R8G8B8A8_SRGB).samples(samples).load_op(vk::AttachmentLoadOp::CLEAR).store_op(if multisampled{vk::AttachmentStoreOp::DONT_CARE}else{vk::AttachmentStoreOp::STORE}).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(if multisampled{vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}else{vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL})];
  if multisampled {attachments.push(vk::AttachmentDescription::default().format(vk::Format::R8G8B8A8_SRGB).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::DONT_CARE).store_op(vk::AttachmentStoreOp::STORE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL));}
  let depth_index=attachments.len() as u32;
  attachments.push(vk::AttachmentDescription::default().format(depth_format).samples(samples).load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::DONT_CARE).stencil_load_op(vk::AttachmentLoadOp::CLEAR).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL));
  let colors=[vk::AttachmentReference{attachment:0,layout:vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}];let depth=vk::AttachmentReference{attachment:depth_index,layout:vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL};
  let resolve=[vk::AttachmentReference{attachment:1,layout:vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}];
  let subpass=vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colors).depth_stencil_attachment(&depth);
  let subpasses=[if multisampled{subpass.resolve_attachments(&resolve)}else{subpass}];
  let dependencies=[vk::SubpassDependency::default().src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0).src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT|vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS).src_access_mask(vk::AccessFlags::SHADER_READ).dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE|vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE),vk::SubpassDependency::default().src_subpass(0).dst_subpass(vk::SUBPASS_EXTERNAL).src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT).dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)];
  let render_pass=unsafe {device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses).dependencies(&dependencies),None)}.map_err(|e|e.to_string())?;
  let push=[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT).size(96)];let layout=match unsafe {device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push),None)} {Ok(layout)=>layout,Err(e)=>{unsafe {device.destroy_render_pass(render_pass,None)};return Err(e.to_string());}};
  let pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Surface,samples) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let mask_pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Mask,samples) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let cap_pipeline=match make_pipeline(&device,render_pass,layout,WorldPass::Cap,samples) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let descriptor_layout=match egui_ash_renderer::vulkan::create_vulkan_descriptor_set_layout(&device) {Ok(l)=>l,Err(e)=>{unsafe {device.destroy_pipeline(cap_pipeline,None);device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::COMBINED_IMAGE_SAMPLER,descriptor_count:4}];let pool=match unsafe {device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(4).pool_sizes(&sizes),None)} {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_descriptor_set_layout(descriptor_layout,None);device.destroy_pipeline(cap_pipeline,None);device.destroy_pipeline(mask_pipeline,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  let mut world=Self{probe_pressure_after:None,contour_metrics:ContourMetrics::default(),contours:None,contour_batches:Vec::new(),eye:[0.;3],width_factor:0.,display_scale:1.,edge_color:[0.;4],device,budget:budget.clone(),resources:Resources::new(context,budget),render_pass,pipeline,mask_pipeline,cap_pipeline,layout,descriptor_layout,pool,depth_format,samples,target:None,vertices:None,indices:None,count:0,cap_vertices:None,cap_signature:None,cap_key:None,revision:None,surface_color:None,mvp:[0.0;16],clip:[0.;4],clip_enabled:0,uploads:0};world.contours=Some(Contours::new(context,render_pass,samples)?);Ok(world)
 }
 /// Must be called after the previous UI/draw fence completes.
 pub fn resize(&mut self,context:&VulkanContext,renderer:&mut egui_ash_renderer::Renderer,extent:vk::Extent2D)->Result<egui::TextureId,String> {
  if let Some(target)=&self.target {if target.extent==extent {if let Some(texture)=target.texture{return Ok(texture);}}}
  let mut target=self.create_target(context,extent)?;
  let texture=renderer.add_user_texture(target.set);target.texture=Some(texture);
  if let Some(old)=self.target.take(){if let Some(texture)=old.texture{renderer.remove_user_texture(texture);}drop(old);}
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
  self.resources.readback_image(context,target._color.handle,target.extent)
 }
 fn create_target(&self,context:&VulkanContext,extent:vk::Extent2D)->Result<Target,String> {
  if extent.width==0||extent.height==0{return Err("Zero viewport extent".into());}
  let color=Image::new(context,self.budget.clone(),extent,vk::Format::R8G8B8A8_SRGB,vk::ImageUsageFlags::COLOR_ATTACHMENT|vk::ImageUsageFlags::SAMPLED|vk::ImageUsageFlags::TRANSFER_SRC,vk::ImageAspectFlags::COLOR)?;
  let depth=Image::new_samples(context,self.budget.clone(),extent,self.depth_format,vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,vk::ImageAspectFlags::DEPTH|vk::ImageAspectFlags::STENCIL,self.samples)?;
  let msaa=if self.samples!=vk::SampleCountFlags::TYPE_1 {Some(Image::new_samples(context,self.budget.clone(),extent,vk::Format::R8G8B8A8_SRGB,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageAspectFlags::COLOR,self.samples)?)}else{None};
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
  let previous=(self.mvp,self.clip,self.clip_enabled,self.eye,self.width_factor,self.edge_color);
  let result=self.prepare_scene(context,scene,camera,style,fence,completed_serial,epoch);
  if result.is_err(){(self.mvp,self.clip,self.clip_enabled,self.eye,self.width_factor,self.edge_color)=previous;}
  result
 }
 fn prepare_scene(&mut self,context:&VulkanContext,scene:&ingetrazo_core::scene::Scene,camera:&ingetrazo_core::camera::OrbitCamera,style:&ingetrazo_render::style::StyleConfig,fence:vk::Fence,completed_serial:u64,epoch:u64)->Result<(),String> {
  let started=std::time::Instant::now();
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
  if self.revision==Some(revision)&&self.surface_color==Some(style.front_color){
   if cap_changed {let next=self.upload_cap(context,cap)?;if let Some(old)=self.cap_vertices.take(){self.resources.retire(old)?;self.resources.collect(fence,completed_serial)?;}self.cap_vertices=next;self.cap_signature=cap;}
   self.cap_key=Some(cap_key);self.contour_metrics.cache_hits+=1;self.contour_metrics.warm_cache_ms=started.elapsed().as_secs_f64()*1000.;return Ok(());
  }
  let mut plain=style.clone();plain.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;plain.show_textures=false;
  let data=ingetrazo_render::pipeline::build_world_surfaces(scene,camera,&plain);
  let transparent=data.transparent_mesh;
  let mesh=data.opaque_mesh;
  let count:u32=mesh.indices.len().try_into().map_err(|_|"Too many GPU indices")?;
  let (vertices,indices)=if mesh.indices.is_empty(){(None,None)}else {
   let vertices=self.resources.upload(context,bytemuck::cast_slice(&mesh.vertices),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;
   let indices=match self.resources.upload(context,bytemuck::cast_slice(&mesh.indices),vk::BufferUsageFlags::INDEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false){Ok(i)=>i,Err(e)=>{self.resources.retire(vertices)?;self.resources.collect(fence,completed_serial)?;return Err(e);}};
   (Some(vertices),Some(indices))
  };
  let mut batches=Vec::new();let limit=self.contours.as_ref().unwrap().limit;let mut preprocessing=0.;let mut next_cap=None;
  let prepare=(||->Result<(),String>{
   if let(Some(v),Some(i))=(vertices,indices){
    if self.resources.logical_bytes(v).unwrap()<=limit&&self.resources.logical_bytes(i).unwrap()<=limit{batches.push(ContourBatch{vertices:v,indices:i,triangles:count/3,compact:false,owned:false});}
    else{upload_contour_chunks(context,&mut self.resources,&mesh,limit,&mut batches,&mut preprocessing,self.probe_pressure_after,&self.budget).map_err(|e|format!("Contour chunk upload failed: {e}"))?;}
   }
   upload_contour_chunks(context,&mut self.resources,&transparent,limit,&mut batches,&mut preprocessing,self.probe_pressure_after,&self.budget).map_err(|e|format!("Contour chunk upload failed: {e}"))?;
   if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Contour cache replacement requires a completed draw fence".into());}
   if cap_changed{next_cap=Some(self.upload_cap(context,cap)?);}
   self.contours.as_mut().unwrap().replace_batches(&self.resources,&batches)
  })();
  if let Err(error)=prepare {self.contour_metrics.failed_prepared_chunks=batches.iter().filter(|b|b.owned).count();self.contour_metrics.rollbacks+=1;for id in batches.iter().filter(|batch|batch.owned).map(|batch|batch.vertices).chain([vertices,indices].into_iter().flatten()).chain(next_cap.into_iter().flatten()){self.resources.retire(id)?;}self.resources.collect(fence,completed_serial)?;return Err(error);}
  if let Some(next)=next_cap{if let Some(old)=self.cap_vertices.take(){self.resources.retire(old)?;}self.cap_vertices=next;self.cap_signature=cap;}self.cap_key=Some(cap_key);
  for old in self.contour_batches.iter().filter(|batch|batch.owned).map(|batch|batch.vertices).chain([self.vertices.take(),self.indices.take()].into_iter().flatten()){self.resources.retire(old)?;}
  self.resources.collect(fence,completed_serial)?;self.contour_metrics.additional_position_bytes=batches.iter().filter(|b|b.owned).map(|b|self.resources.logical_bytes(b.vertices).unwrap()).sum();self.contour_metrics.batches=batches.len();self.contour_metrics.compact_preprocess_ms=preprocessing;self.contour_metrics.cold_build_ms=started.elapsed().as_secs_f64()*1000.;self.contour_batches=batches;
  self.vertices=vertices;self.indices=indices;self.count=count;self.revision=Some(revision);self.surface_color=Some(style.front_color);self.uploads+=1;Ok(())
 }
 fn upload_cap(&mut self,context:&VulkanContext,cap:Option<([[f32;3];4],[f32;4])>)->Result<Option<BufferId>,String>{
  if let Some((frame,color))=cap{let vertices=[0,1,2,0,2,3].map(|i|ingetrazo_render::buffer::Vertex::new(frame[i],[0.,0.,1.],color,[0.,0.]));Ok(Some(self.resources.upload(context,bytemuck::cast_slice(&vertices),vk::BufferUsageFlags::VERTEX_BUFFER,false)?))}else{Ok(None)}
 }
 pub fn record(&mut self,command:vk::CommandBuffer,serial:u64)->Result<(),String> {
  let Some(target)=&self.target else{return Ok(());};
  let mut clears=vec![vk::ClearValue{color:vk::ClearColorValue{float32:[0.65,0.67,0.66,1.0]}}];
  if self.samples!=vk::SampleCountFlags::TYPE_1{clears.push(vk::ClearValue::default());}
  clears.push(vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.0,stencil:0}});
  unsafe {
   self.device.cmd_begin_render_pass(command,&vk::RenderPassBeginInfo::default().render_pass(self.render_pass).framebuffer(target.framebuffer).render_area(vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}).clear_values(&clears),vk::SubpassContents::INLINE);
   self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.pipeline);
   self.device.cmd_set_viewport(command,0,&[vk::Viewport{x:0.0,y:0.0,width:target.extent.width as f32,height:target.extent.height as f32,min_depth:0.0,max_depth:1.0}]);self.device.cmd_set_scissor(command,0,&[vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}]);
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,bytemuck::cast_slice(&self.mvp));
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,64,bytemuck::cast_slice(&self.clip));
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&self.clip_enabled));
   if let (Some(vertices),Some(indices))=(self.vertices,self.indices){self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);self.device.cmd_draw_indexed(command,self.count,1,0,0,0);self.resources.mark_used(vertices,serial)?;self.resources.mark_used(indices,serial)?;}
   if self.clip_enabled!=0 {
    if let (Some(cap),Some(vertices),Some(indices))=(self.cap_vertices,self.vertices,self.indices) {
     self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.mask_pipeline);
     self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);
     self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);
     self.device.cmd_draw_indexed(command,self.count,1,0,0,0);
     self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.cap_pipeline);
     let disabled=0u32;self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,80,bytemuck::bytes_of(&disabled));
     self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(cap).unwrap()],&[0]);self.device.cmd_draw(command,6,1,0,0);self.resources.mark_used(cap,serial)?;
    }
   }
   if self.clip_enabled!=0 {
    let contours=self.contours.as_ref().unwrap();let mut push=[0u8;128];push[..64].copy_from_slice(bytemuck::cast_slice(&self.mvp));push[64..80].copy_from_slice(bytemuck::cast_slice(&self.clip));push[96..108].copy_from_slice(bytemuck::cast_slice(&self.eye));
    let factor=self.width_factor*self.display_scale/target.extent.height as f32;push[108..112].copy_from_slice(bytemuck::bytes_of(&factor));push[112..128].copy_from_slice(bytemuck::cast_slice(&self.edge_color));
    self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,contours.pipeline);self.device.cmd_push_constants(command,contours.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);
    for (batch,set) in self.contour_batches.iter().zip(&contours.sets){
     let mode=[if batch.compact{3u32}else{12u32},u32::from(batch.compact)];self.device.cmd_push_constants(command,contours.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,84,bytemuck::cast_slice(&mode));
     self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,contours.layout,0,&[*set],&[]);self.device.cmd_draw(command,6,batch.triangles,0,0);self.resources.mark_used(batch.vertices,serial)?;if batch.indices!=batch.vertices{self.resources.mark_used(batch.indices,serial)?;}
    }
   }
   self.device.cmd_end_render_pass(command);
  }
  Ok(())
 }
}
impl Drop for World {fn drop(&mut self){let _=unsafe {self.device.device_wait_idle()};drop(self.target.take());drop(self.contours.take());unsafe {self.device.destroy_pipeline(self.cap_pipeline,None);self.device.destroy_pipeline(self.mask_pipeline,None);self.device.destroy_pipeline(self.pipeline,None);self.device.destroy_pipeline_layout(self.layout,None);self.device.destroy_render_pass(self.render_pass,None);self.device.destroy_descriptor_pool(self.pool,None);self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}
#[derive(Clone,Copy)]enum WorldPass {Surface,Mask,Cap,Contour}
fn make_pipeline(device:&Device,render_pass:vk::RenderPass,layout:vk::PipelineLayout,pass:WorldPass,samples:vk::SampleCountFlags)->Result<vk::Pipeline,String> {
 let code=|bytes:&[u8]|ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string());
 let vertex_code=code(if matches!(pass,WorldPass::Contour){include_bytes!("shaders/contour.vert.spv")}else{include_bytes!("shaders/world.vert.spv")})?;
 let fragment_code=code(include_bytes!("shaders/world.frag.spv"))?;
 let vertex=unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&vertex_code),None)}.map_err(|e|e.to_string())?;
 let fragment=match unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&fragment_code),None)} {Ok(m)=>m,Err(e)=>{unsafe {device.destroy_shader_module(vertex,None)};return Err(e.to_string());}};
 let stages=[vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vertex).name(c"main"),vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main")];
 let binding=[vk::VertexInputBindingDescription{binding:0,stride:48,input_rate:vk::VertexInputRate::VERTEX}];let attrs=[vk::VertexInputAttributeDescription{location:0,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:0},vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:12},vk::VertexInputAttributeDescription{location:2,binding:0,format:vk::Format::R32G32B32A32_SFLOAT,offset:24},vk::VertexInputAttributeDescription{location:3,binding:0,format:vk::Format::R32G32_SFLOAT,offset:40}];
 let input=if matches!(pass,WorldPass::Contour){vk::PipelineVertexInputStateCreateInfo::default()}else{vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs)};let assembly=vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);let viewport=vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);let raster=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.0).front_face(vk::FrontFace::COUNTER_CLOCKWISE).cull_mode(if matches!(pass,WorldPass::Mask){vk::CullModeFlags::FRONT}else{vk::CullModeFlags::NONE}).depth_bias_enable(matches!(pass,WorldPass::Cap)).depth_bias_constant_factor(2.0).depth_bias_slope_factor(2.0);let multisample=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);let stencil=vk::StencilOpState::default().fail_op(vk::StencilOp::KEEP).depth_fail_op(vk::StencilOp::KEEP).pass_op(if matches!(pass,WorldPass::Mask){vk::StencilOp::REPLACE}else{vk::StencilOp::KEEP}).compare_op(if matches!(pass,WorldPass::Cap){vk::CompareOp::EQUAL}else{vk::CompareOp::ALWAYS}).compare_mask(0xff).write_mask(if matches!(pass,WorldPass::Mask){0xff}else{0}).reference(1);
 let depth=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(!matches!(pass,WorldPass::Mask)).depth_compare_op(vk::CompareOp::LESS_OR_EQUAL).stencil_test_enable(matches!(pass,WorldPass::Mask|WorldPass::Cap)).front(stencil).back(stencil);let colors=[vk::PipelineColorBlendAttachmentState::default().color_write_mask(if matches!(pass,WorldPass::Mask){vk::ColorComponentFlags::empty()}else{vk::ColorComponentFlags::RGBA})];let blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR]);
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
 let result=Image::new_with_view(context,budget.clone(),vk::Extent2D{width:2,height:2},vk::Format::R8G8B8A8_SRGB,vk::ImageUsageFlags::COLOR_ATTACHMENT,vk::ImageAspectFlags::COLOR,vk::SampleCountFlags::TYPE_1,|_,_|Err(vk::Result::ERROR_OUT_OF_HOST_MEMORY));
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
  for (id,points) in ids.into_iter().zip(positions){batches.push(ContourBatch{vertices:id,indices:id,triangles:(points.len()/3)as u32,compact:true,owned:true});}
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
