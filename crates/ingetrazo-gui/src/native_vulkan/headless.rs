//! Surface-independent production Vulkan context for bounded device/resource evidence.
use super::{VulkanContext,queues};
use ash::{vk,Entry,ext::debug_utils,khr::surface};
use std::{ffi::{CStr,CString},error::Error};
#[derive(Clone,Debug)]
pub struct DeviceRequest {pub vendor_id:Option<u32>,pub require_discrete:bool,pub name:Option<String>}
impl DeviceRequest {
 pub fn nvidia_hardware()->Self {Self{vendor_id:Some(0x10de),require_discrete:true,name:None}}
 pub fn software_fixture()->Self {Self{vendor_id:None,require_discrete:false,name:Some("llvmpipe".into())}}
 fn accepts(&self,vendor:u32,kind:vk::PhysicalDeviceType,name:&str)->bool {
  self.vendor_id.is_none_or(|wanted|wanted==vendor)&&(!self.require_discrete||kind==vk::PhysicalDeviceType::DISCRETE_GPU)&&self.name.as_ref().is_none_or(|wanted|name.to_lowercase().contains(&wanted.to_lowercase()))
 }
}
impl VulkanContext {
 /// No WSI extensions, surface, present queue, swapchain, window or display connection.
 /// Diagnostic contexts have a five-second fence bound and fail fast rather than
 /// destroying resources that may still be in flight after timeout.
 pub fn new_headless(request:&DeviceRequest)->Result<Self,Box<dyn Error>> {
  Self::new_headless_features(request,true)
 }
 #[doc(hidden)]
 pub fn new_headless_rgba_fixture(request:&DeviceRequest)->Result<Self,Box<dyn Error>> { Self::new_headless_features(request,false) }
 fn new_headless_features(request:&DeviceRequest,enable_bc:bool)->Result<Self,Box<dyn Error>> {
  let entry=unsafe {Entry::load()?};
  let version=unsafe {entry.try_enumerate_instance_version()?}.unwrap_or(vk::API_VERSION_1_0);
  if version<vk::API_VERSION_1_1{return Err("Headless context requires Vulkan1.1".into());}
  let name=CString::new("IngeTrazo native GPU evidence")?;
  let application=vk::ApplicationInfo::default().application_name(&name).api_version(vk::API_VERSION_1_1);
  let instance=unsafe {entry.create_instance(&vk::InstanceCreateInfo::default().application_info(&application),None)?};
  let result=(|| {
   let mut candidates=vec![];
   for (index,physical) in unsafe {instance.enumerate_physical_devices()?}.into_iter().enumerate() {
    let properties=unsafe {instance.get_physical_device_properties(physical)};
    let name=unsafe {CStr::from_ptr(properties.device_name.as_ptr())}.to_string_lossy().into_owned();
    let graphics=unsafe {instance.get_physical_device_queue_family_properties(physical)}.iter().enumerate().find(|(_,q)|q.queue_count>0&&q.queue_flags.contains(vk::QueueFlags::GRAPHICS)).map(|(i,_)|i as u32);
    eprintln!("Headless adapter{index}: {name}; vendor0x{:04x}; {:?}; API{}.{}.{}; driver0x{:x}; maxImage2D{}; graphics{:?}",properties.vendor_id,properties.device_type,vk::api_version_major(properties.api_version),vk::api_version_minor(properties.api_version),vk::api_version_patch(properties.api_version),properties.driver_version,properties.limits.max_image_dimension2_d,graphics);
    if request.accepts(properties.vendor_id,properties.device_type,&name) {if let Some(graphics)=graphics {candidates.push((queues::rank(properties.device_type),std::cmp::Reverse(index),physical,graphics,name));}}
   }
   candidates.sort_by_key(|(rank,index,_,_,_)|(*rank,*index));
   let (_,_,physical,graphics,name)=candidates.pop().ok_or("No headless adapter matches strict vendor/type/name request; software fallback refused")?;
   let extensions=unsafe {instance.enumerate_device_extension_properties(physical)?};
   let enabled=if extensions.iter().any(|e|unsafe {CStr::from_ptr(e.extension_name.as_ptr())}==ash::ext::memory_budget::NAME){vec![ash::ext::memory_budget::NAME.as_ptr()]}else{vec![]};
   let priorities=[1.];let queues=[vk::DeviceQueueCreateInfo::default().queue_family_index(graphics).queue_priorities(&priorities)];
   let available=unsafe{instance.get_physical_device_features(physical)};let texture_bc_enabled=enable_bc&&available.texture_compression_bc!=0;let features=vk::PhysicalDeviceFeatures::default().shader_clip_distance(available.shader_clip_distance!=0).texture_compression_bc(texture_bc_enabled);
   let device=unsafe {instance.create_device(physical,&vk::DeviceCreateInfo::default().queue_create_infos(&queues).enabled_extension_names(&enabled).enabled_features(&features),None)?};
   let pool=match unsafe {device.create_command_pool(&vk::CommandPoolCreateInfo::default().queue_family_index(graphics).flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),None)} {Ok(pool)=>pool,Err(e)=>{unsafe {device.destroy_device(None)};return Err(Box::<dyn Error>::from(e));}};
   let queue=unsafe {device.get_device_queue(graphics,0)};
   eprintln!("Selected headless adapter {name}; strict request {:?}; no surface/present requirement",request);
   Ok((physical,graphics,device,pool,queue,texture_bc_enabled))
  })();
  let (physical,graphics,device,pool,queue,texture_bc_enabled)=match result {Ok(values)=>values,Err(e)=>{unsafe {instance.destroy_instance(None)};return Err(e);}};
  let debug_utils=debug_utils::Instance::new(&entry,&instance);let surface=surface::Instance::new(&entry,&instance);
  Ok(Self{startup_audit:Default::default(),lifetime:std::sync::Arc::new(super::lifetime::Lifetime::new(device.clone(),queue)),_entry:Some(entry),instance,debug_utils,debug_utils_messenger:vk::DebugUtilsMessengerEXT::null(),surface,surface_khr:vk::SurfaceKHR::null(),physical_device:physical,graphics_q_index:graphics,present_q_index:graphics,device,graphics_queue:queue,present_queue:queue,command_pool:pool,present_fences_enabled:false,texture_bc_enabled,transfer_timeout_ns:5_000_000_000,diagnostic_fail_fast:true})
 }
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn strict_hardware_request_never_substitutes_software_or_other_vendor(){let r=DeviceRequest::nvidia_hardware();assert!(!r.accepts(0x10005,vk::PhysicalDeviceType::CPU,"llvmpipe RTX"));assert!(!r.accepts(0x1002,vk::PhysicalDeviceType::DISCRETE_GPU,"AMD"));assert!(!r.accepts(0x10de,vk::PhysicalDeviceType::INTEGRATED_GPU,"NVIDIA"));assert!(r.accepts(0x10de,vk::PhysicalDeviceType::DISCRETE_GPU,"RTX5090"));}
}
