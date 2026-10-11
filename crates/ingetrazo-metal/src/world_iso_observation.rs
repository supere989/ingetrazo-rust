//! Three-point diagnostic only; no imageblock write or production resolve replacement.
use crate::{NativeMetal,Buffer,world::Owner};
use objc2::{rc::Retained,runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::sync::Arc;
pub(crate) struct Observation {
 pub(crate) owner:Arc<Owner>,pub(crate) colors:Buffer,pub(crate) depths:Buffer,
 pub(crate) stop:Option<u8>,pub(crate) stages:bool, points:[[u32;2];3], tile:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
 depth:Retained<ProtocolObject<dyn MTLComputePipelineState>>,
}
impl Observation {
 pub(crate) fn new(metal:&NativeMetal,points:[[u32;2];3],stages:bool,stop:Option<u8>)->Result<Self,String>{
  if !metal.device.supportsFamily(MTLGPUFamily::Apple4){return Err("Iso observation requires actual Apple4 tile path".into());}
  let options=MTLCompileOptions::new();options.setFastMathEnabled(false);
  let lib=metal.device.newLibraryWithSource_options_error(&NSString::from_str(include_str!("shaders/world_iso_observation.metal")),Some(&options)).map_err(|e|format!("Iso observation shader:{e:?}"))?;
  let tile_function=lib.newFunctionWithName(&NSString::from_str("observe_iso_color")).ok_or("Iso color observer absent")?;
  let d=MTLTileRenderPipelineDescriptor::new();unsafe{d.setTileFunction(&tile_function);d.setRasterSampleCount(4);}d.setThreadgroupSizeMatchesTileSize(true);
  unsafe{d.colorAttachments().objectAtIndexedSubscript(0)}.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
  let tile=metal.device.newRenderPipelineStateWithTileDescriptor_options_reflection_error(&d,MTLPipelineOption::empty(),None).map_err(|e|format!("Iso tile observer:{e:?}"))?;
  if tile.maxTotalThreadsPerThreadgroup()<256{return Err("Iso tile observer cannot admit16x16 threads".into());}
  let depth_function=lib.newFunctionWithName(&NSString::from_str("observe_iso_depth")).ok_or("Iso depth observer absent")?;
  let depth=metal.device.newComputePipelineStateWithFunction_error(&depth_function).map_err(|e|format!("Iso depth observer:{e:?}"))?;
  let colors=metal.buffer(60*16,true)?;let depths=metal.buffer(12*4,true)?;
  unsafe{std::ptr::write_bytes(colors.raw.contents().as_ptr().cast::<u8>(),0xFF,60*16);std::ptr::write_bytes(depths.raw.contents().as_ptr().cast::<u8>(),0xFF,12*4);}
  Ok(Self{owner:Arc::new(Owner::new(metal)),colors,depths,points,tile,depth,stages,stop})
 }
 pub(crate) fn sample_length(&self)->usize{self.tile.imageblockSampleLength()}
 pub(crate) fn tile(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>){
  self.tile_at(encoder,4);
 }
 pub(crate) fn checkpoint(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,stage:u8)->bool{
  if self.stages {self.tile_at(encoder,stage);}
  self.stop==Some(stage)
 }
 fn tile_at(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,stage:u8){
  encoder.setRenderPipelineState(&self.tile);
  unsafe{encoder.setTileBytes_length_atIndex(std::ptr::NonNull::from(&self.points).cast(),std::mem::size_of_val(&self.points),0);encoder.setTileBuffer_offset_atIndex(Some(&self.colors.raw),stage as usize*192,1);}
  encoder.dispatchThreadsPerTile(MTLSize{width:16,height:16,depth:1});
 }
 pub(crate) fn depth(&self,command:&ProtocolObject<dyn MTLCommandBuffer>,image:&ProtocolObject<dyn MTLTexture>)->Result<(),String>{
  let e=command.computeCommandEncoder().ok_or("Iso depth encoder unavailable")?;e.setComputePipelineState(&self.depth);
  unsafe{e.setTexture_atIndex(Some(image),0);e.setBytes_length_atIndex(std::ptr::NonNull::from(&self.points).cast(),std::mem::size_of_val(&self.points),0);e.setBuffer_offset_atIndex(Some(&self.depths.raw),0,1);}
  e.dispatchThreads_threadsPerThreadgroup(MTLSize{width:12,height:1,depth:1},MTLSize{width:12,height:1,depth:1});e.endEncoding();Ok(())
 }
}
