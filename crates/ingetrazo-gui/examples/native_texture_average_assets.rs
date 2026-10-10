//! Actual imported assets, bounded decoder and source scale kernel; no GPU claim.
use image::ImageDecoder;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let scene = ingetrazo_formats::igz::load_igz(std::path::Path::new(&args[1]))?;
    let mut rows = Vec::new();
    let rawdir = args.get(3).map(std::path::PathBuf::from);
    if let Some(path) = &rawdir {
        std::fs::create_dir(path)?;
    }
    for (name, encoded) in &scene.embedded_textures {
        let format = image::guess_format(encoded)?;
        if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
            return Err("unsupported source texture decode format".into());
        }
        let decoder = image::ImageReader::with_format(std::io::Cursor::new(encoded), format)
            .into_decoder()?;
        let alpha = decoder.color_type().has_alpha();
        let (w, h) = decoder.dimensions();
        let bytes = (w as u64)
            .checked_mul(h as u64)
            .and_then(|n| n.checked_mul(4))
            .ok_or("dimensions overflow")?;
        if bytes > 256 * 1024 * 1024 {
            return Err("decoded admission".into());
        }
        drop(decoder);
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(encoded), format);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let rgba = if format == image::ImageFormat::Jpeg
            && std::env::var_os("INGETRAZO_SOURCE_JPEG").is_some()
        {
            ingetrazo_render::source_texture_decode::decode(encoded, Default::default())?.rgba
        } else {
            reader.decode()?.into_rgba8().into_raw()
        };
        let raw_name = format!("{}.rgba", blake3::hash(encoded).to_hex());
        if let Some(path) = &rawdir {
            std::fs::write(path.join(&raw_name), &rgba)?;
        }
        let pixels =
            ingetrazo_render::source_texture_average::source_average_pixels(&rgba, w, h, alpha)?;
        let average: [f64; 3] = std::array::from_fn(|c| {
            pixels.iter().map(|p| p[c] as u32).sum::<u32>() as f64 / (255. * 16.)
        });
        rows.push(serde_json::json!({"asset":name,"encoded_blake3":blake3::hash(encoded).to_hex().as_str(),"dimensions":[w,h],"source_alpha":alpha,"format":format!("{format:?}"),"raw_name":raw_name,"pixels":pixels,"average":average}));
    }
    rows.sort_by(|a, b| a["asset"].as_str().cmp(&b["asset"].as_str()));
    std::fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({"assets":rows}))?,
    )?;
    Ok(())
}
