//! Direct cached position/UV submission. Replacement descriptors and images are
//! prepared together; old descriptors are destroyed before fence-retired images.
use super::{resources::{BufferId,Resources},texture_cache::{AssetId,PreparedTexture,TextureCache},VulkanContext};
use ash::{vk,Device};
use ingetrazo_render::{buffer::TextureBuffers,native_textures::Geometry};
use std::collections::{HashMap,HashSet};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum Phase{Opaque,BackOpaque,TransparentFront,TransparentBack,Mask,Depth}
#[derive(Clone,Debug)]struct Draw{buffer:BufferId,set:vk::DescriptorSet,asset:AssetId,first:u32,count:u32,opacity:f32,shade:f32,cutout:bool,back:bool,cull:bool,opaque:bool,path:String,order:usize}
struct Pool{device:Device,handle:vk::DescriptorPool}
impl Drop for Pool{fn drop(&mut self){unsafe{self.device.destroy_descriptor_pool(self.handle,None)}}}
pub struct Candidate{pool:Option<Pool>,buffers:Vec<BufferId>,draws:Vec<Draw>,images:Vec<PreparedTexture>,pub contours:Vec<(BufferId,u32,u32)>}
#[derive(Default,Clone,Copy,Debug)]pub struct Stats{pub builds:u64,pub uploads:u64,pub vertex_bytes:u64,pub draws:u64,pub cache_hits:u64,pub rollbacks:u64}
pub struct Textures{probe_pressure:bool,budget:std::sync::Arc<super::budget::VulkanBudget>,device:Device,layout:vk::PipelineLayout,descriptor_layout:vk::DescriptorSetLayout,pipelines:[vk::Pipeline;8],pool:Option<Pool>,buffers:Vec<BufferId>,draws:Vec<Draw>,cache:TextureCache,pub stats:Stats}
impl Textures{
 pub fn new(context:&VulkanContext,render_pass:vk::RenderPass,samples:vk::SampleCountFlags,budget:std::sync::Arc<super::budget::VulkanBudget>)->Result<Self,String>{
  let device=context.device.clone();let mut result=Self{probe_pressure:false,budget,device,layout:vk::PipelineLayout::null(),descriptor_layout:vk::DescriptorSetLayout::null(),pipelines:[vk::Pipeline::null();8],pool:None,buffers:vec![],draws:vec![],cache:TextureCache::default(),stats:Stats::default()};
  let bindings=[vk::DescriptorSetLayoutBinding::default().binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).descriptor_count(1).stage_flags(vk::ShaderStageFlags::FRAGMENT)];
  result.descriptor_layout=unsafe{result.device.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),None)}.map_err(|e|e.to_string())?;
  result.layout=unsafe{result.device.create_pipeline_layout(&vk::PipelineLayoutCreateInfo::default().set_layouts(&[result.descriptor_layout]).push_constant_ranges(&[vk::PushConstantRange::default().stage_flags(vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT).offset(0).size(128)]),None)}.map_err(|e|e.to_string())?;
  for index in 0..8{result.pipelines[index]=pipeline(&result.device,render_pass,result.layout,samples,index)?;}Ok(result)
 }
 pub fn prepare(&mut self,context:&VulkanContext,resources:&mut Resources,buffers:&TextureBuffers,assets:&HashMap<String,Vec<u8>>,fence:vk::Fence,completed:u64,storage_limit:u64)->Result<Candidate,String>{
  if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{return Err("Texture replacement requires completed draw fence".into());}
  let mut candidate=Candidate{pool:None,buffers:vec![],draws:vec![],images:vec![],contours:vec![]};
  let prepare=(||->Result<(),String>{
   if buffers.is_empty(){return Ok(());}
   let count=u32::try_from(buffers.len()).map_err(|_|"Too many texture descriptors")?;
   let pool=unsafe{self.device.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(count).pool_sizes(&[vk::DescriptorPoolSize{ty:vk::DescriptorType::COMBINED_IMAGE_SAMPLER,descriptor_count:count}]),None)}.map_err(|e|e.to_string())?;
   candidate.pool=Some(Pool{device:self.device.clone(),handle:pool});
   let mut paths=buffers.iter().collect::<Vec<_>>();paths.sort_by_key(|(_,b)|b.source_texture_runs.iter().map(|r|r.order).min().unwrap_or(usize::MAX));
   let mut images=HashMap::<AssetId,(vk::ImageView,vk::Sampler,bool)>::new();
   for(path,buffer)in paths{
    let geometry=Geometry::prepare(buffer)?;if geometry.vertices.is_empty(){continue;}
    let file;let encoded=if let Some(bytes)=assets.get(path){bytes.as_slice()}else{file=std::fs::read(path).map_err(|e|format!("Texture {path}: {e}"))?;file.as_slice()};
    let asset=self.cache.identify(encoded);let id=asset.id();
    let sampled=if let Some(sampled)=images.get(&id){*sampled}else{let _pressure=if self.probe_pressure{let mut ledger=self.budget.ledger.lock().unwrap();let previous=ledger.total_limit;ledger.total_limit=Some(ledger.total_committed());Some(PressureRestore{budget:self.budget.clone(),previous})}else{None};let image=self.cache.prepare(context,resources,&asset,256*1024*1024).map_err(|e|format!("Texture image admission: {e}"))?;let sampled=image.sampled();images.insert(id,sampled);candidate.images.push(image);sampled};
    let set=unsafe{self.device.allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&[self.descriptor_layout]))}.map_err(|e|e.to_string())?[0];
    let infos=[vk::DescriptorImageInfo::default().image_view(sampled.0).sampler(sampled.1).image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];unsafe{self.device.update_descriptor_sets(&[vk::WriteDescriptorSet::default().dst_set(set).dst_binding(0).descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER).image_info(&infos)],&[])};
    let vertex=resources.upload(context,geometry.upload_bytes(),vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::STORAGE_BUFFER,false)?;candidate.buffers.push(vertex);self.stats.uploads+=1;self.stats.vertex_bytes+=geometry.upload_bytes().len()as u64;
    if geometry.front_vertices>0 {
     if geometry.upload_bytes().len()as u64<=storage_limit{candidate.contours.push((vertex,geometry.front_vertices/3,5));}
     else {let triangles=storage_limit.min(8*1024*1024)/36;if triangles==0{return Err("Texture contour storage range cannot hold one triangle".into());}for chunk in geometry.vertices[..geometry.front_vertices as usize].chunks(triangles as usize*3){let positions=chunk.iter().map(|v|v.position).collect::<Vec<_>>();let id=resources.upload(context,bytemuck::cast_slice(&positions),vk::BufferUsageFlags::STORAGE_BUFFER,false)?;candidate.buffers.push(id);candidate.contours.push((id,(positions.len()/3)as u32,3));}}
    }
    for draw in geometry.draws{let source=draw.source;let opaque=source.opacity>=0.999;let opacity=if opaque{1.}else{format!("{:.3}",source.opacity).parse::<f32>().map_err(|e|e.to_string())?};candidate.draws.push(Draw{buffer:vertex,set,asset:id,first:draw.first,count:draw.count,opacity,shade:source.shade,cutout:sampled.2,back:source.back_side,cull:source.front_cull,opaque,path:path.clone(),order:source.order});}
   }
   // Source front opaque first occurrence, then sorted back/transparent keys.
   candidate.draws.sort_by(|a,b|{let phase=|d:&Draw|if d.opaque&&!d.back{0}else if d.opaque{1}else if !d.back{2}else{3};phase(a).cmp(&phase(b)).then_with(||if phase(a)==0{a.order.cmp(&b.order)}else{a.path.cmp(&b.path).then(a.shade.total_cmp(&b.shade)).then(a.opacity.total_cmp(&b.opacity)).then(a.cull.cmp(&b.cull))})});Ok(())
  })();
  if let Err(error)=prepare{self.abort(resources,candidate,fence,completed)?;return Err(error);}self.stats.builds+=1;Ok(candidate)
 }
 pub fn abort(&mut self,resources:&mut Resources,mut candidate:Candidate,fence:vk::Fence,completed:u64)->Result<(),String>{drop(candidate.pool.take());for buffer in candidate.buffers{resources.retire(buffer)?;}resources.collect(fence,completed)?;self.stats.rollbacks+=1;Ok(())}
 pub fn publish(&mut self,context:&VulkanContext,resources:&mut Resources,mut candidate:Candidate,fence:vk::Fence,completed:u64)->Result<(),String>{
  if !unsafe{context.device.get_fence_status(fence)}.map_err(|e|e.to_string())?{self.abort(resources,candidate,fence,completed)?;return Err("Texture publication requires completed draw fence".into());}
  for image in candidate.images.drain(..){self.cache.publish(image)?;}
  drop(self.pool.take());for buffer in self.buffers.drain(..){resources.retire(buffer)?;}resources.collect(fence,completed)?;
  self.pool=candidate.pool.take();self.buffers=candidate.buffers;self.draws=candidate.draws;
  let active=self.draws.iter().map(|d|d.asset).collect::<HashSet<_>>();self.cache.collect_unused(context,&active,fence,completed)?;Ok(())
 }
 /// No allocation, geometry traversal, hashing, decoding, or transfer on draw.
 pub fn record(&mut self,resources:&mut Resources,command:vk::CommandBuffer,serial:u64,phase:Phase,mvp:&[f32;16],plane:&[f32;4],clip:u32,height:u32,xray:bool)->Result<(),String>{
  for draw in self.draws.iter().filter(|d|match phase{Phase::Opaque=>d.opaque&&!d.back,Phase::BackOpaque=>d.opaque&&d.back,Phase::TransparentFront=>!d.opaque&&!d.back,Phase::TransparentBack=>!d.opaque&&d.back,Phase::Mask|Phase::Depth=>true}){
   let buffer=resources.buffer(draw.buffer).ok_or("Cached texture buffer retired before draw")?;resources.mark_used(draw.buffer,serial)?;self.cache.mark_used(draw.asset,serial)?;
   // viewport.py1652: XRay affects opaque fronts only; back and translucent
   // texture phases retain their paint opacity. _xray_face_depth uses all
   // texture positions without sampling alpha or writing colour.
   let index=if phase==Phase::Depth{7}else if phase==Phase::Mask{5}else if phase==Phase::Opaque&&xray{6}else if draw.opaque{if draw.back{1}else{0}}else if draw.back{4}else if draw.cull{3}else{2};
   let mut push=[0u8;96];push[..64].copy_from_slice(bytemuck::cast_slice(mvp));push[64..80].copy_from_slice(bytemuck::cast_slice(plane));push[80..84].copy_from_slice(bytemuck::bytes_of(&(clip|if matches!(phase,Phase::Mask|Phase::Depth){2}else{0})));push[84..88].copy_from_slice(bytemuck::bytes_of(&if phase==Phase::Opaque&&xray{0.6f32}else{draw.opacity}));push[88..92].copy_from_slice(bytemuck::bytes_of(&draw.shade));push[92..96].copy_from_slice(bytemuck::bytes_of(&(height<<1|u32::from(draw.cutout))));
   unsafe{self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.pipelines[index]);self.device.cmd_bind_descriptor_sets(command,vk::PipelineBindPoint::GRAPHICS,self.layout,0,&[draw.set],&[]);self.device.cmd_bind_vertex_buffers(command,0,&[buffer],&[0]);self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);self.device.cmd_draw(command,draw.count,1,draw.first,0);}self.stats.draws+=1;
  }Ok(())
 }
 pub fn set_pressure_for_probe(&mut self,enabled:bool){self.probe_pressure=enabled;}
 pub fn cache_stats(&self)->&super::texture_cache::TextureStats{&self.cache.stats}
 pub fn buffers(&self)->&[BufferId]{&self.buffers}
}
impl Drop for Textures{fn drop(&mut self){drop(self.pool.take());unsafe{for p in self.pipelines{self.device.destroy_pipeline(p,None);}self.device.destroy_pipeline_layout(self.layout,None);self.device.destroy_descriptor_set_layout(self.descriptor_layout,None);}}}
fn pipeline(device:&Device,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags,index:usize)->Result<vk::Pipeline,String>{
 let load=|bytes:&[u8]|ash::util::read_spv(&mut std::io::Cursor::new(bytes)).map_err(|e|e.to_string());let v=load(include_bytes!("shaders/texture.vert.spv"))?;let f=load(include_bytes!("shaders/texture.frag.spv"))?;
 let vertex=unsafe{device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&v),None)}.map_err(|e|e.to_string())?;
 let fragment=match unsafe{device.create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&f),None)}{Ok(f)=>f,Err(e)=>{unsafe{device.destroy_shader_module(vertex,None)};return Err(e.to_string());}};
 let stages=[vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::VERTEX).module(vertex).name(c"main"),vk::PipelineShaderStageCreateInfo::default().stage(vk::ShaderStageFlags::FRAGMENT).module(fragment).name(c"main")];
 let bindings=[vk::VertexInputBindingDescription{binding:0,stride:20,input_rate:vk::VertexInputRate::VERTEX}];let attrs=[vk::VertexInputAttributeDescription{location:0,binding:0,format:vk::Format::R32G32B32_SFLOAT,offset:0},vk::VertexInputAttributeDescription{location:1,binding:0,format:vk::Format::R32G32_SFLOAT,offset:12}];let input=vk::PipelineVertexInputStateCreateInfo::default().vertex_binding_descriptions(&bindings).vertex_attribute_descriptions(&attrs);let assembly=vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);let viewport=vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
 let raster=vk::PipelineRasterizationStateCreateInfo::default().polygon_mode(vk::PolygonMode::FILL).line_width(1.).front_face(vk::FrontFace::CLOCKWISE).cull_mode(if matches!(index,1|4|5){vk::CullModeFlags::FRONT}else if index==3{vk::CullModeFlags::BACK}else{vk::CullModeFlags::NONE}).depth_bias_enable(matches!(index,0|2|3|4|6|7)).depth_bias_constant_factor(1.).depth_bias_slope_factor(1.);
 let msaa=vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(samples);let stencil=vk::StencilOpState::default().compare_op(vk::CompareOp::ALWAYS).fail_op(vk::StencilOp::KEEP).depth_fail_op(vk::StencilOp::KEEP).pass_op(vk::StencilOp::REPLACE).compare_mask(255).write_mask(255).reference(1);let depth=vk::PipelineDepthStencilStateCreateInfo::default().depth_test_enable(true).depth_write_enable(index<2||index==7).depth_compare_op(vk::CompareOp::LESS_OR_EQUAL).stencil_test_enable(index==5).front(stencil).back(stencil);
 let colors=[vk::PipelineColorBlendAttachmentState::default().blend_enable(!matches!(index,5|7)).src_color_blend_factor(vk::BlendFactor::SRC_ALPHA).dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA).color_blend_op(vk::BlendOp::ADD).src_alpha_blend_factor(vk::BlendFactor::ZERO).dst_alpha_blend_factor(vk::BlendFactor::ONE).alpha_blend_op(vk::BlendOp::ADD).color_write_mask(if matches!(index,5|7){vk::ColorComponentFlags::empty()}else{vk::ColorComponentFlags::RGBA})];let blend=vk::PipelineColorBlendStateCreateInfo::default().attachments(&colors);let dynamic=vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&[vk::DynamicState::VIEWPORT,vk::DynamicState::SCISSOR]);
 let result=unsafe{device.create_graphics_pipelines(vk::PipelineCache::null(),&[vk::GraphicsPipelineCreateInfo::default().stages(&stages).vertex_input_state(&input).input_assembly_state(&assembly).viewport_state(&viewport).rasterization_state(&raster).multisample_state(&msaa).depth_stencil_state(&depth).color_blend_state(&blend).dynamic_state(&dynamic).layout(layout).render_pass(pass)],None)};unsafe{device.destroy_shader_module(vertex,None);device.destroy_shader_module(fragment,None);}
 match result{Ok(p)=>Ok(p[0]),Err((partial,e))=>{for p in partial{unsafe{device.destroy_pipeline(p,None)}}Err(e.to_string())}}
}

struct PressureRestore{budget:std::sync::Arc<super::budget::VulkanBudget>,previous:Option<u64>}
impl Drop for PressureRestore{fn drop(&mut self){self.budget.ledger.lock().unwrap().total_limit=self.previous;}}
