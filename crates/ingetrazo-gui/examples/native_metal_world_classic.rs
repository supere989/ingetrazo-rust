#[cfg(target_os = "macos")]
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("Output required"));
    if let Err(e) = ingetrazo_gui::native_metal::run_world_classic_probe(root) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal application consumer requires macOS");
    std::process::exit(2);
}
