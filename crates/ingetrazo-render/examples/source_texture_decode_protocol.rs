fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let encoded = std::fs::read(&args[1])?;
    let limits = ingetrazo_render::source_texture_decode::Limits {
        decoded_work_bytes: args
            .get(3)
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(256 * 1024 * 1024),
        ..Default::default()
    };
    let decoded = ingetrazo_render::source_texture_decode::decode(&encoded, limits)?;
    std::fs::write(&args[2], &decoded.rgba)?;
    println!(
        "{} {} {} {}",
        decoded.width,
        decoded.height,
        decoded.alpha,
        decoded.rgba.len()
    );
    Ok(())
}
