#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Actual Vk image reservation followed by staging denial; no user paths/clipboard.
use ingetrazo_gui::native_vulkan::{App,System,VulkanContext};
use ash::vk;
struct Probe {app:ingetrazo_gui::IngeTrazoApp}
impl App for Probe {
 fn title()->&'static str {"IngeTrazo Vulkan allocation probe"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self {
  let device=&system.vulkan_context.device;
  let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).extent(vk::Extent3D{width:2,height:2,depth:1}).mip_levels(1).array_layers(1).format(vk::Format::R8G8B8A8_SRGB).tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST|vk::ImageUsageFlags::SAMPLED).samples(vk::SampleCountFlags::TYPE_1);
  let image=unsafe {device.create_image(&info,None)}.unwrap();let requirements=unsafe {device.get_image_memory_requirements(image)};unsafe {device.destroy_image(image,None)};
  let properties=unsafe {system.vulkan_context.instance.get_physical_device_memory_properties(system.vulkan_context.physical_device)};
  let ty=(0..properties.memory_type_count).find(|&i|requirements.memory_type_bits&(1<<i)!=0&&properties.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)).unwrap();
  let heap=properties.memory_types[ty as usize].heap_index as usize;
  // Production startup may already own admitted attachments (for example the
  // annotation stencil). Deny only the upload's staging allocation, preserving
  // every existing owner's exact per-heap baseline.
  let (limits,baseline)={let mut ledger=system.memory_budget.ledger.lock().unwrap();assert!(ledger.heaps.iter().all(|h|h.reserved==0));let limits=ledger.heaps.iter().map(|h|h.limit).collect::<Vec<_>>();let baseline=ledger.heaps.iter().map(|h|h.committed).collect::<Vec<_>>();for h in &mut ledger.heaps {h.limit=h.committed;}ledger.heaps[heap].limit=baseline[heap].checked_add(requirements.size).unwrap();(limits,baseline)};
  let delta=egui::epaint::ImageDelta::full(egui::ColorImage::new([2,2],egui::Color32::RED),egui::TextureOptions::NEAREST);
  let result=system.renderer.set_textures(system.vulkan_context.graphics_queue,system.vulkan_context.command_pool,&[(egui::TextureId::Managed(777),delta)]);
  assert!(result.is_err(),"Expected staging allocation denial after admitted image");
  {let mut ledger=system.memory_budget.ledger.lock().unwrap();assert!(ledger.peak_committed>=baseline.iter().sum::<u64>()+requirements.size,"Image was never admitted before refusal");assert_eq!(ledger.heaps.iter().map(|h|h.committed).collect::<Vec<_>>(),baseline,"Image leaked or existing ownership changed after staging denial");assert!(ledger.heaps.iter().all(|h|h.reserved==0),"Reservation leaked");for (h,limit) in ledger.heaps.iter_mut().zip(limits){h.limit=limit;}}
  eprintln!("PASS actual image allocation ({}) + staging admission denial: exact startup ownership preserved, no upload memory or reservation leaks",requirements.size);
  Self{app:ingetrazo_gui::IngeTrazoApp::new()}
 }
 fn build_ui(&mut self,ctx:&egui::Context){self.app.update_ui(ctx);}
 fn clean(&mut self,_:&VulkanContext){}
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
