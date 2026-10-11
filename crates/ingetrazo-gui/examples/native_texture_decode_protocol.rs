#[cfg(any(target_os="linux", target_os="windows"))]
mod platform {
//! Production cold decoder protocol against original Qt literal files.
use ingetrazo_gui::native_vulkan::texture_cache::decode_png_jpeg;
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("INPUT OUTPUT required".into());
    }
    let input = std::path::Path::new(&args[0]);
    let output = std::path::Path::new(&args[1]);
    if input.is_file() {
        let encoded=std::fs::read(input)?;
        let decoded=decode_png_jpeg(&encoded,256*1024*1024)?;
        std::fs::write(output,&decoded.rgba)?;
        println!("{} {} {}",decoded.extent.width,decoded.extent.height,decoded.rgba.len());
        return Ok(());
    }
    std::fs::create_dir_all(output)?;
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(input.join("original.json"))?)?;
    let cases = original["cases"]
        .as_object()
        .ok_or("Missing source cases")?;
    let mut result = serde_json::Map::new();
    for name in cases.keys() {
        let bytes = std::fs::read(input.join(format!("{name}.png")))?;
        let decoded = decode_png_jpeg(&bytes, 64 * 1024 * 1024)?;
        std::fs::write(output.join(format!("{name}.rgba")), &decoded.rgba)?;
        result.insert(name.clone(),serde_json::json!({"width":decoded.extent.width,"height":decoded.extent.height,"cutout":decoded.hard_cutout}));
    }
    std::fs::write(
        output.join("native.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}

}

#[cfg(any(target_os="linux", target_os="windows"))]
fn main() -> impl std::process::Termination { platform::main() }

#[cfg(not(any(target_os="linux", target_os="windows")))]
fn main() {
    eprintln!("This diagnostic requires native Vulkan on Linux or Windows; macOS uses native Metal.");
    std::process::exit(2);
}
