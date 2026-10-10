#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Actual inline production precise CAD2D with UI primitives before and after.
use std::sync::Arc;
use ingetrazo_render::cad2d::{Geometry,Vertex,Batch,EncodedRgba,Pose};
use ingetrazo_gui::native_vulkan::{App,System,cad2d::Handle};
struct FailOnce(std::sync::atomic::AtomicBool);
impl egui_ash_renderer::VulkanPaint for FailOnce {
 fn as_any(&self)->&dyn std::any::Any{self}
 fn paint(&self,_:&egui::PaintCallbackInfo,_:&ash::Device,_:ash::vk::CommandBuffer)->egui_ash_renderer::RendererResult<()>{if !self.0.swap(true,std::sync::atomic::Ordering::Relaxed){Err(egui_ash_renderer::RendererError::Init("engineering callback failure after prior commands".into()))}else{Ok(())}}
}
struct Fixture{geometry:Arc<Geometry>,uniform_updated:bool,paint:Handle,texture:egui::TextureHandle,error:Option<Arc<egui_ash_renderer::VulkanPaintCallback>>}
impl App for Fixture {
 fn title()->&'static str{"IngeTrazo precise CAD2D fixture"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self{
  if std::env::var_os("INGETRAZO_CAD2D_PRESSURE").is_some(){system.verify_cad2d_rollback_for_probe().unwrap();}
  let geometry=Geometry::new_with_ordered_draws(vec![Vertex{position:[0.,0.],coverage:1.},Vertex{position:[80.,0.],coverage:1.},Vertex{position:[80.,80.],coverage:1.},Vertex{position:[0.,80.],coverage:1.}],vec![0,1,2,0,2,3],vec![0..6]).unwrap();
  let rgba=std::env::var("INGETRAZO_CAD2D_RGBA").unwrap_or("37,93,157,128".into()).split(',').map(|v|v.parse::<u8>().unwrap()).collect::<Vec<_>>();let rgba:[u8;4]=rgba.try_into().unwrap();
  let batches:Arc<[Batch]>=vec![Batch{first:0,count:6,color:EncodedRgba::from_bytes(rgba,1.).unwrap()}].into();
  let paint=system.enable_cad2d().unwrap().handle(geometry.clone(),batches,Pose{translation:[20.,20.],..Default::default()}).unwrap();
  let texture=system.egui_ctx.load_texture("callback-restoration",egui::ColorImage{size:[2,2],pixels:vec![egui::Color32::RED,egui::Color32::GREEN,egui::Color32::BLUE,egui::Color32::WHITE]},egui::TextureOptions::NEAREST);Self{geometry,uniform_updated:false,paint,texture,error:std::env::var_os("INGETRAZO_CAD2D_CALLBACK_ERROR").map(|_|Arc::new(egui_ash_renderer::VulkanPaintCallback(Arc::new(FailOnce(Default::default())))))}
 }
 fn build_ui(&mut self,ctx:&egui::Context){
  if std::env::var_os("INGETRAZO_CAD2D_UNIFORM_UPDATE").is_some()&&!self.uniform_updated{
   if ctx.input(|i|i.time)>0.5{let batches:Arc<[Batch]>=vec![Batch{first:0,count:6,color:EncodedRgba::from_bytes([255,0,0,255],1.).unwrap()}].into();self.paint.update(self.geometry.clone(),batches,Pose{translation:[60.,20.],..Default::default()}).unwrap();self.uniform_updated=true;}else{ctx.request_repaint_after(std::time::Duration::from_millis(100));}
  }
  if std::env::var_os("INGETRAZO_CAD2D_CALLBACK_ONLY").is_some(){ctx.layer_painter(egui::LayerId::background()).add(self.paint.shape(egui::Rect::from_min_size(egui::pos2(20.,20.),egui::vec2(80.,80.))));}
  else {egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::WHITE)).show(ctx,|ui|{
  let painter=ui.painter();painter.rect_filled(egui::Rect::from_min_size(egui::pos2(10.,10.),egui::vec2(100.,100.)),0.,egui::Color32::WHITE);
  painter.add(self.paint.shape(egui::Rect::from_min_size(egui::pos2(20.,20.),egui::vec2(80.,80.))));
  painter.image(self.texture.id(),egui::Rect::from_min_size(egui::pos2(120.,20.),egui::vec2(80.,80.)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::WHITE);
  painter.rect_filled(egui::Rect::from_min_size(egui::pos2(30.,30.),egui::vec2(10.,10.)),0.,egui::Color32::BLACK);
 });}
 if let Some(error)=&self.error{ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground,egui::Id::new("injectedfailure"))).add(egui::PaintCallback{rect:ctx.screen_rect(),callback:error.clone()});}
 if ctx.input(|i|i.viewport().close_requested()){ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
 fn gpu_error(&mut self,error:&str){if self.error.is_some()||std::env::var_os("INGETRAZO_DISABLE_PRESENT_FENCES").is_some(){eprintln!("EXPECTED callback error: {error}");}else{panic!("Precise CAD2D failed: {error}");}}
}

