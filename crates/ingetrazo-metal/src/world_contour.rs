//! Direct cached GPU section contours, independent of stencil fill.
use crate::NativeMetal;
use objc2::{rc::Retained,runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::*;
#[repr(C)] pub(crate) struct Parameters {pub eye_width:[f32;4],pub color:[f32;4],pub info:[u32;4]}
pub(crate) struct Pipeline {pub state:Retained<ProtocolObject<dyn MTLRenderPipelineState>>,pub depth:Retained<ProtocolObject<dyn MTLDepthStencilState>>,pub samples:usize,pub fill:bool}
impl Pipeline {
 pub fn new(metal:&NativeMetal,samples:usize,fill:bool,reflected:bool)->Result<Self,String>{
  let lib=metal.device.newLibraryWithSource_options_error(&NSString::from_str(&format!("#define SOURCE_REFLECT_Y {}\n{}",u8::from(reflected),include_str!("shaders/contour.metal"))),None).map_err(|e|format!("Metal contour shader:{e:?}"))?;
  let vertex=lib.newFunctionWithName(&NSString::from_str("section_contour")).ok_or("Metal contour vertex missing")?;
  let fragment=lib.newFunctionWithName(&NSString::from_str("section_contour_color")).ok_or("Metal contour fragment missing")?;
  let descriptor=MTLRenderPipelineDescriptor::new();descriptor.setVertexFunction(Some(&vertex));descriptor.setFragmentFunction(Some(&fragment));descriptor.setRasterSampleCount(samples);
  let format=if fill{MTLPixelFormat::Depth32Float_Stencil8}else{MTLPixelFormat::Depth32Float};descriptor.setDepthAttachmentPixelFormat(format);if fill{descriptor.setStencilAttachmentPixelFormat(format);}
  unsafe{descriptor.colorAttachments().objectAtIndexedSubscript(0)}.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
  let state=metal.device.newRenderPipelineStateWithDescriptor_error(&descriptor).map_err(|e|format!("Metal contour pipeline:{e:?}"))?;
  let depth=MTLDepthStencilDescriptor::new();depth.setDepthCompareFunction(MTLCompareFunction::LessEqual);depth.setDepthWriteEnabled(true);
  Ok(Self{state,depth:metal.device.newDepthStencilStateWithDescriptor(&depth).ok_or("Metal contour depth missing")?,samples,fill})
 }
}
