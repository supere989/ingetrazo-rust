//! Source-ordered tile resolve for programmable Apple4 four-sample World.
//! Compute resolve remains an explicit diagnostic-only path.
use crate::{NativeMetal,world::Owner};
use objc2::{rc::Retained,runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::sync::Arc;
pub(crate) enum Kind{Tile(Retained<ProtocolObject<dyn MTLRenderPipelineState>>),Compute(Retained<ProtocolObject<dyn MTLComputePipelineState>>)}
pub(crate) struct Pipeline{pub(crate) kind:Kind,pub(crate) owner:Arc<Owner>,pub(crate) reflected:bool}
impl Pipeline{
 pub(crate) fn new(metal:&NativeMetal)->Result<Arc<Self>,String>{
  Self::build(metal,false)
 }
 pub(crate) fn reflected(metal:&NativeMetal)->Result<Arc<Self>,String>{
  super::annotation::reflected_sample_diagnostic_policy(metal)?;Self::build(metal,true)
 }
 fn build(metal:&NativeMetal,reflected:bool)->Result<Arc<Self>,String>{
  let options=MTLCompileOptions::new();options.setFastMathEnabled(false);
  let tile=!reflected && metal.device.supportsFamily(MTLGPUFamily::Apple4);
  let source=format!("#define SOURCE_TILE {}\n#define SOURCE_REFLECT_Y {}\n{}",u8::from(tile),u8::from(reflected),include_str!("shaders/world_resolve.metal"));
  let library=metal.device.newLibraryWithSource_options_error(&NSString::from_str(&source),Some(&options)).map_err(|e|format!("Source resolve shader:{e:?}"))?;
  let function=library.newFunctionWithName(&NSString::from_str(if tile{"source_resolve_tile"}else{"source_resolve_compute"})).ok_or("Source resolve function absent")?;
  let kind=if tile{
   let descriptor=MTLTileRenderPipelineDescriptor::new();unsafe{descriptor.setTileFunction(&function);}descriptor.setThreadgroupSizeMatchesTileSize(true);unsafe{descriptor.setRasterSampleCount(4);}
   unsafe{descriptor.colorAttachments().objectAtIndexedSubscript(0)}.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
   {let p=metal.device.newRenderPipelineStateWithTileDescriptor_options_reflection_error(&descriptor,MTLPipelineOption::empty(),None).map_err(|e|format!("Source tile resolve pipeline:{e:?}"))?;if p.maxTotalThreadsPerThreadgroup()<256{return Err("Source tile resolve cannot admit16x16 tile".into());}Kind::Tile(p)}
  }else{{let p=metal.device.newComputePipelineStateWithFunction_error(&function).map_err(|e|format!("Source compute resolve pipeline:{e:?}"))?;if p.maxTotalThreadsPerThreadgroup()<64{return Err("Source resolve cannot admit8x8 group".into());}Kind::Compute(p)}};
  Ok(Arc::new(Self{kind,owner:Arc::new(Owner::new(metal)),reflected}))
 }
 pub(crate) fn is_compute(&self)->bool{matches!(self.kind,Kind::Compute(_))}
 pub(crate) fn configure(&self,pass:&MTLRenderPassDescriptor){if let Kind::Tile(p)=&self.kind{pass.setTileWidth(16);pass.setTileHeight(16);pass.setImageblockSampleLength(p.imageblockSampleLength());}}
 pub(crate) fn tile(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>){if let Kind::Tile(p)=&self.kind{encoder.setRenderPipelineState(p);encoder.dispatchThreadsPerTile(MTLSize{width:16,height:16,depth:1});}}
 pub(crate) fn compute(&self,command:&ProtocolObject<dyn MTLCommandBuffer>,input:&ProtocolObject<dyn MTLTexture>,output:&ProtocolObject<dyn MTLTexture>)->Result<(),String>{
  let Kind::Compute(p)=&self.kind else{return Ok(())};
  let encoder=command.computeCommandEncoder().ok_or("Source resolve encoder absent")?;encoder.setComputePipelineState(p);
  unsafe{encoder.setTexture_atIndex(Some(input),0);encoder.setTexture_atIndex(Some(output),1);}
  let group=MTLSize{width:8,height:8,depth:1};
  encoder.dispatchThreads_threadsPerThreadgroup(MTLSize{width:output.width(),height:output.height(),depth:1},group);encoder.endEncoding();Ok(())
 }
 pub(crate) fn diagnostic(&self,extent:(u32,u32))->serde_json::Value{
  let compute=self.is_compute();let pixels=extent.0 as u64*extent.1 as u64;
  serde_json::json!({"supports_family_apple4":!compute,"diagnostic_row_reflection":self.reflected,"path":if compute{"compute"}else{"same-pass-tile"},"source_decode_table_bytes":1024,"logical_sample_read_bytes":if compute{pixels*16}else{0},"logical_output_write_bytes":pixels*4,"extra_output_images":0,"extra_full_frame_passes":if compute{1}else{0},"uniform_samples_preserved":true,"fast_math":false,"srgb_linearization":false,"quantization":"ordered binary32, rint ties-even encoded RGBA8"})
 }
}
