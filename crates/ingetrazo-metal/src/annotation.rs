//! Immutable depth queries and source draw metadata retained with a native callback.
use crate::{Buffer,NativeMetal};
use ingetrazo_render::annotation_query::Query;
use objc2::{rc::Retained,runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::sync::Arc;
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
pub struct Draw {pub gates:[u32;2],pub kind:u32,pub padding:u32,pub points:[f32;16]}
#[repr(C)]
#[derive(Clone,Copy,bytemuck::Pod,bytemuck::Zeroable)]
pub struct Parameters {pub inverse:[f32;16],pub eye:[f32;4],pub forward:[f32;4],pub extent_count_sample:[u32;4]}
pub struct Visibility {device:Retained<ProtocolObject<dyn MTLDevice>>,queue:Retained<ProtocolObject<dyn MTLCommandQueue>>,budget:Arc<std::sync::Mutex<ingetrazo_core::gpu_memory::MemoryBudget>>,poison:Arc<std::sync::atomic::AtomicBool>,pub(crate) queries:Buffer,pub(crate) hidden:Buffer,pub(crate) draws:Buffer,pub(crate) parameters:Parameters,pipeline:[Arc<Retained<ProtocolObject<dyn MTLComputePipelineState>>>;2],pub uploaded_bytes:u64,reads_depth:bool}
impl Visibility {
 pub fn prepare(metal:&NativeMetal,queries:&[Query],draws:&[Draw],parameters:Parameters,previous:Option<&Self>)->Result<Arc<Self>,String>{
  if previous.is_some_and(|old|!std::ptr::eq(&*old.device,&*metal.device)||!std::ptr::eq(&*old.queue,&*metal.queue)||!Arc::ptr_eq(&old.budget,&metal.budget)||!Arc::ptr_eq(&old.poison,&metal.poisoned)){return Err("Metal annotation foreign pipeline owner".into());}
  let n=queries.len();if n>200_000||draws.is_empty()||draws.len()>4096||parameters.extent_count_sample[2] as usize!=n||parameters.extent_count_sample[..2].contains(&0){return Err("Metal annotation query/count admission".into());}
  if !parameters.inverse.iter().chain(parameters.eye.iter()).chain(parameters.forward.iter()).all(|v|v.is_finite())||queries.iter().any(|q|q.active>1||!q.normalized.iter().all(|v|v.is_finite())||!q.threshold.is_finite()){return Err("Metal annotation finite admission".into());}
  if queries.iter().any(|q|q.active!=0&&(q.pixel[0]<0||q.pixel[1]<0||q.pixel[0]as u32>=parameters.extent_count_sample[0]||q.pixel[1]as u32>=parameters.extent_count_sample[1])){return Err("Metal annotation query pixel bounds".into());}
  for draw in draws {if draw.gates.iter().any(|&g|g!=u32::MAX&&g as usize>=n)||draw.kind>1||!draw.points.iter().all(|p|p.is_finite()&&p.abs()<=1_000_000.){return Err("Metal annotation draw admission".into());}}
  let pipeline=if let Some(old)=previous {old.pipeline.clone()}else{
   let options=MTLCompileOptions::new();options.setFastMathEnabled(false);
   let library=metal.device.newLibraryWithSource_options_error(&NSString::from_str(include_str!("shaders/annotation_visibility.metal")),Some(&options)).map_err(|e|format!("Metal annotation visibility shader:{e:?}"))?;
   let build=|name:&str|->Result<_,String>{let f=library.newFunctionWithName(&NSString::from_str(name)).ok_or("Metal annotation visibility function missing")?;Ok(Arc::new(metal.device.newComputePipelineStateWithFunction_error(&f).map_err(|e|format!("Metal annotation visibility pipeline:{e:?}"))?))};[build("visibility_single")?,build("visibility_ms")?]
  };
  let query_bytes=bytemuck::cast_slice(queries);let draw_bytes=bytemuck::cast_slice(draws);
  let mut q=metal.buffer(query_bytes.len().max(32),true)?;if !query_bytes.is_empty(){q.write(query_bytes)?;}
  let h=metal.buffer((n*4).max(4),false)?;let mut d=metal.buffer(draw_bytes.len(),true)?;d.write(draw_bytes)?;
  Ok(Arc::new(Self {device:metal.device.clone(),queue:metal.queue.clone(),budget:metal.budget.clone(),poison:metal.poisoned.clone(),queries:q,hidden:h,draws:d,parameters,pipeline,uploaded_bytes:(query_bytes.len()+draw_bytes.len())as u64,reads_depth:queries.iter().any(|q|q.active!=0)}))
 }
 pub(crate) fn reads_depth(&self)->bool{self.reads_depth}
 pub(crate) fn encode(&self,command:&ProtocolObject<dyn MTLCommandBuffer>,world:&crate::world::Prepared)->Result<(),String>{
  if !std::ptr::eq(&*command.commandQueue(),&*self.queue)||!std::ptr::eq(&*world.depth_texture().device(),&*self.device){return Err("Metal annotation command/depth owner mismatch".into());}
  let samples=world.sample_count();let [width,height,count,sample]=self.parameters.extent_count_sample;
  if world.extent()!=(width,height)||sample>=samples as u32{return Err("Metal annotation depth target/sample mismatch".into());}
  if count==0{return Ok(());}
  let encoder=command.computeCommandEncoder().ok_or("Metal annotation compute encoder unavailable")?;
  let pipeline=&self.pipeline[usize::from(samples>1)];encoder.setComputePipelineState(pipeline);
  unsafe {encoder.setBuffer_offset_atIndex(Some(&self.queries.raw),0,0);encoder.setBuffer_offset_atIndex(Some(&self.hidden.raw),0,1);encoder.setBytes_length_atIndex(std::ptr::NonNull::from(&self.parameters).cast(),std::mem::size_of::<Parameters>(),2);encoder.setTexture_atIndex(Some(world.depth_texture()),0);}
  let threads=pipeline.threadExecutionWidth().min(pipeline.maxTotalThreadsPerThreadgroup());encoder.dispatchThreads_threadsPerThreadgroup(MTLSize{width:count as usize,height:1,depth:1},MTLSize{width:threads,height:1,depth:1});encoder.endEncoding();Ok(())
 }
}

/// Source NVIDIA GL 4x positions reflected once from bottom-origin to Metal rows.
const SOURCE_SAMPLE_POSITIONS: [MTLSamplePosition; 4] = [
    MTLSamplePosition { x: 0.375, y: 0.875 },
    MTLSamplePosition { x: 0.875, y: 0.625 },
    MTLSamplePosition { x: 0.125, y: 0.375 },
    MTLSamplePosition { x: 0.625, y: 0.125 },
];

/// Raster coverage uses the same pattern whether annotations are present or absent.
/// Devices without programmable positions retain their defaults; this alone does
/// not admit the source annotation depth selector on those devices.
pub(crate) fn source_raster_sample_positions(
    metal: &NativeMetal,
    samples: usize,
) -> Option<[MTLSamplePosition; 4]> {
    (samples == 4 && metal.programmable_sample_positions)
        .then_some(SOURCE_SAMPLE_POSITIONS)
}

/// Source GL 4x resolve selects the physical sample at (.375,.125) in
/// bottom-origin coordinates. Metal rows are top-origin: reflect Y once.
/// Program the full measured pattern, or require a literal default match.
pub fn source_sample_policy(metal:&NativeMetal,samples:usize)->Result<(u32,Option<[MTLSamplePosition;4]>),String>{
 if samples==1{return Ok((0,None));}
 if samples!=4{return Err("Metal source annotation sample policy requires1/4 samples".into());}
 let defaults=metal.default_sample_positions_4.as_ref().ok_or("Metal device does not support source4x samples")?;
 if let Some(positions)=source_raster_sample_positions(metal,samples){return Ok((0,Some(positions)));}
 if SOURCE_SAMPLE_POSITIONS.iter().all(|p|defaults.iter().any(|d|d.x==p.x&&d.y==p.y)){let selected=defaults.iter().position(|p|p.x==SOURCE_SAMPLE_POSITIONS[0].x&&p.y==SOURCE_SAMPLE_POSITIONS[0].y).unwrap();return Ok((selected as u32,None));}
 Err("Metal default sample positions cannot admit measured source annotation resolve".into())
}

/// Observations only: these do not authorize a depth-resolve selector.
pub fn sample_diagnostics(metal: &NativeMetal) -> serde_json::Value {
    let policy = source_sample_policy(metal, 4);
    serde_json::json!({
        "device_name": metal.info.name,
        "registry_id": metal.device.registryID(),
        "samples": 4,
        "programmable": metal.programmable_sample_positions,
        "default_positions_top_origin": metal.default_sample_positions_4.map(|positions| positions.map(|p| [p.x,p.y])),
        "source_positions_top_origin": SOURCE_SAMPLE_POSITIONS.map(|p| [p.x,p.y]),
        "strict_nvidia_reference_policy": match policy {
            Ok((sample, programmed)) => serde_json::json!({"admitted":true,"selected_sample":sample,"programmed":programmed.is_some()}),
            Err(error) => serde_json::json!({"admitted":false,"error":error}),
        },
        "resolve_selector_measured_on_this_device": false
    })
}
