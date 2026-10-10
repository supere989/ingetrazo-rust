#[cfg(any(target_os="linux",target_os="windows"))]
mod native {
// Real renderer/budget ownership probe, after a normally presented baseline frame.
use ingetrazo_gui::native_vulkan::{App,System,VulkanContext};
struct Probe{phase:u8,handle:Option<egui::TextureHandle>,id:Option<egui::TextureId>,serial:u64,bytes:u64}
impl App for Probe{
 fn title()->&'static str{"IngeTrazo managed raw GPU lifetime probe"}
 fn new(_: &mut System,_:Option<std::path::PathBuf>)->Self{Self{phase:0,handle:None,id:None,serial:0,bytes:0}}
 fn build_ui(&mut self,ctx:&egui::Context){egui::CentralPanel::default().show(ctx,|ui|{ui.label("Managed texture lifetime admission probe");let(rect,_)=ui.allocate_exact_size(egui::Vec2::splat(64.),egui::Sense::hover());if let Some(handle)=self.handle.take(){ui.painter().image(handle.id(),rect,egui::Rect::from_min_max(egui::Pos2::ZERO,egui::Pos2::new(1.,1.)),egui::Color32::WHITE);drop(handle);}else{ui.painter().rect_filled(rect,0.,egui::Color32::RED);}});ctx.request_repaint();if self.phase==2{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
 fn prepare_gpu(&mut self,system:&mut System,before:bool){
  if !before{return;}
  if self.phase==0&&system.submitted_frames_for_probe()>=2{
   system.verify_raw_texture_lifetime_for_probe().expect("Actual raw GPU lifetime acceptance");
   self.serial=system.submitted_frames_for_probe();self.bytes=system.memory_budget.ledger.lock().unwrap().total_committed();
   let handle=system.create_raw_texture_for_probe().unwrap();self.id=Some(handle.id());self.handle=Some(handle);self.phase=1;
  }else if self.phase==1{
   assert!(system.submitted_frames_for_probe()>self.serial,"Set/use/free frame must actually present");
   assert!(!system.managed_texture_for_probe(self.id.unwrap()),"Same-frame texture must retire after presented frame completion");
   let ledger=system.memory_budget.ledger.lock().unwrap();assert_eq!(ledger.total_committed(),self.bytes);assert!(ledger.heaps.iter().all(|h|h.reserved==0));
   eprintln!("PASS RAW_LIFETIME same-frame raw set / actual mesh paint+present / free after completion; presented={} baseline_bytes={}",system.submitted_frames_for_probe(),self.bytes);self.phase=2;
  }
 }
 fn gpu_error(&mut self,error:&str){panic!("Unexpected probe frame failure: {error}");}
 fn clean(&mut self,_:&VulkanContext){}
}
pub fn run()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::run_app::<Probe>(None)}

}
#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{native::run()}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Actual Vulkan managed lifetime probe requires Linux/Windows native Vulkan backend");}
