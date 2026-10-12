//! Source gradient on the existing World pass; push constants, no buffers/uploads.
use ash::{vk,Device};
use std::sync::Arc;
use super::{WorldPass,make_pipeline};
use crate::native_vulkan::{VulkanContext,lifetime::Lifetime};
pub(super) struct Owner {device:Device,lifetime:Arc<Lifetime>,layout:vk::PipelineLayout,fill:vk::Pipeline,line:vk::Pipeline,pub background:Option<ingetrazo_render::world_background::Background>}
impl Owner {
 pub fn new(context:&VulkanContext,pass:vk::RenderPass,layout:vk::PipelineLayout,samples:vk::SampleCountFlags)->Result<Self,String>{
  let mut owner=Self{device:context.device.clone(),lifetime:context.lifetime.clone(),layout,fill:vk::Pipeline::null(),line:vk::Pipeline::null(),background:None};
  owner.fill=make_pipeline(&owner.device,pass,layout,WorldPass::Sky,samples,false,context.source_depth_clip_enabled)?;
  owner.line=make_pipeline(&owner.device,pass,layout,WorldPass::SkyLine,samples,false,context.source_depth_clip_enabled)?;
  Ok(owner)
 }
 pub fn record(&self,command:vk::CommandBuffer){
  let Some(background)=&self.background else{return;};
  unsafe {
   self.device.cmd_push_constants(command,self.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,bytemuck::bytes_of(background));
   self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.fill);
   self.device.cmd_draw(command,12,1,0,0);
   if background.horizon>-1.&&background.horizon<1.{self.device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,self.line);self.device.cmd_draw(command,2,1,12,0);}
  }
 }
}
impl Drop for Owner {fn drop(&mut self){if self.lifetime.uncertain(){return;}unsafe{for p in [self.fill,self.line]{if p!=vk::Pipeline::null(){self.device.destroy_pipeline(p,None);}}}}}
