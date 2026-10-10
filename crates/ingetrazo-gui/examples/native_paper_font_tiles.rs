// SPDX-License-Identifier: GPL-3.0-or-later
#[cfg(target_os = "macos")]
fn main() {
    let root = std::env::args().nth(1).expect("empty output directory");
    if let Err(error) =
        ingetrazo_gui::composer::probe_native_font_tiles(std::path::Path::new(&root))
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Actual native CoreText Canvas observation requires macOS");
    std::process::exit(2);
}
