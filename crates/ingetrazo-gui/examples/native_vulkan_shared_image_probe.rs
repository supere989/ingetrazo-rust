//! Actual shared face/reference image ownership, removal and pressure proof.
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{budget::VulkanBudget,headless::DeviceRequest,world::World,VulkanContext};
use std::sync::Arc;
fn draw(context:&VulkanContext,world:&mut World,command:vk::CommandBuffer,fence:vk::Fence,serial:u64)->Result<Vec<u8>,Box<dyn std::error::Error>>{
 unsafe{context.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;}
 world.record(command,serial)?;
 unsafe{context.device.end_command_buffer(command)?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&[command])],fence)?;}
 context.wait_transfer(fence)?;
 Ok(world.readback(context)?)
}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().collect::<Vec<_>>();
 let context=VulkanContext::new_headless(&DeviceRequest::nvidia_hardware())?;
 let properties=unsafe{context.instance.get_physical_device_properties(context.physical_device)};
 let device_name=unsafe{std::ffi::CStr::from_ptr(properties.device_name.as_ptr())}.to_string_lossy().into_owned();
 let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)?};
 let command=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))?}[0];
 let original=ingetrazo_formats::igz::load_igz(&std::path::Path::new(&args[1]).join("image-quadrants.igz"))?;
 let path=original.image_planes[0].path.clone();
 let mut records=Vec::new();
 for remove_plane_first in [true,false]{
  let mut scene=original.clone();
  let vertices=[DVec3::new(-1.,-1.,0.1),DVec3::new(1.,-1.,0.1),DVec3::new(1.,1.,0.1),DVec3::new(-1.,1.,0.1)].map(|p|scene.mesh.add_vertex(p));
  let face=scene.mesh.add_face(vertices.to_vec(),vec![]).unwrap();
  scene.mesh.faces[face].attrs.insert("texture".into(),serde_json::json!({"path":path,"uvw":[0.5,0.,0.,0.5,0.,0.5,0.,0.5]}));
  let mut camera=ingetrazo_core::camera::OrbitCamera::default();camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);camera.target=DVec3::ZERO;camera.distance=4.;camera.perspective=false;camera.aspect=800./600.;
  let mut style=ingetrazo_render::style::StyleConfig::default();style.show_textures=true;style.show_edges=false;style.show_sky=false;style.show_profiles=false;style.background_color=[1.,1.,1.,1.];
  let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:800,height:600})?;
  world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
  assert_eq!(world.texture_stats().1.uploads,1,"shared face+plane must allocate one image");
  let shared=draw(&context,&mut world,command,fence,1)?;
  let old=budget.ledger.lock().unwrap().total_committed();
  let mut replacement=scene.clone();for bytes in replacement.embedded_textures.values_mut(){bytes.push(0);}replacement.version+=1;
  world.set_texture_pressure_for_probe(true);
  let error=world.set_scene(&context,&replacement,&camera,&style,fence,1,2).expect_err("real shared replacement pressure must refuse");
  world.set_texture_pressure_for_probe(false);
  assert!(error.contains("Reference image admission"),"{error}");
  assert_eq!(budget.ledger.lock().unwrap().total_committed(),old);
  assert_eq!(draw(&context,&mut world,command,fence,2)?,shared,"failed joint replacement preserves every old sampled pixel");
  if remove_plane_first{scene.image_planes.clear();}else{scene.mesh=ingetrazo_core::mesh::Mesh::new();}
  scene.version+=1;world.set_scene(&context,&scene,&camera,&style,fence,2,3)?;
  assert_eq!(world.texture_stats().1.uploads,1,"surviving consumer must reuse image");
  assert_eq!(world.texture_stats().1.retired,0,"live shared asset cannot retire");
  let survivor=draw(&context,&mut world,command,fence,3)?;
  assert!(survivor.chunks_exact(4).any(|p|p!=[255,255,255,255]),"surviving sampled consumer must actually draw");
  // A plane is behind the shared opaque face; removing it must not alter the
  // full visible framebuffer. Removing the face reveals the unlit plane.
  if remove_plane_first{assert_eq!(survivor,shared,"face draw survives plane removal exactly");}
  scene.image_planes.clear();scene.mesh=ingetrazo_core::mesh::Mesh::new();scene.version+=1;
  world.set_scene(&context,&scene,&camera,&style,fence,3,4)?;
  assert_eq!(world.texture_stats().1.retired,1,"last completed consumer removal retires once");
  let empty=draw(&context,&mut world,command,fence,4)?;
  assert!(empty.chunks_exact(4).all(|p|p==[255,255,255,255]));
  records.push(serde_json::json!({"remove_plane_first":remove_plane_first,"image_uploads":world.texture_stats().1.uploads,"retired":world.texture_stats().1.retired,"shared_replacement_pressure":error,"old_pixels_preserved":true}));
  drop(world);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0);
 }
 unsafe{context.device.free_command_buffers(context.command_pool,&[command]);context.device.destroy_fence(fence,None);}
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"renderer_device":{"name":device_name,"vendor_id":properties.vendor_id,"device_id":properties.device_id,"device_type":format!("{:?}",properties.device_type),"software_fixture_requested":false},"cases":records,"ledger_final":budget.ledger.lock().unwrap().total_committed()}))?);
 Ok(())
}
