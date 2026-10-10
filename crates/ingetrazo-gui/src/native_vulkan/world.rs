//! Bounded opaque world-space Vulkan pass. Full style/texture/section parity remains open.
use ash::{vk,Device};
use egui_ash_renderer::MemoryObserver;
use std::sync::Arc;
use super::{budget::VulkanBudget,resources::{Resources,BufferId},VulkanContext};
struct Image {device:Device,budget:Arc<VulkanBudget>,handle:vk::Image,memory:vk::DeviceMemory,view:vk::ImageView}
impl Image {
 fn new(context:&VulkanContext,budget:Arc<VulkanBudget>,extent:vk::Extent2D,format:vk::Format,usage:vk::ImageUsageFlags,aspect:vk::ImageAspectFlags)->Result<Self,String> {
  let device=context.device.clone();let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).extent(vk::Extent3D{width:extent.width,height:extent.height,depth:1}).mip_levels(1).array_layers(1).format(format).tiling(vk::ImageTiling::OPTIMAL).usage(usage).samples(vk::SampleCountFlags::TYPE_1);
  let handle=unsafe {device.create_image(&info,None)}.map_err(|e|e.to_string())?;let requirements=unsafe {device.get_image_memory_requirements(handle)};
  let result=(|| {
   let properties=unsafe {context.instance.get_physical_device_memory_properties(context.physical_device)};
   let ty=(0..properties.memory_type_count).find(|&i|requirements.memory_type_bits&(1<<i)!=0&&properties.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)).ok_or("No device-local image memory")?;
   let ticket=budget.reserve(properties.memory_types[ty as usize].heap_index,requirements.size).map_err(|e|e.to_string())?;
   let info=vk::MemoryAllocateInfo::default().allocation_size(requirements.size).memory_type_index(ty);
   let memory=match unsafe {device.allocate_memory(&info,None)} {Ok(m)=>m,Err(e)=>{budget.cancel(ticket);return Err(e.to_string());}};
   if let Err(e)=unsafe {device.bind_image_memory(handle,memory,0)} {unsafe {device.free_memory(memory,None)};budget.cancel(ticket);return Err(e.to_string());}
   budget.commit(ticket,memory);
   let range=vk::ImageSubresourceRange::default().aspect_mask(aspect).level_count(1).layer_count(1);
   match unsafe {device.create_image_view(&vk::ImageViewCreateInfo::default().image(handle).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(range),None)} {
    Ok(view)=>Ok((memory,view)),Err(e)=>{unsafe {device.free_memory(memory,None)};budget.freed(memory);Err(e.to_string())}
   }
  })();
  match result {Ok((memory,view))=>Ok(Self{device,budget,handle,memory,view}),Err(e)=>{unsafe {device.destroy_image(handle,None)};Err(e)}}
 }
}
impl Drop for Image {fn drop(&mut self){unsafe {self.device.destroy_image_view(self.view,None);self.device.destroy_image(self.handle,None);self.device.free_memory(self.memory,None)};self.budget.freed(self.memory);}}
struct Target {device:Device,pool:vk::DescriptorPool,_color:Image,_depth:Image,framebuffer:vk::Framebuffer,sampler:vk::Sampler,set:vk::DescriptorSet,extent:vk::Extent2D,texture:egui::TextureId}
impl Drop for Target {fn drop(&mut self){unsafe {self.device.destroy_framebuffer(self.framebuffer,None);self.device.destroy_sampler(self.sampler,None);let _=self.device.free_descriptor_sets(self.pool,&[self.set]);}}}
pub struct World {device:Device,budget:Arc<VulkanBudget>,resources:Resources,render_pass:vk::RenderPass,pipeline:vk::Pipeline,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pool:vk::DescriptorPool,depth_format:vk::Format,target:Option<Target>,vertices:Option<BufferId>,indices:Option<BufferId>,count:u32,revision:Option<(u64,u64,u64)>,style:Option<ingetrazo_render::style::StyleConfig>,mvp:[f32;16],pub uploads:u64}
impl World {
 pub fn new(context:&VulkanContext,budget:Arc<VulkanBudget>)->Result<Self,String> {
  let device=context.device.clone();
  let depth_format=[vk::Format::D24_UNORM_S8_UINT,vk::Format::D32_SFLOAT_S8_UINT].into_iter().find(|f|unsafe {context.instance.get_physical_device_format_properties(context.physical_device,*f)}.optimal_tiling_features.contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT)).ok_or("No supported depth/stencil format")?;
  let attachments=[vk::AttachmentDescription::default().format(vk::Format::R8G8B8A8_SRGB).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::STORE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL),vk::AttachmentDescription::default().format(depth_format).samples(vk::SampleCountFlags::TYPE_1).load_op(vk::AttachmentLoadOp::CLEAR).store_op(vk::AttachmentStoreOp::DONT_CARE).stencil_load_op(vk::AttachmentLoadOp::CLEAR).stencil_store_op(vk::AttachmentStoreOp::DONT_CARE).initial_layout(vk::ImageLayout::UNDEFINED).final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)];
  let colors=[vk::AttachmentReference{attachment:0,layout:vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL}];let depth=vk::AttachmentReference{attachment:1,layout:vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL};
  let subpasses=[vk::SubpassDescription::default().pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS).color_attachments(&colors).depth_stencil_attachment(&depth)];
  let dependencies=[vk::SubpassDependency::default().src_subpass(vk::SUBPASS_EXTERNAL).dst_subpass(0).src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT|vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS).src_access_mask(vk::AccessFlags::SHADER_READ).dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE|vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE),vk::SubpassDependency::default().src_subpass(0).dst_subpass(vk::SUBPASS_EXTERNAL).src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT).dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER).src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE).dst_access_mask(vk::AccessFlags::SHADER_READ)];
  let render_pass=unsafe {device.create_render_pass(&vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses).dependencies(&dependencies),None)}.map_err(|e|e.to_string())?;
  let push=[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX).size(64)];let layout=match unsafe {device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push),None)} {Ok(layout)=>layout,Err(e)=>{unsafe {device.destroy_render_pass(render_pass,None)};return Err(e.to_string());}};
  let pipeline=match make_pipeline(&device,render_pass,layout) {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e);}};
  let descriptor_layout=match egui_ash_renderer::vulkan::create_vulkan_descriptor_set_layout(&device) {Ok(l)=>l,Err(e)=>{unsafe {device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  let sizes=[vk::DescriptorPoolSize{ty:vk::DescriptorType::COMBINED_IMAGE_SAMPLER,descriptor_count:4}];let pool=match unsafe {device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET).max_sets(4).pool_sizes(&sizes),None)} {Ok(p)=>p,Err(e)=>{unsafe {device.destroy_descriptor_set_layout(descriptor_layout,None);device.destroy_pipeline(pipeline,None);device.destroy_pipeline_layout(layout,None);device.destroy_render_pass(render_pass,None);}return Err(e.to_string());}};
  Ok(Self{device,budget:budget.clone(),resources:Resources::new(context,budget),render_pass,pipeline,layout,descriptor_layout,pool,depth_format,target:None,vertices:None,indices:None,count:0,revision:None,style:None,mvp:[0.0;16],uploads:0})
 }
 /// Must be called after the previous UI/draw fence completes.
 pub fn resize(&mut self,context:&VulkanContext,renderer:&mut egui_ash_renderer::Renderer,extent:vk::Extent2D)->Result<egui::TextureId,String> {
  if let Some(target)=&self.target {if target.extent==extent{return Ok(target.texture);}}
  if extent.width==0||extent.height==0{return Err("Zero viewport extent".into());}
  let color=Image::new(context,self.budget.clone(),extent,vk::Format::R8G8B8A8_SRGB,vk::ImageUsageFlags::COLOR_ATTACHMENT|vk::ImageUsageFlags::SAMPLED,vk::ImageAspectFlags::COLOR)?;
  let depth=Image::new(context,self.budget.clone(),extent,self.depth_format,vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,vk::ImageAspectFlags::DEPTH|vk::ImageAspectFlags::STENCIL)?;
  let sampler=unsafe {self.device.create_sampler(&vk::SamplerCreateInfo::default().mag_filter(vk::Filter::LINEAR).min_filter(vk::Filter::LINEAR).address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE).address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE),None)}.map_err(|e|e.to_string())?;
  let attachments=[color.view,depth.view];let framebuffer=match unsafe {self.device.create_framebuffer(&vk::FramebufferCreateInfo::default().render_pass(self.render_pass).attachments(&attachments).width(extent.width).height(extent.height).layers(1),None)} {Ok(f)=>f,Err(e)=>{unsafe {self.device.destroy_sampler(sampler,None)};return Err(e.to_string());}};
  let set=match egui_ash_renderer::vulkan::create_vulkan_descriptor_set(&self.device,self.descriptor_layout,self.pool,color.view,sampler) {Ok(set)=>set,Err(e)=>{unsafe {self.device.destroy_framebuffer(framebuffer,None);self.device.destroy_sampler(sampler,None);}return Err(e.to_string());}};
  let texture=renderer.add_user_texture(set);
  if let Some(old)=self.target.take(){renderer.remove_user_texture(old.texture);drop(old);}
  self.target=Some(Target{device:self.device.clone(),pool:self.pool,_color:color,_depth:depth,framebuffer,sampler,set,extent,texture});Ok(texture)
 }
 pub fn set_scene(&mut self,context:&VulkanContext,scene:&ingetrazo_core::scene::Scene,camera:&ingetrazo_core::camera::OrbitCamera,style:&ingetrazo_render::style::StyleConfig,fence:vk::Fence,completed_serial:u64,epoch:u64)->Result<(),String> {
  self.mvp=(camera.projection_matrix()*camera.view_matrix()).as_mat4().to_cols_array();
  let revision=(scene.version,scene.view_version,epoch);
  if self.revision==Some(revision)&&self.style.as_ref()==Some(style){return Ok(());}
  let mut plain=style.clone();plain.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;plain.show_textures=false;
  let data=ingetrazo_render::pipeline::build_world_surfaces(scene,camera,&plain);
  let mesh=data.opaque_mesh;
  let count=mesh.indices.len().try_into().map_err(|_|"Too many GPU indices")?;
  let (vertices,indices)=if mesh.indices.is_empty(){(None,None)}else {
   let vertices=self.resources.upload(context,bytemuck::cast_slice(&mesh.vertices),vk::BufferUsageFlags::VERTEX_BUFFER,false)?;
   let indices=match self.resources.upload(context,bytemuck::cast_slice(&mesh.indices),vk::BufferUsageFlags::INDEX_BUFFER,false){Ok(i)=>i,Err(e)=>{self.resources.retire(vertices)?;self.resources.collect(fence,completed_serial)?;return Err(e);}};
   (Some(vertices),Some(indices))
  };
  for old in [self.vertices.take(),self.indices.take()].into_iter().flatten(){self.resources.retire(old)?;}
  self.resources.collect(fence,completed_serial)?;
  self.vertices=vertices;self.indices=indices;self.count=count;self.revision=Some(revision);self.style=Some(style.clone());self.uploads+=1;Ok(())
 }
 pub fn record(&mut self,command:vk::CommandBuffer,serial:u64)->Result<(),String> {
  let Some(target)=&self.target else{return Ok(());};
  let clears=[vk::ClearValue{color:vk::ClearColorValue{float32:[0.65,0.67,0.66,1.0]}},vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.0,stencil:0}}];
  unsafe {
   self.device.cmd_begin_render_pass(command,&vk::RenderPassBeginInfo::default().render_pass(self.render_pass).framebuffer(target.framebuffer).render_area(vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}).clear_values(&clears),vk::SubpassContents::INLINE);
   self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.pipeline);
   self.device.cmd_set_viewport(command,0,&[vk::Viewport{x:0.0,y:0.0,width:target.extent.width as f32,height:target.extent.height as f32,min_depth:0.0,max_depth:1.0}]);self.device.cmd_set_scissor(command,0,&[vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent}]);
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX,0,bytemuck::cast_slice(&self.mvp));
   if let (Some(vertices),Some(indices))=(self.vertices,self.indices){self.device.cmd_bind_vertex_buffers(command,0,&[self.resources.buffer(vertices).unwrap()],&[0]);self.device.cmd_bind_index_buffer(command,self.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);self.device.cmd_draw_indexed(command,self.count,1,0,0,0);self.resources.mark_used(vertices,serial)?;self.resources.mark_used(indices,serial)?;}
   self.device.cmd_end_render_pass(command);
  }
  Ok(())
 }
}
impl Drop for World {fn drop(&mut self){let _=unsafe {self.device.device_wait_idle()};drop(self.target.take());unsafe {self.device.destroy_pipeline(self.pipeline,None);self.device.destroy_pipeline_layout(self.layout,None);self.device.destroy_render_pass(self.render_pass,None);self.device.destroy_descriptor_pool(self.pool,None);self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}
fn make_pipeline(device:&Device,render_pass:vk::RenderPass,layout:vk::PipelineLayout)->Result<vk::Pipeline,String> {
 let code=|bytes:&[u8]|ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string());
 let vertex=unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code(include_bytes!("shaders/world.vert.spv"))?),None)}.map_err(|e|e.to_string())?;
 let fragment=match unsafe {device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&code(include_bytes!("shaders/world.frag.spv"))?),None)} {Ok(m)=>m,Err(e)=>{unsafe {device.destroy_shader_module(vertex,None)};return Err(e.to_string());}};
 let stages=[vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vertex).name(c"main"),vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main")];
 let binding=[vk::VertexInputBindingDescription{binding:0,stride:48,input_rate:vk::VertexInputRate::VERTEX}];let attrs=[vk::VertexInputAttributeDescription{location:0,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:0},vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:12},vk::VertexInputAttributeDescription{location:2,binding:0,format:vk::Format::R32G32B32A32_SFLOAT,offset:24},vk::VertexInputAttributeDescription{location:3,binding:0,format:vk::Format::R32G32_SFLOAT,offset:40}];
 let input=vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&binding).vertex_attribute_descriptions(&attrs);let assembly=vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);let viewport=vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);let raster=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.0).cull_mode(vk::CullModeFlags::NONE);let multisample=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);let depth=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(true).depth_compare_op(vk::CompareOp::LESS_OR_EQUAL);let colors=[vk::PipelineColorBlendAttachmentState::default().color_write_mask(vk::ColorComponentFlags::RGBA)];let blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR]);
 let result=unsafe {device.create_graphics_pipelines(vk::PipelineCache::null(),&[vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&input).input_assembly_state(&assembly).viewport_state(&viewport).rasterization_state(&raster).multisample_state(&multisample).depth_stencil_state(&depth).color_blend_state(&blend).dynamic_state(&dynamic).layout(layout).render_pass(render_pass)],None)};
 unsafe {device.destroy_shader_module(vertex,None);device.destroy_shader_module(fragment,None);}
 result.map(|p|p[0]).map_err(|(_,e)|e.to_string())
}
