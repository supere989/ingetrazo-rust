#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ash::vk;use std::sync::Arc;use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,world::World};
 let args=std::env::args().collect::<Vec<_>>();let root=std::path::Path::new(&args[1]);let out=std::path::Path::new(&args[2]);std::fs::create_dir_all(out)?;
 let reference:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("original.json"))?)?;
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
 let command=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))}?[0];
 let mut scene=ingetrazo_formats::igz::load_igz(&root.join("batch.igz"))?;
 let mut camera=ingetrazo_core::camera::OrbitCamera::default();camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);camera.target=glam::DVec3::ZERO;camera.distance=4.;camera.aspect=800./600.;camera.perspective=false;
 let mut style=ingetrazo_render::style::StyleConfig::default();style.visual_style=ingetrazo_render::style::VisualStyle::ShadedWithTextures;style.show_edges=false;style.show_profiles=false;style.show_sky=false;style.background_color=[1.,1.,1.,1.];
 let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:800,height:600})?;
 let start=std::time::Instant::now();world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;let cold_ms=start.elapsed().as_secs_f64()*1000.;let cold=world.resource_upload_stats_for_probe();let texture=world.texture_stats();assert_eq!(texture.1.uploads,5);assert_eq!(texture.1.bc7_uploads,1);assert_eq!(cold.image_upload_calls,5);assert_eq!(cold.image_mips_generated,5+6+8+7);assert!(cold.staging_peak<=8*1024*1024);
 let mut rows=Vec::new();let mut serial=0;
 for case in reference["cases"].as_array().ok_or("cases")?{
  camera.distance=case["distance"].as_f64().ok_or("distance")?;world.set_scene(&context,&scene,&camera,&style,fence,serial,1)?;serial+=1;
  unsafe{context.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;}world.record(command,serial)?;unsafe{context.device.end_command_buffer(command)?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence)?;}context.wait_transfer(fence)?;
  let pixels=world.readback(&context)?;let name=case["case"].as_str().ok_or("name")?;image::save_buffer(out.join(format!("{name}.png")),&pixels,800,600,image::ColorType::Rgba8)?;
  let samples=case["points"].as_array().ok_or("points")?.iter().map(|point|{let offset=(point[1].as_u64().unwrap()as usize*800+point[0].as_u64().unwrap()as usize)*4;pixels[offset..offset+4].to_vec()}).collect::<Vec<_>>();
  rows.push(serde_json::json!({"case":name,"samples":samples}));
 }
 let before=world.resource_upload_stats_for_probe();let images_before=world.texture_stats().1.uploads;let entries_before=world.texture_stats().0.sampled_entries;
 let start=std::time::Instant::now();for _ in 0..20{world.set_scene(&context,&scene,&camera,&style,fence,serial,1)?;}let warm_ms=start.elapsed().as_secs_f64()*1000.;let warm=world.resource_upload_stats_for_probe();assert_eq!(before.transfer_submissions,warm.transfer_submissions);assert_eq!(images_before,world.texture_stats().1.uploads);
 let old=world.readback(&context)?;let old_bytes=budget.ledger.lock().unwrap().total_committed();for data in scene.embedded_textures.values_mut(){data.push(0);}scene.version+=1;world.set_texture_pressure_for_probe(true);let error=world.set_scene(&context,&scene,&camera,&style,fence,serial,2).expect_err("Actual image allocation pressure");world.set_texture_pressure_for_probe(false);assert!(error.contains("Texture image admission"),"{error}");assert_eq!(budget.ledger.lock().unwrap().total_committed(),old_bytes);assert_eq!(world.texture_stats().0.sampled_entries,entries_before);assert_eq!(world.readback(&context)?,old);
 world.set_scene(&context,&scene,&camera,&style,fence,serial,2)?;serial+=1;unsafe{context.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;}world.record(command,serial)?;unsafe{context.device.end_command_buffer(command)?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence)?;}context.wait_transfer(fence)?;assert_eq!(world.readback(&context)?,old);
 let final_stats=world.resource_upload_stats_for_probe();drop(world);unsafe{context.device.free_command_buffers(context.command_pool,&[command]);context.device.destroy_fence(fence,None);}assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
 std::fs::write(out.join("native.json"),serde_json::to_vec_pretty(&serde_json::json!({"source":option_env!("INGETRAZO_BUILD_SOURCE"),"cases":rows,"cold":cold,"warm":warm,"final":final_stats,"cold_ms":cold_ms,"warm20_ms":warm_ms,"resident_after_cold":old_bytes,"pressure_error":error,"old_frame_retained":true,"retry_exact":true,"ledger_final":0}))?)?;Ok(())
}
#[cfg(not(any(target_os="linux",target_os="windows")))]fn main(){eprintln!("Native Vulkan diagnostic; macOS uses Metal");std::process::exit(2);}
