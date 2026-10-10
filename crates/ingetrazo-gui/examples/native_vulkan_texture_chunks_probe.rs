#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Production texture uploader: image fits budget, full-image staging does not.
use ingetrazo_gui::native_vulkan::{App,System,VulkanContext};
use ash::vk;
struct Probe;
impl App for Probe {
 fn title()->&'static str {"IngeTrazo bounded texture probe"}
 fn new(system:&mut System,_:Option<std::path::PathBuf>)->Self {
  let device=&system.vulkan_context.device;
  let info=vk::ImageCreateInfo::default().image_type(vk::ImageType::TYPE_2D).extent(vk::Extent3D{width:4096,height:3072,depth:1}).mip_levels(1).array_layers(1).format(vk::Format::R8G8B8A8_SRGB).tiling(vk::ImageTiling::OPTIMAL).usage(vk::ImageUsageFlags::TRANSFER_DST|vk::ImageUsageFlags::SAMPLED).samples(vk::SampleCountFlags::TYPE_1);
  let image=unsafe {device.create_image(&info,None)}.unwrap();let requirements=unsafe {device.get_image_memory_requirements(image)};unsafe {device.destroy_image(image,None)};
  let properties=unsafe {system.vulkan_context.instance.get_physical_device_memory_properties(system.vulkan_context.physical_device)};
  let ty=(0..properties.memory_type_count).find(|&i|requirements.memory_type_bits&(1<<i)!=0&&properties.memory_types[i as usize].property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)).unwrap();
  let heap=properties.memory_types[ty as usize].heap_index as usize;
  let limits={let mut ledger=system.memory_budget.ledger.lock().unwrap();let limits=ledger.heaps.iter().map(|h|h.limit).collect::<Vec<_>>();for h in &mut ledger.heaps {h.limit=0;}ledger.heaps[heap].limit=requirements.size+8*1024*1024;limits};
  let mut pixels=egui::ColorImage::new([4096,3072],egui::Color32::RED);
  for y in 1024..3072 {for x in 0..4096 {pixels[(x,y)]=if y<2048 {egui::Color32::GREEN}else{egui::Color32::BLUE};}}
  let delta=egui::epaint::ImageDelta::full(pixels,egui::TextureOptions::NEAREST);
  system.renderer.set_textures(system.vulkan_context.graphics_queue,system.vulkan_context.command_pool,&[(egui::TextureId::Managed(777),delta)]).expect("48MiB image with 8MiB staging headroom must upload");
  let partial=egui::epaint::ImageDelta::partial([3072,2048],egui::ColorImage::new([1024,1024],egui::Color32::YELLOW),egui::TextureOptions::NEAREST);
  system.renderer.set_textures(system.vulkan_context.graphics_queue,system.vulkan_context.command_pool,&[(egui::TextureId::Managed(777),partial)]).unwrap();
  {let mut ledger=system.memory_budget.ledger.lock().unwrap();assert_eq!(ledger.total_committed(),requirements.size,"Staging must be freed after completed transfer");assert!(ledger.heaps.iter().all(|h|h.reserved==0));for (h,limit) in ledger.heaps.iter_mut().zip(limits){h.limit=limit;}}
  eprintln!("PASS actual 48MiB texture with only 8MiB staging headroom, partial lower-right update, no retained staging charge");
  Self
 }
 fn build_ui(&mut self,ctx:&egui::Context){egui::CentralPanel::default().show(ctx,|ui|{ui.painter().image(egui::TextureId::Managed(777),egui::Rect::from_min_size(egui::pos2(20.,20.),egui::vec2(384.,288.)),egui::Rect::from_min_max(egui::Pos2::ZERO,egui::pos2(1.,1.)),egui::Color32::WHITE);});}
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
