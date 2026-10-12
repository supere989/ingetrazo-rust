//! Immutable camera-facing buffers, encoded directly in the existing world pass.
//! Only billboard vertices change with the camera; ordinary meshes and images stay cached.
use crate::{Buffer, NativeMetal};
use crate::world_texture::{self, Asset};
use ingetrazo_core::{camera::OrbitCamera, scene::Scene};
use ingetrazo_render::{billboards::Packet, native_textures::Geometry};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
use std::sync::Arc;

const MAX_BYTES: usize = 256 * 1024 * 1024;
struct Pipeline {
    textured: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    colored: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    depth: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    disabled: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    samples: usize,
    stencil: bool,
}
struct BufferOwner { vertices: Buffer, asset: Option<Arc<Asset>>, color: Option<[f32; 3]> }
struct Draw { order: usize, buffer: usize, first: usize, count: usize }
pub(crate) struct Prepared {
 reflected:bool,
    owner:super::world::Owner,
    pipeline: Option<Arc<Pipeline>>,
    buffers: Vec<BufferOwner>,
    draws: Vec<Draw>,
    assets: Vec<(String, Arc<Asset>)>,
    pub uploads: u64,
    pub texture_uploads: u64,
    pub bytes: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key { pub revisions: super::world_scene::Revisions, pub pose: Option<u64>, pub camera: [u32;7], pub enabled: bool, pub samples: usize, pub stencil: bool }
impl Key {
 pub fn new(revisions:super::world_scene::Revisions,pose:Option<u64>,camera:&OrbitCamera,eye:glam::Vec3,enabled:bool,samples:usize,stencil:bool)->Result<Self,String>{
  if !eye.is_finite()||!camera.target.is_finite(){return Err("Metal billboard camera admission".into());}
  let target=camera.target.as_vec3();
  Ok(Self{revisions,pose,camera:[u32::from(camera.perspective),eye.x.to_bits(),eye.y.to_bits(),eye.z.to_bits(),target.x.to_bits(),target.y.to_bits(),target.z.to_bits()],enabled,samples,stencil})
 }
 pub fn same_topology(&self,other:&Self)->bool{self.revisions==other.revisions&&self.pose.is_some()&&self.pose==other.pose}
}
impl Prepared {
 pub fn validate_owner(&self,metal:&NativeMetal)->Result<(),String>{self.owner.validate(metal)}
 pub fn retained_bytes(&self)->usize{std::mem::size_of::<Self>()+self.buffers.capacity()*std::mem::size_of::<BufferOwner>()+self.draws.capacity()*std::mem::size_of::<Draw>()+self.assets.capacity()*std::mem::size_of::<(String,Arc<Asset>)>()+self.assets.iter().map(|a|a.0.capacity()).sum::<usize>()}
 pub fn prepare(metal:&NativeMetal,scene:&Scene,packet:&Packet,camera:&OrbitCamera,eye:glam::Vec3,key:Key,previous:Option<&Self>,reuse_images:bool,world_assets:&[Arc<Asset>],reflected:bool)->Result<Self,String>{
  if let Some(previous)=previous{previous.validate_owner(metal)?;if previous.reflected!=reflected{return Err("Billboard diagnostic orientation owner mismatch".into());}}
  let mut out=Self{reflected,owner:super::world::Owner::new(metal),pipeline:None,buffers:Vec::new(),draws:Vec::new(),assets:Vec::new(),uploads:0,texture_uploads:0,bytes:0};
  if !key.enabled||packet.runs.is_empty(){
   // Hiding a receiver retires its geometry, while its immutable image cache
   // remains available for Hide/Unhide and style restoration.
   if let Some(previous)=previous{out.assets.try_reserve_exact(previous.assets.len()).map_err(|_|"Metal hidden billboard cache admission")?;for(path,asset)in &previous.assets{let mut name=String::new();name.try_reserve_exact(path.len()).map_err(|_|"Metal hidden billboard path admission")?;name.push_str(path);out.assets.push((name,asset.clone()));}out.pipeline=previous.pipeline.clone();}
   return Ok(out);
  }
  let textures=packet.textures(camera,eye)?;
  out.buffers.try_reserve_exact(textures.len()).map_err(|_|"Metal billboard buffer admission")?;
  let count=textures.values().map(|b|b.source_texture_runs.len()).sum::<usize>();
  out.draws.try_reserve_exact(count).map_err(|_|"Metal billboard draw admission")?;
  out.assets.try_reserve_exact(textures.len()).map_err(|_|"Metal billboard image admission")?;
  for (path,buffer) in textures {
   let geometry=Geometry::prepare(&buffer)?;
   if geometry.vertices.is_empty(){continue;}
   out.bytes=out.bytes.checked_add(geometry.upload_bytes().len()).filter(|n|*n<=MAX_BYTES).ok_or("Metal billboard geometry exceeds256MiB")?;
   let asset=if buffer.source_billboard_color.is_some(){None}else{
    let old=previous.and_then(|p|p.assets.iter().find(|a|a.0==path).map(|a|a.1.clone()));
    let asset=if reuse_images&&old.is_some(){let old=old.unwrap();world_assets.iter().find(|a|a.id==old.id).cloned().unwrap_or(old)}else{
     let bytes=super::world_scene::sampled_asset(scene,&path)?;let id=world_texture::identity(&bytes)?;
     if let Some(asset)=world_assets.iter().find(|a|a.id==id){asset.clone()}else if let Some(old)=old.filter(|a|a.id==id){old}else if let Some((_,asset))=out.assets.iter().find(|a|a.1.id==id){asset.clone()}else{
      let mut published=Vec::new();published.try_reserve_exact(world_assets.len()+previous.map_or(0,|p|p.assets.len())).map_err(|_|"Metal billboard asset lookup admission")?;published.extend(world_assets.iter().cloned());if let Some(previous)=previous{published.try_reserve_exact(previous.assets.len()).map_err(|_|"Metal billboard asset lookup admission")?;published.extend(previous.assets.iter().map(|a|a.1.clone()));}
      if let Some(asset)=published.iter().find(|a|a.id==id){asset.clone()}else{
       let mut candidates=Vec::new();candidates.try_reserve_exact(out.assets.len()).map_err(|_|"Metal billboard asset lookup admission")?;candidates.extend(out.assets.iter().map(|a|a.1.clone()));
       out.texture_uploads+=1;world_texture::upload(metal,id,&bytes,&published,&candidates)?
      }
     }
    };out.assets.push((path,asset.clone()));Some(asset)
   };
   let mut vertices=metal.buffer(geometry.upload_bytes().len(),true)?;vertices.write(geometry.upload_bytes())?;
   let index=out.buffers.len();out.buffers.push(BufferOwner{vertices,asset,color:buffer.source_billboard_color});out.uploads+=1;
   for draw in geometry.draws {out.draws.push(Draw{order:draw.source.order,buffer:index,first:draw.first as usize,count:draw.count as usize});}
  }
  out.draws.sort_unstable_by_key(|d|d.order);
  if !out.draws.is_empty(){out.pipeline=Some(if let Some(p)=previous.and_then(|p|p.pipeline.as_ref()).filter(|p|p.samples==key.samples&&p.stencil==key.stencil){p.clone()}else{Arc::new(Pipeline::new(metal,key.samples,key.stencil,reflected)?)});}
  if out.retained_bytes()>8*1024*1024{return Err("Metal billboard metadata exceeds8MiB".into());}Ok(out)
 }
 pub(crate) fn assets(&self)->&[(String,Arc<Asset>)]{&self.assets}
 pub fn encode(&self,encoder:&ProtocolObject<dyn MTLRenderCommandEncoder>,camera:&[[f32;4];4],section:&super::world_section::Section,height:u32,depth_enabled:bool){
  let Some(pipeline)=&self.pipeline else{return;};
  let identity=glam::Mat4::IDENTITY.to_cols_array_2d();
  encoder.setFrontFacingWinding(if self.reflected{MTLWinding::Clockwise}else{MTLWinding::CounterClockwise});encoder.setCullMode(MTLCullMode::None);encoder.setDepthBias_slopeScale_clamp(0.,0.,0.);
  encoder.setDepthStencilState(Some(if depth_enabled{&pipeline.depth}else{&pipeline.disabled}));
  unsafe{encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(&identity).cast(),64,1);encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(camera).cast(),64,2);encoder.setVertexBytes_length_atIndex(std::ptr::NonNull::from(section).cast(),std::mem::size_of::<super::world_section::Section>(),3);}
  for draw in &self.draws {
   let buffer=&self.buffers[draw.buffer];
   unsafe{encoder.setVertexBuffer_offset_atIndex(Some(&buffer.vertices.raw),0,0);}
   if let Some(asset)=&buffer.asset {
    encoder.setRenderPipelineState(&pipeline.textured);
    // Original always forces hard cutout and samples the image in both shaded styles.
    #[repr(C)] struct Paint {opacity:f32,shade:f32,hard_cutout:u32,height:u32,mean:[f32;4]}
    let paint=Paint{opacity:1.,shade:1.,hard_cutout:1,height,mean:[0.;4]};
    unsafe{encoder.setFragmentTexture_atIndex(Some(&asset.image),0);encoder.setFragmentSamplerState_atIndex(Some(&asset.sampler),0);encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&paint).cast(),32,0);}
   }else{
    encoder.setRenderPipelineState(&pipeline.colored);let rgb=buffer.color.expect("Admitted billboard color");let color=[rgb[0],rgb[1],rgb[2],1.];
    unsafe{encoder.setFragmentBytes_length_atIndex(std::ptr::NonNull::from(&color).cast(),16,0);}
   }
   unsafe{encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(MTLPrimitiveType::Triangle,draw.first,draw.count,1);}
  }
 }
}
impl Pipeline {
 fn new(metal:&NativeMetal,samples:usize,stencil:bool,reflected:bool)->Result<Self,String>{
  let lib=metal.device.newLibraryWithSource_options_error(&NSString::from_str(&format!("#define SOURCE_REFLECT_Y {}\n{}",u8::from(reflected),include_str!("shaders/world.metal"))),None).map_err(|e|format!("Metal billboard shader:{e:?}"))?;
  let make=|fragment:&str|->Result<_,String>{
   let d=MTLRenderPipelineDescriptor::new();let vertex=lib.newFunctionWithName(&NSString::from_str("world_texture_vertex")).ok_or("Metal billboard vertex missing")?;let fragment=lib.newFunctionWithName(&NSString::from_str(fragment)).ok_or("Metal billboard fragment missing")?;
   d.setVertexFunction(Some(&vertex));d.setFragmentFunction(Some(&fragment));d.setRasterSampleCount(samples);
   let format=if stencil{MTLPixelFormat::Depth32Float_Stencil8}else{MTLPixelFormat::Depth32Float};d.setDepthAttachmentPixelFormat(format);if stencil{d.setStencilAttachmentPixelFormat(format);}
   let color=unsafe{d.colorAttachments().objectAtIndexedSubscript(0)};color.setPixelFormat(MTLPixelFormat::RGBA8Unorm);color.setBlendingEnabled(true);color.setSourceRGBBlendFactor(MTLBlendFactor::SourceAlpha);color.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);color.setRgbBlendOperation(MTLBlendOperation::Add);color.setSourceAlphaBlendFactor(MTLBlendFactor::Zero);color.setDestinationAlphaBlendFactor(MTLBlendFactor::One);color.setAlphaBlendOperation(MTLBlendOperation::Add);
   metal.device.newRenderPipelineStateWithDescriptor_error(&d).map_err(|e|format!("Metal billboard pipeline:{e:?}"))
  };
  let depth=MTLDepthStencilDescriptor::new();depth.setDepthCompareFunction(MTLCompareFunction::LessEqual);depth.setDepthWriteEnabled(true);
  let disabled=MTLDepthStencilDescriptor::new();disabled.setDepthCompareFunction(MTLCompareFunction::Always);disabled.setDepthWriteEnabled(false);
  Ok(Self{textured:make("world_texture_fragment")?,colored:make("world_billboard_color")?,depth:metal.device.newDepthStencilStateWithDescriptor(&depth).ok_or("Metal billboard depth state")?,disabled:metal.device.newDepthStencilStateWithDescriptor(&disabled).ok_or("Metal billboard disabled depth state")?,samples,stencil})
 }
}