// Alternate direct native host: exercise actual System Drop without on_exit.
#[derive(Default)]struct OwnerProbe{window:Option<winit::window::Window>,system:Option<System>,fixture:Option<Fixture>,budget:Option<Arc<ingetrazo_gui::native_vulkan::budget::VulkanBudget>>}
impl winit::application::ApplicationHandler for OwnerProbe{
 fn resumed(&mut self,event_loop:&winit::event_loop::ActiveEventLoop){
  let window=event_loop.create_window(winit::window::Window::default_attributes().with_title(Fixture::title()).with_inner_size(winit::dpi::PhysicalSize::new(800,600))).unwrap();
  let mut system=System::new(Fixture::title(),&window).unwrap();let fixture=Fixture::new(&mut system,None);self.budget=Some(system.memory_budget.clone());self.fixture=Some(fixture);self.system=Some(system);self.window=Some(window);self.window.as_ref().unwrap().request_redraw();
 }
 fn window_event(&mut self,event_loop:&winit::event_loop::ActiveEventLoop,_:winit::window::WindowId,event:winit::event::WindowEvent){
  if self.system.is_none(){return;}
  self.system.as_mut().unwrap().on_window_event(&event,self.window.as_ref().unwrap(),event_loop);
  match event{
   winit::event::WindowEvent::CloseRequested=>{let system=self.system.as_mut().unwrap();
    let device=system.vulkan_context.device.clone();let queue=system.vulkan_context.graphics_queue;
    let pool=system.vulkan_context.command_pool;let lifetime=system.vulkan_context.lifetime.clone();
    let hold_before_present=std::env::var_os("INGETRAZO_WSI_HOLD_BEFORE_PRESENT").is_some();
    let frames_before=system.submitted_frames_for_probe();
    if !hold_before_present{system.draw(self.fixture.as_mut().unwrap(),self.window.as_ref().unwrap());}
    let descriptor_owner=system.cad2d.as_ref().unwrap().descriptor_owner_weak_for_probe();
    let descriptor_before=descriptor_owner.state().expect("Actual integer CAD descriptor owner missing before fault");
    assert!(descriptor_before.complete_integer(),"Integer descriptor pipeline not active; enable INGETRAZO_CAD2D_INTEGER_BLEND=1");
    let before=self.budget.as_ref().unwrap().ledger.lock().unwrap().total_committed();
    let event=unsafe{device.create_event(&ash::vk::EventCreateInfo::default(),None).unwrap()};
    unsafe{device.reset_event(event).unwrap();}
    let command=unsafe{device.allocate_command_buffers(&ash::vk::CommandBufferAllocateInfo::default().command_pool(pool).level(ash::vk::CommandBufferLevel::PRIMARY).command_buffer_count(1)).unwrap()[0]};
    let held=unsafe{device.create_fence(&ash::vk::FenceCreateInfo::default(),None).unwrap()};
    unsafe{device.begin_command_buffer(command,&ash::vk::CommandBufferBeginInfo::default()).unwrap();device.cmd_wait_events(command,&[event],ash::vk::PipelineStageFlags::HOST,ash::vk::PipelineStageFlags::TRANSFER,&[],&[],&[]);device.end_command_buffer(command).unwrap();device.queue_submit(queue,&[ash::vk::SubmitInfo::default().command_buffers(&[command])],held).unwrap();}
    eprintln!("Actual host-event queue hold submitted; hold_before_present={hold_before_present}");
    if hold_before_present{system.draw(self.fixture.as_mut().unwrap(),self.window.as_ref().unwrap());}
    let present_fences=system.pending_present_fences_for_probe();
    assert_eq!(system.submitted_frames_for_probe(),frames_before+1,"Production frame was not submitted behind the hold");
    assert_eq!(unsafe{device.get_fence_status(held)},Ok(false),"Held GPU work must actually be incomplete");
    let pending_present_count=present_fences.iter().filter(|f|unsafe{device.get_fence_status(**f)}==Ok(false)).count();
    eprintln!("Actual queued WSI frame: incomplete event fence=true unsignaled_present_fences={pending_present_count}; present fences may signal after payload reference transfer before display completion");
    let start=std::time::Instant::now();assert!(!system.bounded_completion_for_probe());
    let mut refused=ingetrazo_gui::native_vulkan::resources::Resources::new(&system.vulkan_context,self.budget.as_ref().unwrap().clone());
    assert!(refused.upload(&system.vulkan_context,&[1u8;64],ash::vk::BufferUsageFlags::VERTEX_BUFFER,true).is_err(),"Poisoned native allocation admission accepted");drop(refused);
    drop(self.fixture.take()); // Drop final external Handle/TextureHandle; no Factory is retained.
    assert_eq!(descriptor_owner.strong_count(),1,"External CAD pipeline owner survived fixture drop");
    drop(self.system.take());
    let descriptor_after_drop=descriptor_owner.state();
    let descriptor_retained=descriptor_after_drop.as_ref()==Some(&descriptor_before);
    let budget=self.budget.as_ref().unwrap();let committed=budget.ledger.lock().unwrap().total_committed();
    assert!(lifetime.uncertain());assert!(lifetime.check().is_err());assert_eq!(committed,before,"Pending WSI Drop freed charged resources");assert!(lifetime.retained_owners()>=3);
    assert!(start.elapsed().as_secs_f64()>=4.9&&start.elapsed().as_secs_f64()<7.,"Bounded present fence timeout outside protocol");
    let elapsed=start.elapsed().as_millis();let weak=std::sync::Arc::downgrade(budget);drop(self.budget.take());assert_eq!(weak.upgrade().unwrap().ledger.lock().unwrap().total_committed(),committed);
    unsafe{device.set_event(event).unwrap();device.wait_for_fences(&[held],true,5_000_000_000).unwrap();device.wait_for_fences(&present_fences,true,5_000_000_000).unwrap();let done=device.create_fence(&ash::vk::FenceCreateInfo::default(),None).unwrap();device.queue_submit(queue,&[],done).unwrap();device.wait_for_fences(&[done],true,5_000_000_000).unwrap();device.destroy_fence(done,None);device.destroy_fence(held,None);device.free_command_buffers(pool,&[command]);device.destroy_event(event,None);}
    assert!(lifetime.uncertain());assert_eq!(weak.upgrade().unwrap().ledger.lock().unwrap().total_committed(),committed);
    assert!(descriptor_retained,"Quarantine lost final integer descriptor owner after dropping Fixture and System");
    assert_eq!(descriptor_owner.state(),Some(descriptor_before.clone()),"Real GPU completion silently freed or changed quarantined integer descriptor metadata");
    assert!(descriptor_owner.state().unwrap().complete_integer());
    println!("PASS actual integer descriptor ownership: weak_upgrades=true external_handles_dropped=true descriptor_sets={} metadata_identical=true real_completion_still_quarantined=true",descriptor_before.descriptor_set_count());
    println!("PASS actual pending WSI timeout: elapsed_ms={elapsed} retained_bytes={committed} tracked_present_fences={} unsignaled_present_fences_before_drop={pending_present_count} retained_owner_records={} new_admission_refused=true last_external_ledger_drop_retained=true subsequent_real_graphics_and_present_completion=true automatic_recovery=false",present_fences.len(),lifetime.retained_owners());
    event_loop.exit();},
   winit::event::WindowEvent::Resized(_)=>self.window.as_ref().unwrap().request_redraw(),
   winit::event::WindowEvent::RedrawRequested=>{self.system.as_mut().unwrap().draw(self.fixture.as_mut().unwrap(),self.window.as_ref().unwrap());self.window.as_ref().unwrap().request_redraw();},_=>{}
  }
 }
}
pub fn main()->Result<(),Box<dyn std::error::Error>>{let event_loop=winit::event_loop::EventLoop::new()?;event_loop.run_app(&mut OwnerProbe::default())?;Ok(())}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
