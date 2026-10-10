// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal persistent render process. No window, device or GUI runtime initialization.
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 || args[1] != "--hlr-worker" {
        eprintln!("Usage: ingetrazo-render-worker --hlr-worker SHARED_RESULT_FILE");
        std::process::exit(2);
    }
    if let Err(error) = ingetrazo_render::process_hlr::worker_main(std::path::Path::new(&args[2])) {
        eprintln!("Render worker: {error}");
        std::process::exit(1);
    }
}
