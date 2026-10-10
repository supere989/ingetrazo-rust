//! Uses the actual World render pass/target and shaders with controlled sample masks.
//! No production policy or render path is changed by this diagnostic.
use super::*;
pub fn run(context:&VulkanContext,budget:Arc<VulkanBudget>)->Result<serde_json::Value,String>{
 let mut world=World::new(context,budget.clone())?;
 world.resize_offscreen(context,vk::Extent2D{width:8,height:8})?;
 if world.samples!=vk::SampleCountFlags::TYPE_4{return Err("Probe requires actual four-sample target".into());}
 let device=&context.device;
 let fence=unsafe{device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),None)}.map_err(|e|e.to_string())?;
 let commands=unsafe{device.allocate_command_buffers(&vk::CommandBufferAllocateInfo::default().command_pool(context.command_pool).command_buffer_count(1))}.map_err(|e|e.to_string())?;
 let command=commands[0];
 let mut pipelines=Vec::new();
 let result:Result<Vec<serde_json::Value>,String>=(||{
  for sample in 0..4{pipelines.push(make_pipeline_with_mask(device,world.render_pass,world.layout,WorldPass::Surface,world.samples,false,Some(&[1<<sample]))?);}
  let mut scene=ingetrazo_core::scene::Scene::new();
  let ids=[[-10.,-10.,0.],[10.,-10.,0.],[10.,10.,0.],[-10.,10.,0.]].map(|p|scene.mesh.add_vertex(glam::DVec3::from_array(p)));
  scene.mesh.add_face(ids.to_vec(),vec![]).ok_or("Diagnostic quad face refused")?;
  let mut camera=ingetrazo_core::camera::OrbitCamera::default();camera.set_standard_view(ingetrazo_core::camera::StandardView::Top);camera.target=glam::DVec3::ZERO;camera.distance=4.;camera.perspective=false;camera.aspect=1.;
  world.set_scene(context,&scene,&camera,&ingetrazo_render::style::StyleConfig::default(),fence,0,1)?;
  let vertices=world.vertices.ok_or("Missing diagnostic quad vertices")?;let indices=world.indices.ok_or("Missing diagnostic quad indices")?;
  let mut cases=vec![[33,33,33,107],[33,33,33,105],[50,50,51,51],[51,51,52,52],[100,100,101,101]];
  for base in [0u8,33]{for extra in 0..16{for slot in 0..4{let mut values=[base;4];values[slot]+=extra;cases.push(values);}}}
  let mut rows=Vec::new();
  for (case,values) in cases.into_iter().enumerate(){
   let serial=case as u64+1;
   let target=world.target.as_ref().unwrap();let area=vk::Rect2D{offset:vk::Offset2D::default(),extent:target.extent};
   let clears=[vk::ClearValue{color:vk::ClearColorValue{float32:[0.,0.,0.,1.]}},vk::ClearValue::default(),vk::ClearValue{depth_stencil:vk::ClearDepthStencilValue{depth:1.,stencil:0}}];
   unsafe{
    device.reset_command_buffer(command,vk::CommandBufferResetFlags::empty()).map_err(|e|e.to_string())?;
    device.begin_command_buffer(command,&vk::CommandBufferBeginInfo::default()).map_err(|e|e.to_string())?;
    device.cmd_begin_render_pass(command,&vk::RenderPassBeginInfo::default().render_pass(world.render_pass).framebuffer(target.framebuffer).render_area(area).clear_values(&clears),vk::SubpassContents::INLINE);
    device.cmd_set_viewport(command,0,&[vk::Viewport{x:0.,y:0.,width:8.,height:8.,min_depth:0.,max_depth:1.}]);device.cmd_set_scissor(command,0,&[area]);
    let mut push=[0u8;128];push[..64].copy_from_slice(bytemuck::cast_slice(&world.mvp));push[84..88].copy_from_slice(&1f32.to_ne_bytes());push[88..92].copy_from_slice(&1u32.to_ne_bytes());
    device.cmd_push_constants(command,world.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,0,&push);
    device.cmd_bind_vertex_buffers(command,0,&[world.resources.buffer(vertices).unwrap()],&[0]);device.cmd_bind_index_buffer(command,world.resources.buffer(indices).unwrap(),0,vk::IndexType::UINT32);
    for (sample,red) in values.iter().enumerate(){let color=[*red as f32/255.,0.,0.,1.];device.cmd_bind_pipeline(command,vk::PipelineBindPoint::GRAPHICS,pipelines[sample]);device.cmd_push_constants(command,world.layout,vk::ShaderStageFlags::VERTEX|vk::ShaderStageFlags::FRAGMENT,96,bytemuck::cast_slice(&color));device.cmd_draw_indexed(command,world.count,1,0,0,0);}
    device.cmd_end_render_pass(command);device.end_command_buffer(command).map_err(|e|e.to_string())?;
   }
   world.resources.mark_used(vertices,serial)?;world.resources.mark_used(indices,serial)?;
   unsafe{device.reset_fences(&[fence]).map_err(|e|e.to_string())?;device.queue_submit(context.graphics_queue,&[vk::SubmitInfo::default().command_buffers(&commands)],fence).map_err(|e|e.to_string())?;}
   context.wait_transfer(fence).map_err(|e|e.to_string())?;
   let rgba=world.readback(context)?;let first=&rgba[..4];if !rgba.chunks_exact(4).all(|p|p==first){return Err("Controlled sample color not uniform across target".into());}
   rows.push(serde_json::json!({"sample_red_bytes":values,"resolved_rgba":first}));
  }
  Ok(rows)
 })();
 // Only this isolated diagnostic device is drained before temporary handle destruction.
 if unsafe{device.device_wait_idle()}.is_ok(){unsafe{for p in pipelines{device.destroy_pipeline(p,None);}device.free_command_buffers(context.command_pool,&commands);device.destroy_fence(fence,None);}}
 let rows=result?;drop(world);
 let ledger=budget.ledger.lock().unwrap().total_committed();if ledger!=0{return Err(format!("GPU ledger leaked {ledger}"));}
 Ok(serde_json::json!({"scope":"Actual World MSAA4 resolve after controlled per-sample shader colors; no production policy modification", "cases":rows,"final_ledger":ledger}))
}
