#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::annotation_overlay_probe::run()}
#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){eprintln!("Native Vulkan diagnostic is Linux/Windows only; macOS uses Metal.");std::process::exit(2);}
