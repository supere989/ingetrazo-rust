//! Actual App frame anchors and model annotation observations, no GPU pixel claim.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: frame_annotation_sync SOURCE OUTPUT".into());
    }
    ingetrazo_gui::frame_annotation_sync_consumer::run(
        std::path::Path::new(&args[0]),
        std::path::Path::new(&args[1]),
    )
}
