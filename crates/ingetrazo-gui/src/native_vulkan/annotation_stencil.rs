//! Optional same-pass source-operation union attachment. One graphics command
//! buffer/fence serializes use; presentation never reads this attachment.
use std::sync::Arc;
use ash::{vk,Device};
use egui_ash_renderer::MemoryObserver;
use super::{VulkanContext,budget::VulkanBudget,lifetime::Lifetime};
pub(super) struct Target {device:Device,lifetime:Arc<Lifetime>,budget:Arc<VulkanBudget>,pub format:vk::Format,pub view:vk::ImageView,image:vk::Image,memory:vk::DeviceMemory}
/// Cold source raster selection. The legacy path is retained only for bounded
/// diagnostics; unknown values refuse rather than silently changing operators.
pub(super) fn selected()->Result<bool,String>{let value=std::env::var("INGETRAZO_ANNOTATION_STENCIL").ok();parse_selection(value.as_deref())}
fn parse_selection(value:Option<&str>)->Result<bool,String>{match value{None|Some("1")=>Ok(true),Some("0")=>Ok(false),Some(_)=>Err("Unknown native annotation stencil mode".into())}}
impl Target{
 /// Diagnostic allocation refusal; the submitted target is never replaced.
 pub fn verify_candidate_refusal(&self,context:&VulkanContext,extent:vk::Extent2D)->Result<(),String>{
  if !Arc::ptr_eq(&self.lifetime,&context.lifetime){return Err("Stencil refusal context owner mismatch".into());}
  context.lifetime.check()?;
  let (before,previous)={let mut ledger=self.budget.ledger.lock().map_err(|_|"Stencil ledger poisoned")?;let before=ledger.total_committed();let previous=ledger.total_limit;ledger.total_limit=Some(before);(before,previous)};
  struct Restore<'a>{budget:&'a VulkanBudget,previous:Option<u64>}
  impl Drop for Restore<'_>{fn drop(&mut self){self.budget.ledger.lock().unwrap_or_else(|e|e.into_inner()).total_limit=self.previous;}}
  let restore=Restore{budget:&self.budget,previous};
  let candidate=Self::new(context,self.budget.clone(),extent);
  let denied=candidate.is_err();drop(candidate);drop(restore);
  let ledger=self.budget.ledger.lock().map_err(|_|"Stencil ledger poisoned")?;
  if !denied||ledger.total_committed()!=before||ledger.total_limit!=previous{return Err("Stencil candidate refusal changed retained allocation ledger".into());}
  drop(ledger);
  let retry=Self::new(context,self.budget.clone(),extent)?;drop(retry);
  if self.budget.ledger.lock().map_err(|_|"Stencil ledger poisoned")?.total_committed()!=before{return Err("Stencil candidate retry leaked allocation ledger".into());}
  eprintln!("Native annotation stencil candidate allocation refused; previous target and ledger retained; restored-budget retry retired");Ok(())
 }
 pub fn new(context:&VulkanContext,budget:Arc<VulkanBudget>,extent:vk::Extent2D)->Result<Self,String>{
  context.lifetime.check()?;
  if extent.width==0||extent.height==0{return Err("Native annotation stencil extent is empty".into());}
  let usage=vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
  let (format,format_limit)=[vk::Format::S8_UINT,vk::Format::D24_UNORM_S8_UINT,vk::Format::D32_SFLOAT_S8_UINT].into_iter().find_map(|format|{
   let features=unsafe{context.instance.get_physical_device_format_properties(context.physical_device,format)}.optimal_tiling_features;
   if !features.contains(vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT){return None;}
   unsafe{context.instance.get_physical_device_image_format_properties(context.physical_device,format,vk::ImageType::TYPE_2D,vk::ImageTiling::OPTIMAL,usage,vk::ImageCreateFlags::empty())}.ok().filter(|p|p.sample_counts.contains(vk::SampleCountFlags::TYPE_1)&&extent.width<=p.max_extent.width&&extent.height<=p.max_extent.height).map(|p|(format,p.max_resource_size))
  }).ok_or("No admitted native annotation stencil attachment format")?;
  let device=context.device.clone();let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).format(format).extent(vk::Extent3D{width:extent.width,height:extent.height,depth:1}).mip_levels(1).array_layers(1).samples(vk::SampleCountFlags::TYPE_1).tiling(vk::ImageTiling::OPTIMAL).usage(usage);
  let image=unsafe{device.create_image(&info,None)}.map_err(|e|e.to_string())?;
  let mut memory=None;let result=(||{
   let req=unsafe{device.get_image_memory_requirements(image)};if req.size>format_limit{return Err("Native annotation stencil format resource size exceeded".into());}let props=unsafe{context.instance.get_physical_device_memory_properties(context.physical_device)};
   let ty=(0..props.memory_type_count).find(|&i|req.memory_type_bits&(1<<i)!=0&&props.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)).ok_or("No device-local stencil memory type")?;
   let ticket=budget.reserve(props.memory_types[ty as usize].heap_index,req.size).map_err(|e|e.to_string())?;
   let allocation=match unsafe{device.allocate_memory(&vk::MemoryAllocateInfo::default().allocation_size(req.size).memory_type_index(ty),None)}{Ok(m)=>m,Err(e)=>{budget.cancel(ticket);return Err(e.to_string());}};
   if let Err(e)=unsafe{device.bind_image_memory(image,allocation,0)}{unsafe{device.free_memory(allocation,None)};budget.cancel(ticket);return Err(e.to_string());}
   budget.commit(ticket,allocation);memory=Some(allocation);
   let range=vk::ImageSubresourceRange::default().aspect_mask(vk::ImageAspectFlags::STENCIL).level_count(1).layer_count(1);
   let view=unsafe{device.create_image_view(&vk::ImageViewCreateInfo::default().image(image).view_type(vk::ImageViewType::TYPE_2D).format(format).subresource_range(range),None)}.map_err(|e|e.to_string())?;
   eprintln!("Native annotation stencil: format={format:?} extent={}x{} allocation_bytes={}",extent.width,extent.height,req.size);Ok(view)
  })();
  match result{Ok(view)=>Ok(Self{device,lifetime:context.lifetime.clone(),budget,format,view,image,memory:memory.unwrap()}),Err(e)=>{unsafe{device.destroy_image(image,None);if let Some(m)=memory{device.free_memory(m,None);budget.freed(m);}}Err(e)}}
 }
}
impl Drop for Target{fn drop(&mut self){
 if self.lifetime.uncertain(){self.lifetime.retain((self.device.clone(),self.budget.clone(),self.image,self.memory,self.view));return;}
 unsafe{self.device.destroy_image_view(self.view,None);self.device.destroy_image(self.image,None);self.device.free_memory(self.memory,None);}self.budget.freed(self.memory);
}}

#[cfg(test)]mod selection_tests{
 #[test]fn source_mode_defaults_on_and_refuses_unknown_selection(){assert!(super::parse_selection(None).unwrap());assert!(super::parse_selection(Some("1")).unwrap());assert!(!super::parse_selection(Some("0")).unwrap());assert!(super::parse_selection(Some("approximate")).is_err());}
}
