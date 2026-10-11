#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ash::vk;use std::sync::Arc;use image::ImageEncoder;use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,world::World};
 let args=std::env::args().collect::<Vec<_>>();let root=std::path::Path::new(&args[1]);let output=std::path::Path::new(&args[2]);std::fs::create_dir_all(output)?;
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;let command=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))}?[0];
 let small=ingetrazo_formats::igz::load_igz(&root.join("batch.igz"))?;
 let mut large=ingetrazo_formats::igz::load_igz(&root.join("batch.igz"))?;let mut names=large.embedded_textures.keys().cloned().collect::<Vec<_>>();names.sort();let mut encoded_hashes=Vec::new();
 for (i,name)in names.iter().enumerate(){if name.ends_with("asset-4.png"){continue;}let raw=[37+i as u8*13,93+i as u8*17,157-i as u8*11,255].repeat(1024*512);let mut encoded=Vec::new();image::codecs::png::PngEncoder::new(&mut encoded).write_image(&raw,1024,512,image::ExtendedColorType::Rgba8)?;encoded_hashes.push(blake3::hash(&encoded).to_hex().to_string());large.embedded_textures.insert(name.clone(),encoded);}
 let mut camera=ingetrazo_core::camera::OrbitCamera::default();camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);camera.target=glam::DVec3::ZERO;camera.distance=4.;camera.aspect=800./600.;camera.perspective=false;
 let mut style=ingetrazo_render::style::StyleConfig::default();style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;style.show_edges=false;style.show_profiles=false;style.show_sky=false;style.background_color=[1.,1.,1.,1.];
 let expected=image::open(root.join("near.png"))?.into_rgba8().into_raw();let mut rows=Vec::new();
 for (name,scene)in [("small",&small),("large",&large)]{
  for iteration in 0..2{
   let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:800,height:600})?;
   let start=std::time::Instant::now();world.set_scene(&context,scene,&camera,&style,fence,0,1)?;let elapsed=start.elapsed().as_secs_f64()*1000.;
   let (geometry,texture)=world.texture_stats();let phases=serde_json::json!({"hash_ms":texture.hash_ms,"decode_ms":texture.decode_ms,"orientation_ms":texture.orientation_ms,"compression_ms":texture.compression_ms,"image_transfer_ms":texture.upload_ms,"vertex_uploads":geometry.uploads,"image_uploads":texture.uploads,"bc7_uploads":texture.bc7_uploads});let stats=world.resource_upload_stats_for_probe();assert_eq!(texture.uploads,5);assert_eq!(texture.bc7_uploads,1);
   unsafe{context.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;}world.record(command,1)?;unsafe{context.device.end_command_buffer(command)?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence)?;}context.wait_transfer(fence)?;let pixels=world.readback(&context)?;if name=="small"{assert_eq!(pixels,expected,"Unchanged actual original whole image");}
   let pixel_hash=blake3::hash(&pixels).to_hex().to_string();let resident=budget.ledger.lock().unwrap().total_committed();let before=world.resource_upload_stats_for_probe();world.set_scene(&context,scene,&camera,&style,fence,1,1)?;assert_eq!(world.resource_upload_stats_for_probe().transfer_submissions,before.transfer_submissions);
   drop(world);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
   rows.push(serde_json::json!({"set":name,"iteration":iteration,"driver_warmup":iteration==0,"cold_set_scene_ms":elapsed,"phases":phases,"resources":stats,"resident_bytes":resident,"pixel_blake3":pixel_hash,"warm_extra_submissions":0,"ledger_final":0}));
  }
 }
 unsafe{context.device.free_command_buffers(context.command_pool,&[command]);context.device.destroy_fence(fence,None);}
 std::fs::write(output.join("perf.json"),serde_json::to_vec_pretty(&serde_json::json!({"source":option_env!("INGETRAZO_BUILD_SOURCE"),"rows":rows,"large_encoded_blake3":encoded_hashes,"boundary":"Instant immediately before full World::set_scene through successful return; fresh World/cache/resources each iteration; same device/driver warmed first iteration; render/readback excluded"}))?)?;Ok(())
}
#[cfg(not(any(target_os="linux",target_os="windows")))]fn main(){eprintln!("Vulkan diagnostic; macOS uses Metal");std::process::exit(2);}
