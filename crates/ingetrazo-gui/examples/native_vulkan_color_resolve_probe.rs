#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,world::color_resolve_probe};
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;
 let budget=std::sync::Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 println!("{}",color_resolve_probe::run(&context,budget)?);Ok(())
}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Vulkan probe requires Linux/Windows; macOS uses native Metal.");std::process::exit(2);}
