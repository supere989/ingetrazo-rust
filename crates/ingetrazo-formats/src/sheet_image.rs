// SPDX-License-Identifier: GPL-3.0-or-later
//! Physical paper image encoding. The native renderer supplies opaque RGBA8;
//! PNG streams one RGB row, JPEG borrows pixels without a second full-page copy.
use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Format {
    Png,
    Jpeg,
}
impl Format {
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".png") {
            Some(Self::Png)
        } else if name.ends_with(".jpg") || name.ends_with(".jpeg") {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
}
/// An existing recognized extension wins; otherwise append the chosen filter.
pub fn resolve_path(path: &Path, selected: Format) -> (PathBuf, Format) {
    if let Some(format) = Format::from_path(path) {
        return (path.into(), format);
    }
    let mut name = path.as_os_str().to_owned();
    name.push(match selected {
        Format::Png => ".png",
        Format::Jpeg => ".jpg",
    });
    (PathBuf::from(name), selected)
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Metrics {
    pixels: [u32; 2],
    dots_per_meter: u32,
    dpi: u16,
}
impl Metrics {
    pub fn pixels(self) -> [u32; 2] {
        self.pixels
    }
    pub fn dots_per_meter(self) -> u32 {
        self.dots_per_meter
    }
    pub fn dpi(self) -> u16 {
        self.dpi
    }
    /// The source export dialog's 50..1200 DPI domain; nearest-even dimensions.
    /// Backend tile/device limits do not change the physical output dimensions.
    pub fn new(page_mm: [f64; 2], dpi: u16) -> Result<Self, String> {
        if !(50..=1200).contains(&dpi) || page_mm.iter().any(|x| !x.is_finite() || *x <= 0.) {
            return Err("Invalid paper dimensions or export resolution".into());
        }
        let scale = f64::from(dpi) / 25.4;
        let dimensions = page_mm.map(|v| (v * scale).round_ties_even().max(1.));
        if dimensions
            .iter()
            .any(|v| !v.is_finite() || *v > f64::from(u32::MAX))
        {
            return Err("Paper image exceeds file dimension range".into());
        }
        let result = Self {
            pixels: dimensions.map(|v| v as u32),
            dots_per_meter: (f64::from(dpi) / 0.0254).round_ties_even() as u32,
            dpi,
        };
        result.rgba_bytes()?;
        Ok(result)
    }
    pub fn rgba_bytes(self) -> Result<usize, String> {
        (self.pixels[0] as usize)
            .checked_mul(self.pixels[1] as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| "Paper image exceeds addressable memory".into())
    }
}
fn encode(
    writer: &mut impl Write,
    metrics: Metrics,
    rgba: &[u8],
    format: Format,
) -> Result<(), String> {
    let [width, height] = metrics.pixels;
    match format {
        Format::Png => {
            let mut encoder = png::Encoder::new(writer, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_pixel_dims(Some(png::PixelDimensions {
                xppu: metrics.dots_per_meter,
                yppu: metrics.dots_per_meter,
                unit: png::Unit::Meter,
            }));
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            let mut row = vec![0u8; width as usize * 3];
            {
                let mut stream = writer.stream_writer().map_err(|e| e.to_string())?;
                for input in rgba.chunks_exact(width as usize * 4) {
                    for (source, target) in input.chunks_exact(4).zip(row.chunks_exact_mut(3)) {
                        target.copy_from_slice(&source[..3]);
                    }
                    stream.write_all(&row).map_err(|e| e.to_string())?;
                }
                stream.finish().map_err(|e| e.to_string())?;
            }
            writer.finish().map_err(|e| e.to_string())
        }
        Format::Jpeg => crate::sheet_jpeg::encode(writer, metrics, |y, rows| {
            let first = y as usize * width as usize * 4;
            let length = rows as usize * width as usize * 4;
            Ok(rgba[first..first + length].to_vec())
        }),
    }
}
/// Encode into a same-directory temporary file, then atomically publish.
/// Invalid input or an encoding failure preserves an existing destination.
/// White paper must already be composited by the native renderer; alpha loss is refused.
pub fn save_rgba(path: &Path, metrics: Metrics, rgba: &[u8]) -> Result<(), String> {
    let format = Format::from_path(path).ok_or("Paper image requires PNG or JPEG extension")?;
    if metrics.pixels.contains(&0)
        || rgba.len() != metrics.rgba_bytes()?
        || rgba.chunks_exact(4).any(|p| p[3] != 255)
    {
        return Err("Paper image requires exact opaque RGBA8 extent".into());
    }
    if format == Format::Jpeg && metrics.pixels.iter().any(|&v| v > u32::from(u16::MAX)) {
        return Err("JPEG file dimensions exceed 65535 pixels".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = parent.join(format!(".ingetrazo-image-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);
        encode(&mut writer, metrics, rgba, format)?;
        writer.flush().map_err(|e| e.to_string())?;
        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        drop(writer);
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

/// A bounded native readback provider. Each request covers complete consecutive
/// rows; JPEG requests eight-row MCU bands, PNG requests one row. Returning an
/// error aborts the temporary file, including errors encountered lazily by JPEG.
pub fn save_rows(
    path: &Path,
    metrics: Metrics,
    mut provider: impl FnMut(u32, u32) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let format = Format::from_path(path).ok_or("Paper image requires PNG or JPEG extension")?;
    let [width, height] = metrics.pixels;
    if width == 0 || height == 0 {
        return Err("Empty paper image".into());
    }
    if format == Format::Jpeg && metrics.pixels.iter().any(|v| *v > 65535) {
        return Err("JPEG dimensions exceed 65535".into());
    }
    let row_bytes = (width as usize)
        .checked_mul(4)
        .ok_or("Paper row size overflow")?;
    if row_bytes
        .checked_mul(8)
        .is_none_or(|n| n > 32 * 1024 * 1024)
    {
        return Err("Paper band exceeds 32 MiB".into());
    }
    let check = |pixels: &[u8], rows: u32| {
        if pixels.len() == row_bytes * rows as usize && pixels.chunks_exact(4).all(|p| p[3] == 255)
        {
            Ok(())
        } else {
            Err("Paper band requires exact opaque RGBA8 extent".to_owned())
        }
    };
    let first_rows = if format == Format::Jpeg {
        height.min(8)
    } else {
        1
    };
    let mut first = Some(provider(0, first_rows)?);
    check(first.as_ref().unwrap(), first_rows)?;
    let mut rows = |y: u32, count: u32| {
        let pixels = if y == 0 {
            first
                .take()
                .ok_or("Paper first band already consumed".to_owned())?
        } else {
            provider(y, count)?
        };
        check(&pixels, count)?;
        Ok(pixels)
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temp = parent.join(format!(".ingetrazo-image-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let mut writer = BufWriter::new(file);
        match format {
            Format::Jpeg => crate::sheet_jpeg::encode(&mut writer, metrics, &mut rows)?,
            Format::Png => {
                let mut encoder = png::Encoder::new(&mut writer, width, height);
                encoder.set_color(png::ColorType::Rgb);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_pixel_dims(Some(png::PixelDimensions {
                    xppu: metrics.dots_per_meter,
                    yppu: metrics.dots_per_meter,
                    unit: png::Unit::Meter,
                }));
                let mut png = encoder.write_header().map_err(|e| e.to_string())?;
                let mut rgb = Vec::new();
                rgb.try_reserve_exact(width as usize * 3)
                    .map_err(|_| "PNG row allocation failed")?;
                rgb.resize(width as usize * 3, 0);
                {
                    let mut stream = png.stream_writer().map_err(|e| e.to_string())?;
                    for y in 0..height {
                        let input = rows(y, 1)?;
                        for (p, out) in input.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
                            out.copy_from_slice(&p[..3]);
                        }
                        stream.write_all(&rgb).map_err(|e| e.to_string())?;
                    }
                    stream.finish().map_err(|e| e.to_string())?;
                }
                png.finish().map_err(|e| e.to_string())?;
            }
        }
        writer.flush().map_err(|e| e.to_string())?;
        writer.get_ref().sync_all().map_err(|e| e.to_string())?;
        drop(writer);
        std::fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
