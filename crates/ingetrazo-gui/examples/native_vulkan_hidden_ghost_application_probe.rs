#[cfg(not(target_os="macos"))]
fn main()->Result<(),Box<dyn std::error::Error>>{ingetrazo_gui::native_vulkan::hidden_ghost_application_probe::run()}
#[cfg(target_os="macos")]
fn main(){eprintln!("Direct Vulkan ghost application diagnostic is Linux/Windows only.");std::process::exit(2);}
