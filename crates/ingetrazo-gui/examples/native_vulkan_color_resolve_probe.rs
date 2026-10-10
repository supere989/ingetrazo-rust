#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,world::color_resolve_probe};
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;
 let budget=std::sync::Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 let result=if let Some(path)=std::env::args().nth(1){
  let bytes=std::fs::read(path)?;if bytes.len()>1024*1024{return Err("Diagnostic input exceeds 1 MiB".into());}
  #[derive(serde::Deserialize)]struct Case{samples:[[u8;4];4]}
  let rows:Vec<Case>=serde_json::from_slice(&bytes)?;
  let samples:Vec<_>=rows.into_iter().map(|row|row.samples).collect();
  color_resolve_probe::run_cases(&context,budget,&samples)?
 }else{color_resolve_probe::run(&context,budget)?};
 println!("{}",result);Ok(())
}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Vulkan probe requires Linux/Windows; macOS uses native Metal.");std::process::exit(2);}
