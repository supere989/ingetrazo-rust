#[cfg(target_os = "macos")]
fn main() {
    let mut a = std::env::args().skip(1);
    let input = a.next().expect("Original input");
    let output = a.next().expect("Fresh output");
    if let Err(e) = ingetrazo_gui::native_metal::run_world_sweep_probe(input.into(), output.into())
    {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal requires macOS");
    std::process::exit(2);
}
