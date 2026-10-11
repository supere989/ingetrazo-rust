#[cfg(target_os = "macos")]
fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("Original input directory");
    let output = args.next().expect("Fresh output directory");
    if let Err(e) = ingetrazo_gui::native_metal::run_world_ghost_probe(input.into(), output.into())
    {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Native Metal GPU execution requires macOS");
    std::process::exit(2);
}
