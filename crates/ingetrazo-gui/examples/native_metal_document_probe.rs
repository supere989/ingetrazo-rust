//! Shared native host, actual own-window Cocoa events and document guard.
#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .iter()
        .find_map(|a| a.strip_prefix("--output="))
        .ok_or("--output=FRESH_DIR required")?;
    let choice = args
        .iter()
        .find_map(|a| a.strip_prefix("--choice="))
        .ok_or("--choice=save|discard|cancel required")?;
    ingetrazo_gui::native_metal::run_document_probe(out.into(), choice)
}
#[cfg(not(target_os = "macos"))]
fn main() {
    std::process::exit(2);
}
