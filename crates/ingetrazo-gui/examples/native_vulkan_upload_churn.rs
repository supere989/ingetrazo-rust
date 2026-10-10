//! Actual upload/retirement churn, no automatic throughput claims.
use ash::vk;
use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,resources::Resources};
use std::{sync::Arc,time::Instant};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();
 let request=if args.iter().any(|a|a=="--software-fixture"){DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()};
 let context=VulkanContext::new_headless(&request)?;
 let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
 let mut rows=Vec::new();
 for (name,bytes,count) in [("tiny32",36usize,32usize),("packed32",36*32,1),("small32",4096,32),("large17MiB",17*1024*1024,1)]{
  for repeat in 0..5 {
   let data=(0..bytes).map(|i|(i%251)as u8).collect::<Vec<_>>();
   let mut resources=Resources::new(&context,budget.clone());
   let started=Instant::now();let mut ids=Vec::new();
   for _ in 0..count{ids.push(resources.upload(&context,&data,vk::BufferUsageFlags::STORAGE_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC,true)?);}
   let upload_ms=started.elapsed().as_secs_f64()*1000.;
   let retained=budget.ledger.lock().unwrap().total_committed();
   let upload_stats=serde_json::to_value(&resources.stats)?;
   // Verify every uploaded buffer by actual GPU-to-host transfers outside the timing window.
   for &id in &ids{assert_eq!(resources.readback_buffer(&context,id)?,data);}
   let retirement=Instant::now();
   for id in ids{resources.retire(id)?;}
   assert_eq!(resources.collect(fence,0)?,count);
   let retirement_ms=retirement.elapsed().as_secs_f64()*1000.;
   assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
   assert_eq!(resources.stats.upload_calls,count as u64);
   assert_eq!(resources.stats.staging_allocations,count as u64);
   assert_eq!(resources.stats.transfer_fence_creations,count as u64);
   rows.push(serde_json::json!({"case":name,"repeat":repeat,"logical_bytes":bytes*count,"upload_ms":upload_ms,"retirement_ms":retirement_ms,"charged_retained_after_upload":retained,"upload_stats":upload_stats,"stats_including_readback":resources.stats,"final_ledger":0}));
  }
 }
 unsafe{context.device.destroy_fence(fence,None)};
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"passed":true,"rows":rows,"scope":"Strict actual adapter; synchronous upload walltime excluding readback. No workers/graphics throughput claim; tiny vs packed buffers differ resource ownership."}))?);Ok(())
}
