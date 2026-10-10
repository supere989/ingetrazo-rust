//! Bounded, strict hardware evidence using production context/resources/world commands.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,resources::Resources,world::World};
use ingetrazo_core::{scene::Scene,camera::{OrbitCamera,StandardView}};
use std::{sync::Arc,time::Instant};
fn pixel(bytes:&[u8],x:usize,y:usize)->&[u8]{let i=(y*800+x)*4;&bytes[i..i+3]}
fn red(c:&[u8])->bool{c[0]>=180&&c[1]<10&&c[2]<10}fn blue(c:&[u8])->bool{c[2]>200&&c[0]<10&&c[1]<10}
fn main()->Result<(),Box<dyn std::error::Error>> {
 let args=std::env::args().skip(1).collect::<Vec<_>>();
 let mut request=if args.iter().any(|a|a=="--software-fixture"){DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()};
 if let Some(value)=args.windows(2).find(|a|a[0]=="--vendor").map(|a|&a[1]) {request.vendor_id=Some(if let Some(hex)=value.strip_prefix("0x"){u32::from_str_radix(hex,16)?}else{value.parse()?});}
 if let Some(value)=args.windows(2).find(|a|a[0]=="--device-name").map(|a|&a[1]) {request.name=Some(value.clone());}
 let output=args.windows(2).find(|a|a[0]=="--output").map(|a|std::path::PathBuf::from(&a[1])).unwrap_or_else(||"/tmp/ingetrazo-headless-depth.png".into());
 let context=VulkanContext::new_headless(&request)?;
 let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 {let mut ledger=budget.ledger.lock().unwrap();ledger.total_limit=Some(128*1024*1024);}
 if args.iter().any(|a|a=="--timeout-control") {let fence=unsafe {context.device.create_fence(&vk::FenceCreateInfo::default(),None)}?;context.wait_transfer(fence)?;return Err("Unsignaled timeout control unexpectedly completed".into());}
 let mut resources=Resources::new(&context,budget.clone());
 let no_readback=resources.upload(&context,&[0u8;4],vk::BufferUsageFlags::VERTEX_BUFFER,true)?;
 let charged=budget.ledger.lock().unwrap().total_committed();
 assert!(resources.readback_buffer(&context,no_readback).unwrap_err().contains("not created for readback"));
 assert_eq!(budget.ledger.lock().unwrap().total_committed(),charged,"Rejected readback allocated staging");
 resources.retire(no_readback)?;
 let completed=unsafe {context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
 assert_eq!(resources.collect(completed,0)?,1);unsafe {context.device.destroy_fence(completed,None)};
 eprintln!("PASS ordinary GPU buffer rejects undeclared readback before staging allocation");
 let expected=(0..17*1024*1024).map(|i|(i%251)as u8).collect::<Vec<_>>();let started=Instant::now();
 let buffer=resources.upload(&context,&expected,vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC,true)?;
 assert_eq!(resources.stats.transfer_command_allocations,1,"Repeated buffer upload recreated command buffer");
 assert_eq!(resources.stats.transfer_fence_creations,1,"Repeated buffer upload recreated transfer fence");
 let upload_ms=started.elapsed().as_secs_f64()*1000.;let started=Instant::now();let copied=resources.readback_buffer(&context,buffer)?;
 assert_eq!(copied,expected,"Actual GPU-to-host copy differs; device-local mapping is not used");
 let readback_ms=started.elapsed().as_secs_f64()*1000.;assert!(resources.stats.staging_peak<=8*1024*1024&&resources.stats.readback_staging_peak<=8*1024*1024);
 let batch_a=(0..1024).map(|i|(i%239)as u8).collect::<Vec<_>>();
 let batch_b=(0..4096).map(|i|(i%197)as u8).collect::<Vec<_>>();
 let batch=resources.upload_many(&context,&[(&batch_a,vk::BufferUsageFlags::VERTEX_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC),(&batch_b,vk::BufferUsageFlags::INDEX_BUFFER|vk::BufferUsageFlags::TRANSFER_SRC)])?;
 assert_eq!(resources.readback_buffer(&context,batch[0])?,batch_a);assert_eq!(resources.readback_buffer(&context,batch[1])?,batch_b);
 assert_eq!(resources.stats.transfer_command_allocations,1,"Batch upload recreated command buffer");assert_eq!(resources.stats.transfer_fence_creations,1,"Batch upload recreated transfer fence");
 let done=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
 for id in batch{resources.retire(id)?;}assert_eq!(resources.collect(done,0)?,2);unsafe{context.device.destroy_fence(done,None)};
 eprintln!("PASS batched buffer exact readback shares single transfer owner");
 let device=&context.device;let fence=unsafe {device.create_fence(&vk::FenceCreateInfo::default(),None)}?;
 resources.mark_used(buffer,7)?;resources.retire(buffer)?;let charged=budget.ledger.lock().unwrap().total_committed();assert_eq!(resources.collect(fence,7)?,0);assert_eq!(budget.ledger.lock().unwrap().total_committed(),charged);
 unsafe {device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default()],fence)?;}
 context.wait_transfer(fence)?;assert_eq!(resources.collect(fence,7)?,1);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
 eprintln!("PASS17MiB actualGPUcopy/readback; upload{upload_ms:.3}ms readback{readback_ms:.3}ms; stats{:?}; fence-delayed actualfree",resources.stats);
 let mut scene=Scene::new();
 for (points,color) in [([DVec3::new(-1.,-1.,1.),DVec3::new(1.,-1.,-1.),DVec3::new(0.,1.,0.)],[1.,0.,0.,1.]),([DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(0.,1.,0.)],[0.,0.,1.,1.])] {
  let vertices=points.map(|p|scene.mesh.add_vertex(p));let face=scene.mesh.add_face(vertices.to_vec(),vec![]).unwrap();scene.mesh.faces[face].attrs.insert("color".into(),serde_json::json!(color));
 }
 let mut camera=OrbitCamera::default();camera.set_standard_view(StandardView::Top);camera.target=DVec3::ZERO;camera.distance=4.;camera.perspective=false;camera.aspect=800./600.;
 let style=ingetrazo_render::style::StyleConfig::default();let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:800,height:600})?;
 let commands=unsafe {device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).level(vk::CommandBufferLevel::PRIMARY).command_buffer_count(1))}?;let command=commands[0];
 for serial in 1..=2 {
  if serial==2 {for vertex in &mut scene.mesh.vertices[..3]{vertex.position.z=-vertex.position.z;}scene.bump_version();}
  world.set_scene(&context,&scene,&camera,&style,fence,serial-1,serial)?;
  world.set_scene(&context,&scene,&camera,&style,fence,serial-1,serial)?;assert_eq!(world.uploads,serial,"Unchanged cache key re-uploaded geometry");
  unsafe {device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;}
  world.record(command,serial)?;
  unsafe {device.end_command_buffer(command)?;device.reset_fences(&[fence])?;device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&commands)],fence)?;}
  context.wait_transfer(fence)?;let image=world.readback(&context)?;
  let left=pixel(&image,350,350);let right=pixel(&image,450,350);
  if serial==1 {assert!(red(left)&&left[0]>200&&blue(right),"GPU depth failed: left{left:?} right{right:?}");image::save_buffer(&output,&image,800,600,image::ColorType::Rgba8)?;}else{assert!(blue(left)&&red(right),"GPU geometry invalidation failed: left{left:?} right{right:?}");}
  eprintln!("PASS native world draw{serial} actualGPUreadback left{left:?} right{right:?}; uploads{}",world.uploads);
 }
 unsafe {device.free_command_buffers(context.command_pool,&commands);device.destroy_fence(fence,None);}
 eprintln!("{}",budget.diagnostics());
 drop(world);drop(resources);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0,"Production GPU owners leaked allocations");
 println!("PASS headless strict adapter {:?}; actual17MiB GPUupload/readback+fence/free; productionworld depth/cache/invalidation; global128MiB limit; eachfence5s; image{}",request,output.display());
 Ok(())
}
