#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Engineering fault: an actual host-reset event holds the GPU queue until a
//! finite transfer wait expires. This is NOT a device-loss or hardware-hang test.
use ash::vk;
use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,resources::{Resources,TextureImage}};
pub fn main()->Result<(),Box<dyn std::error::Error>>{
 let arguments=std::env::args().collect::<Vec<_>>();let hardware=arguments.iter().any(|s|s=="--hardware");
 let kind=arguments.iter().find_map(|s|s.strip_prefix("--kind=")).unwrap_or("image");
 assert!(["image","buffer","batch","readback"].contains(&kind));
 let mut context=VulkanContext::new_headless(&if hardware{DeviceRequest::nvidia_hardware()}else{DeviceRequest::software_fixture()})?;
 context.diagnostic_fail_fast=false;context.transfer_timeout_ns=50_000_000;
 let budget=std::sync::Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 let mut resources=Resources::new(&context,budget.clone());
 let old=resources.upload(&context,&[1u8;4096],vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC,true)?;
 let before=budget.ledger.lock().unwrap().total_committed();
 let event=unsafe{context.device.create_event(&vk::EventCreateInfo::default(),None)?};
 unsafe{context.device.reset_event(event)?;}
 let command=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))?[0]};
 let held=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default(),None)?};
 unsafe{
  context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;
  context.device.cmd_wait_events(command,&[event],vk::PipelineStageFlags::HOST,vk::PipelineStageFlags::TRANSFER,&[],&[],&[]);
  context.device.end_command_buffer(command)?;
  context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],held)?;
 }
 let start=std::time::Instant::now();
 let failed=match kind{
  "image"=>TextureImage::rgba8(&context,&mut resources,vk::Extent2D{width:64,height:64},&[255u8;64*64*4],false,false).is_err(),
  "buffer"=>resources.upload(&context,&[2u8;4096],vk::BufferUsageFlags::VERTEX_BUFFER,true).is_err(),
  "batch"=>resources.upload_many(&context,&[(&[2u8;4096],vk::BufferUsageFlags::VERTEX_BUFFER),(&[3u8;4096],vk::BufferUsageFlags::INDEX_BUFFER)]).is_err(),
  "readback"=>resources.readback_buffer(&context,old).is_err(),_=>unreachable!()
 };
 let elapsed=start.elapsed();
 assert!(failed,"Actual queued image upload unexpectedly completed behind reset host event");
 assert!(context.lifetime.uncertain());assert!(elapsed.as_secs_f64()<2.);
 let retained=budget.ledger.lock().unwrap().total_committed();assert!(retained>before);
 assert!(resources.buffer(old).is_some());
 assert!(resources.upload(&context,&[2u8;4096],vk::BufferUsageFlags::VERTEX_BUFFER,true).is_err());
 let destroyed=resources.stats.buffer_destructions;
 drop(resources);
 assert_eq!(budget.ledger.lock().unwrap().total_committed(),retained,"Timeout Drop freed charged in-flight resources");
 // Release the controlled queue hold, and independently confirm actual execution
 // completion. Production poison stays permanent; no automatic recovery claimed.
 unsafe{
  context.device.set_event(event)?;
  context.device.wait_for_fences(&[held],true,5_000_000_000)?;
  let complete=context.device.create_fence(&vk::FenceCreateInfo::default(),None)?;
  context.device.queue_submit(context.graphics_queue,&[],complete)?;
  context.device.wait_for_fences(&[complete],true,5_000_000_000)?;
  context.device.destroy_fence(complete,None);context.device.destroy_fence(held,None);
  context.device.free_command_buffers(context.command_pool,&[command]);context.device.destroy_event(event,None);
 }
 assert!(context.lifetime.uncertain());assert_eq!(budget.ledger.lock().unwrap().total_committed(),retained);
 let weak_budget=std::sync::Arc::downgrade(&budget);drop(budget);
 let kept_budget=weak_budget.upgrade().expect("Quarantine dropped the last memory ledger owner");
 assert_eq!(kept_budget.ledger.lock().unwrap().total_committed(),retained);
 assert!(context.lifetime.retained_owners()>=2);

 println!("PASS actual queued timeout: kind={} elapsed_ms={} retained_bytes={} before_bytes={} prior_buffer_destructions={} old_buffer_preserved=true new_admission_refused=true drop_frees_pending=false subsequent_real_completion=true automatic_recovery=false last_external_ledger_drop_retained=true",kind,elapsed.as_millis(),retained,before,destroyed);
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
