#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Actual bounded transfer + in-flight memory retirement probe on Mesa unified heaps.
use ingetrazo_gui::native_vulkan::{App,System};
use ash::vk;
struct Probe {app:ingetrazo_gui::IngeTrazoApp}
impl App for Probe {
 fn title()->&'static str {"IngeTrazo Vulkan buffer probe"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self {
  let mut resources=ingetrazo_gui::native_vulkan::resources::Resources::new(&system.vulkan_context,system.memory_budget.clone());
  let data=(0..17*1024*1024).map(|i|(i%251) as u8).collect::<Vec<_>>();
  let id=resources.upload(&system.vulkan_context,&data,vk::BufferUsageFlags::VERTEX_BUFFER,true).unwrap();
  resources.verify_host_visible(id,&data).unwrap();assert_eq!(resources.stats.bytes,data.len() as u64);assert_eq!(resources.stats.chunks,3);assert!(resources.stats.staging_peak<=8*1024*1024);
  let charged=system.memory_budget.ledger.lock().unwrap().total_committed();assert!(charged>=data.len() as u64);
  resources.mark_used(id,9).unwrap();resources.retire(id).unwrap();
  ingetrazo_gui::native_vulkan::world::verify_image_view_failure_rollback(&system.vulkan_context,system.memory_budget.clone()).unwrap();
  eprintln!("PASS actual image allocation/binding followed by injected view failure rollback");
  let device=&system.vulkan_context.device;let fence=unsafe {device.create_fence(&vk::FenceCreateInfo::default(),None)}.unwrap();
  assert_eq!(resources.collect(fence,9).unwrap(),0);assert_eq!(system.memory_budget.ledger.lock().unwrap().total_committed(),charged);
  unsafe {device.queue_submit(system.vulkan_context.graphics_queue,&[vk::SubmitInfo::default()],fence).unwrap();device.wait_for_fences(&[fence],true,u64::MAX).unwrap();}
  assert_eq!(resources.collect(fence,9).unwrap(),1);assert_eq!(system.memory_budget.ledger.lock().unwrap().total_committed(),0);
  unsafe {device.destroy_fence(fence,None)};
  eprintln!("PASS actual 17MiB GPU upload: everybyteverified, 3chunks, stagingpeak {}, retirementchargeduntilfence",resources.stats.staging_peak);
  Self{app:ingetrazo_gui::IngeTrazoApp::new()}
 }
 fn build_ui(&mut self,ctx:&egui::Context){self.app.update_ui(ctx);}
}
pub fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Probe>(None)}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
