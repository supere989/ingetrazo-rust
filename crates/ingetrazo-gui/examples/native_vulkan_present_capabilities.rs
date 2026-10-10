#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Actual adapter/loader capability query; no WSI support promotion.
use ash::{vk,Entry};
use std::ffi::CStr;
use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest};
pub fn main()->Result<(),Box<dyn std::error::Error>>{
 let software=std::env::args().any(|s|s=="--software-fixture");
 let context=VulkanContext::new_headless(&if software{DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()})?;
 let entry=unsafe{Entry::load()?};
 let ie=unsafe{entry.enumerate_instance_extension_properties(None)?};
 let de=unsafe{context.instance.enumerate_device_extension_properties(context.physical_device)?};
 let has=|name:&CStr|de.iter().any(|e|unsafe{CStr::from_ptr(e.extension_name.as_ptr())}==name);
 let surface=ie.iter().any(|e|unsafe{CStr::from_ptr(e.extension_name.as_ptr())}==ash::ext::surface_maintenance1::NAME);
 let maintenance=has(ash::ext::swapchain_maintenance1::NAME);
 let mut feature=vk::PhysicalDeviceSwapchainMaintenance1FeaturesEXT::default();
 if maintenance{let mut features=vk::PhysicalDeviceFeatures2::default().push_next(&mut feature);unsafe{context.instance.get_physical_device_features2(context.physical_device,&mut features);}}
 println!("actual_surface_maintenance1_extension={surface} actual_swapchain_maintenance1_extension={maintenance} actual_swapchain_maintenance1_feature={} present_wait_extension={} present_id_extension={}",feature.swapchain_maintenance1!=0,has(ash::khr::present_wait::NAME),has(ash::khr::present_id::NAME));
 Ok(())
}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
