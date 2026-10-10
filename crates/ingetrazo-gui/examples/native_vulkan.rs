#[cfg(any(target_os="linux",target_os="windows"))]
fn main()->Result<(),Box<dyn std::error::Error>> { ingetrazo_gui::native_vulkan::run_app::<ingetrazo_gui::IngeTrazoApp>(std::env::args_os().nth(1).map(Into::into)) }

#[cfg(not(any(target_os="linux",target_os="windows")))]
fn main(){ panic!("This proof host targets Linux/Windows; native Metal is a separate backend"); }
