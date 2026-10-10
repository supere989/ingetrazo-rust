#[cfg(any(target_os="linux",target_os="windows"))]
mod platform {
use ash::vk;
use glam::DVec3;
use ingetrazo_gui::native_vulkan::{VulkanContext,headless::DeviceRequest,budget::VulkanBudget,world::World};
use ingetrazo_core::{scene::Scene,camera::{OrbitCamera,StandardView}};
use std::sync::Arc;
pub fn main()->Result<(),Box<dyn std::error::Error>>{
 let software=std::env::args().any(|x|x=="--software-fixture");
 let context=VulkanContext::new_headless(&if software{DeviceRequest::software_fixture()}else{DeviceRequest::nvidia_hardware()})?;
 let budget=Arc::new(VulkanBudget::new(&context.instance,context.physical_device)?);
 budget.ledger.lock().unwrap().total_limit=Some(128*1024*1024);
 let fence=unsafe{context.device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}?;
 let commands=unsafe{context.device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))}?;
 let command=commands[0];
 let mut scene=Scene::new();
 let ids=[DVec3::new(-1.,-1.,0.),DVec3::new(1.,-1.,0.),DVec3::new(1.,1.,0.),DVec3::new(-1.,1.,0.)].map(|p|scene.mesh.add_vertex(p));
 scene.mesh.add_face(ids.to_vec(),vec![]).unwrap();
 let mut camera=OrbitCamera::default();camera.set_standard_view(StandardView::Top);camera.target=DVec3::ZERO;camera.distance=4.;camera.perspective=false;camera.aspect=1.;
 let style=ingetrazo_render::style::StyleConfig::default();
 let mut world=World::new(&context,budget.clone())?;world.resize_offscreen(&context,vk::Extent2D{width:64,height:64})?;
 world.set_scene(&context,&scene,&camera,&style,fence,0,1)?;
 // Convert the actual native 0..1 depth matrix to source GL -1..1 depth.
 // These robust front/behind controls do not assert Qt matrix/resolve equality.
 let mut m=(camera.projection_matrix()*camera.view_matrix()).as_mat4().to_cols_array();
 for c in 0..4{m[c*4+2]=2.*m[c*4+2]-m[c*4+3];}
 let inverse=glam::Mat4::from_cols_array(&m).inverse().to_cols_array();
 let eye=camera.eye().as_vec3().to_array();let forward=camera.forward().as_vec3().to_array();
 let points=[[0.,0.,1.],[0.,0.,-1.],[0.,0.,0.],[3.,3.,-1.]];
 let(_,samples,extent)=world.annotation_depth_info().ok_or("No retained sampled depth")?;
 let mut outputs=Vec::new();
 for sample in 0..samples.as_raw(){
  let serial=sample as u64+1;
  world.prepare_annotation_queries_for_probe(&context,&points,m,inverse,eye,forward,sample,fence,serial-1)?;
  assert_eq!(world.annotation_query_uploads_for_probe(),1,"Unchanged query geometry uploaded again");
  unsafe{context.device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty())?;context.device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default())?;}
  world.record(command,serial)?;world.record_annotation_queries_for_probe(command,serial)?;
  unsafe{context.device.end_command_buffer(command)?;context.device.reset_fences(&[fence])?;context.device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&commands)],fence)?;}
  context.wait_transfer(fence)?;
  let bytes=world.readback_annotation_queries_for_probe(&context)?;
  let flags=bytes.chunks_exact(4).map(|v|u32::from_ne_bytes(v.try_into().unwrap())).collect::<Vec<_>>();
  assert_eq!(flags,vec![0,1,0,0],"Actual retained depth sample {sample}");
  outputs.push(flags);
 }
 assert!(world.prepare_annotation_queries_for_probe(&context,&points,m,inverse,eye,forward,samples.as_raw(),fence,samples.as_raw() as u64).is_err());
 assert_eq!(world.annotation_query_uploads_for_probe(),1);
 // A resized target invalidates the immutable image-view binding before dispatch.
 world.resize_offscreen(&context,vk::Extent2D{width:32,height:32})?;
 assert!(world.record_annotation_queries_for_probe(command,99).is_err());
 drop(world);assert_eq!(budget.ledger.lock().unwrap().total_committed(),0,"Annotation/depth GPU owner leak");
 unsafe{context.device.free_command_buffers(context.command_pool,&commands);context.device.destroy_fence(fence,None);}
 println!("{}",serde_json::json!({"scope":"actual retained depth sampling; not original MSAA resolve policy or annotation overlay proof","samples":samples.as_raw(),"extent":[extent.width,extent.height],"flags":outputs,"warm_query_uploads":1,"invalid_sample_refused":true,"stale_target_refused":true,"final_ledger":0}));
 Ok(())
}
}
#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->impl std::process::Termination{platform::main()}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("This Vulkan diagnostic requires Linux/Windows; macOS uses native Metal.");std::process::exit(2);}
