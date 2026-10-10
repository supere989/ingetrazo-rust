//! Diagnostic production World rendering of complete native SKP imports.
//! Annotation UI overlays are not emitted by World and remain a required gap.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{budget::VulkanBudget,headless::DeviceRequest,world::World,VulkanContext};
use std::sync::Arc;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();
 let input=std::path::PathBuf::from(args.get(1).ok_or("input directory required")?);
 let output=std::path::PathBuf::from(args.get(2).ok_or("output directory required")?);
 std::fs::create_dir_all(&output)?;
 let protocol:serde_json::Value=serde_json::from_slice(&std::fs::read(input.join("protocol.json"))?)?;
 let cases=protocol["cases"].as_array().ok_or("missing cases")?;
 let names=cases.iter().map(|c|c["name"].as_str().ok_or("case name")).collect::<Result<Vec<_>,_>>()?;
 if names!=["gondola_v20-iso","gondola_v20-asymmetric","capilla_quiroz_v17-iso","capilla_quiroz_v17-asymmetric"] {return Err("mandatory four-case corpus required".into());}
 let request=if args.iter().any(|a|a=="--software-fixture"){DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()};
 let context=VulkanContext::new_headless(&request)?;
 let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(256*1024*1024);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)?};
 let commands=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))?};
 let mut rows=Vec::new();
 for case in cases{
  let name=case["name"].as_str().unwrap();let model=case["model"].as_str().ok_or("model name")?;
  let scene=ingetrazo_formats::igz::load_igz(&input.join(format!("{model}.igz")))?;
  let c=&case["camera"];let number=|k:&str|c[k].as_f64().ok_or("missing numeric camera field");
  let mut camera=ingetrazo_core::camera::OrbitCamera::default();
  camera.target=DVec3::new(c["target"][0].as_f64().ok_or("target")?,c["target"][1].as_f64().ok_or("target")?,c["target"][2].as_f64().ok_or("target")?);
  camera.distance=number("distance")?;camera.yaw=number("yaw")?;camera.pitch=number("pitch")?;camera.fov_deg=number("fov_deg")?;camera.perspective=false;camera.aspect=800./600.;
  let mut style=ingetrazo_render::style::StyleConfig::default();style.background_color=[1.;4];style.show_sky=false;style.show_edges=true;style.show_profiles=false;style.show_textures=false;
  let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:800,height:600})?;
  world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
  let cold=world.texture_stats();let cold=(cold.0.uploads,cold.1.hashes,cold.1.decodes,cold.1.uploads);world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
  assert_eq!(world.uploads,1);let warm=world.texture_stats();assert_eq!(cold,(warm.0.uploads,warm.1.hashes,warm.1.decodes,warm.1.uploads));
  unsafe{context.device.reset_command_buffer(commands[0],vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(commands[0],&vk::CommandBufferBeginInfo::default())?;}
  world.record(commands[0],1)?;
  unsafe{context.device.end_command_buffer(commands[0])?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&commands)],fence)?;}
  context.wait_transfer(fence)?;let pixels=world.readback(&context)?;assert_eq!(pixels.len(),800*600*4);
  image::save_buffer(output.join(format!("{name}-native-vulkan.png")),&pixels,800,600,image::ColorType::Rgba8)?;
  rows.push(serde_json::json!({"name":name,"geometry_uploads":world.uploads,"warm_uploads":0,"hard_edge_bytes":world.edge_geometry_bytes(),"dimensions":scene.dimensions.len(),"text_labels":scene.text_labels.len(),"scope":"production native World; annotation overlays not rendered"}));
  drop(world);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
 }
 unsafe{context.device.free_command_buffers(context.command_pool,&commands);context.device.destroy_fence(fence,None);}
 std::fs::write(output.join("native-world-results.json"),serde_json::to_vec_pretty(&serde_json::json!({"cases":rows,"ledger_final":0,"scope":"complete imported scene input, World-only drawing; strict pixels checked separately"}))?)?;
 Ok(())
}
