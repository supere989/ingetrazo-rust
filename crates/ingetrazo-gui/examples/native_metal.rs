//! Opt-in direct shared native Metal application host; no backend wrapper.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    ingetrazo_gui::native_metal::run(std::env::args_os().nth(1).map(Into::into))
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This diagnostic requires native Metal on macOS; Linux and Windows use native Vulkan.");
    std::process::exit(2);
}
