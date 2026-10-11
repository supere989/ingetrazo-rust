#[cfg(not(target_os="macos"))]
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::face_sweep_application_probe::run()}
#[cfg(target_os="macos")]
fn main(){eprintln!("Direct Vulkan sweep application diagnostic is Linux/Windows only.");std::process::exit(2);}
