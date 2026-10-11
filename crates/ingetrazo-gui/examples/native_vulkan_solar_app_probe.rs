// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{
 use ingetrazo_gui::{IngeTrazoApp,native_vulkan::{App,System,VulkanContext}};
 struct Probe{app:IngeTrazoApp,phase:u8,baseline:u64,initial:Option<(u64,u64,bool)>,start:std::time::Instant,error:bool}
 impl App for Probe{
  fn title()->&'static str{"IngeTrazo solar application probe"}
  fn new(system:&mut System,path:Option<std::path::PathBuf>)->Self{
   let mut app=<IngeTrazoApp as App>::new(system,path);app.shadows_enabled=true;
   Self{app,phase:0,baseline:0,initial:None,start:std::time::Instant::now(),error:false}
  }
  fn build_ui(&mut self,ctx:&egui::Context){
   <IngeTrazoApp as App>::build_ui(&mut self.app,ctx);ctx.request_repaint();
   if self.phase==3||self.error||self.start.elapsed().as_secs()>=15{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
  }
  fn prepare_gpu(&mut self,system:&mut System,resize:bool){
   <IngeTrazoApp as App>::prepare_gpu(&mut self.app,system,resize);
   if resize{return;}
   let Some(stats)=system.solar_stats_for_probe()else{return;};let serial=system.submitted_frames_for_probe();
   if stats.0==0{return;}
   if self.initial.is_none(){assert!(stats.2);self.initial=Some(stats);self.baseline=serial;return;}
   if serial<=self.baseline+8{return;}
   let first=self.initial.unwrap();
   match self.phase{
    0=>{assert_eq!(stats,first,"warm application rebuilt shadows");self.app.shadows_enabled=false;self.phase=1;},
    1=>{assert!(!stats.2);assert_eq!(stats.0,first.0);self.app.shadows_enabled=true;self.phase=2;},
    2=>{assert_eq!(stats,first,"restored application shadow map changed");self.phase=3;eprintln!("SOLAR_APPLICATION_PASS submitted={} renders={} uniform_uploads={}",serial,stats.0,stats.1);},_=>{}
   }
   self.baseline=serial;
  }
  fn gpu_error(&mut self,error:&str){self.error=true;<IngeTrazoApp as App>::gpu_error(&mut self.app,error);}
  fn clean(&mut self,context:&VulkanContext){assert!(!self.error&&self.phase==3,"actual solar app did not complete");<IngeTrazoApp as App>::clean(&mut self.app,context);}
 }
 let path=std::env::args().nth(1).ok_or("INPUT.igz required")?;
 ingetrazo_gui::native_vulkan::run_app::<Probe>(Some(path.into()))
}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Native Vulkan diagnostic is Linux/Windows only");}
