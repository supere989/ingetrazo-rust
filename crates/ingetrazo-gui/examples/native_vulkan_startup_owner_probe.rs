#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Real startup allocations cancelled before any submission, no fake Vulkan result.
use winit::{application::ApplicationHandler,event_loop::{ActiveEventLoop,EventLoop},window::{Window,WindowId},event::WindowEvent};
use ingetrazo_gui::native_vulkan::{System,resize::Stage};
struct Probe;
impl ApplicationHandler for Probe{
 fn resumed(&mut self,event:&ActiveEventLoop){
  let window=event.create_window(Window::default_attributes().with_title("IngeTrazo startup owner probe").with_inner_size(winit::dpi::PhysicalSize::new(800,600))).unwrap();
  if std::env::var_os("INGETRAZO_STARTUP_EXPECT_REFUSAL").is_some(){let audit=System::verify_startup_refusal_for_probe("IngeTrazo startup owner probe",&window).unwrap();println!("STARTUP_ACTUAL_ADAPTER_REFUSAL_PASS audit={}",serde_json::to_string(&audit).unwrap());event.exit();return;}
  for stage in [Stage::StartupInstance,Stage::StartupSurface,Stage::StartupDevice,Stage::StartupPool,Stage::StartupCommand,Stage::StartupSwapchain,Stage::StartupSync,Stage::StartupRenderer]{
   let audit=System::verify_startup_cancellation_for_probe("IngeTrazo startup owner probe",&window,stage).unwrap();
   println!("STARTUP_CANCEL {stage:?} audit={}",serde_json::to_string(&audit).unwrap());
  }
  println!("STARTUP_ALL_8_PASS no_queue_submission=true");event.exit();
 }
 fn window_event(&mut self,_:&ActiveEventLoop,_:WindowId,_:WindowEvent){}
}
pub fn main()->Result<(),Box<dyn std::error::Error>>{let events=EventLoop::new()?;events.run_app(&mut Probe)?;Ok(())}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
