//! Bounded cold native decoding for the pinned source's texture-average path.
//! Existing image-crate callers retain their decoder policy.
use image::ImageDecoder;
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub alpha: bool,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub encoded_bytes: usize,
    pub decoded_work_bytes: u64,
    pub max_jpeg_scans: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            encoded_bytes: 16 * 1024 * 1024,
            decoded_work_bytes: 256 * 1024 * 1024,
            max_jpeg_scans: 64,
        }
    }
}
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Decoded, String> {
    if bytes.is_empty() || bytes.len() > limits.encoded_bytes {
        return Err("source image encoded admission".into());
    }
    let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
    if format == image::ImageFormat::WebP {
        return decode_webp(bytes, limits);
    }
    if !matches!(format, image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::Bmp | image::ImageFormat::Tiff) {
        return Err("source image format unsupported".into());
    }
    if format == image::ImageFormat::Jpeg {
        if limits.decoded_work_bytes < 1024 * 1024 {
            return Err("source JPEG header work admission".into());
        }
        let options = zune_core::options::DecoderOptions::default()
            .set_max_width(u16::MAX as usize)
            .set_max_height(u16::MAX as usize)
            .set_strict_mode(true)
            .jpeg_set_max_scans(limits.max_jpeg_scans)
            .jpeg_set_out_colorspace(zune_core::colorspace::ColorSpace::RGBA);
        let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
            zune_core::bytestream::ZCursor::new(bytes),
            options,
        );
        decoder.use_source_islow();
        decoder.decode_headers().map_err(|e| e.to_string())?;
        let info = decoder.info().ok_or("source JPEG missing dimensions")?;
        let (w, h) = (info.width as u32, info.height as u32);
        // Covers RGBA, four progressive coefficient planes and per-row/upsample scratch,
        // with MCU padding and metadata reserve; admission precedes decoded allocation.
        let padded = (w as u64 + 31)
            .checked_mul(h as u64 + 31)
            .ok_or("source image dimensions overflow")?;
        let work = padded
            .checked_mul(64)
            .and_then(|n| n.checked_add(1024 * 1024))
            .ok_or("source image work overflow")?;
        if w == 0 || h == 0 || work > limits.decoded_work_bytes {
            return Err("source JPEG decoded work admission".into());
        }
        let rgba = decoder.decode().map_err(|e| e.to_string())?;
        if rgba.len() != w as usize * h as usize * 4 {
            return Err("source JPEG invalid output size".into());
        }
        return Ok(Decoded {
            width: w,
            height: h,
            alpha: false,
            rgba,
        });
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut header_limits = image::Limits::default();
    header_limits.max_alloc = Some(limits.decoded_work_bytes);
    reader.limits(header_limits);
    let decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    let alpha = decoder.color_type().has_alpha();
    let output = (w as u64)
        .checked_mul(h as u64)
        .and_then(|n| n.checked_mul(16))
        .ok_or("source bitmap work overflow")?;
    if output > limits.decoded_work_bytes || w == 0 || h == 0 {
        return Err("source bitmap decoded work admission".into());
    }
    let rgba = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| e.to_string())?
        .into_rgba8()
        .into_raw();
    Ok(Decoded {
        width: w,
        height: h,
        alpha,
        rgba,
    })
}
/// Static WebP shares the pinned Qt codec's straight RGBA decode. The supplied
/// buffer is the final output owner; no intermediate image or conversion copy.
fn decode_webp(bytes: &[u8], limits: Limits) -> Result<Decoded, String> {
    use libwebp_sys as ffi;
    let mut features = std::mem::MaybeUninit::<ffi::WebPBitstreamFeatures>::uninit();
    let status = unsafe {
        ffi::WebPGetFeaturesInternal(
            bytes.as_ptr(),
            bytes.len(),
            features.as_mut_ptr(),
            ffi::WEBP_DECODER_ABI_VERSION as i32,
        )
    };
    if status != ffi::VP8StatusCode::VP8_STATUS_OK {
        return Err(format!("source WebP header: {status:?}"));
    }
    let features = unsafe { features.assume_init() };
    if features.has_animation != 0 {
        return Err("source animated WebP composition is not implemented".into());
    }
    if features.width <= 0 || features.height <= 0 {
        return Err("source WebP invalid dimensions".into());
    }
    let (width, height) = (features.width as u32, features.height as u32);
    // Conservatively admit output plus the codec's lossless/lossy scratch domain
    // before either is allocated. Callers control this budget independently of
    // the GPU allocation; this is not a claim about maximum device capacity.
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or("source WebP dimensions overflow")?;
    let work = pixels
        .checked_mul(64)
        .and_then(|n| n.checked_add(1024 * 1024))
        .ok_or("source WebP work overflow")?;
    if work > limits.decoded_work_bytes {
        return Err("source WebP decoded work admission".into());
    }
    let len = pixels
        .checked_mul(4)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("source WebP output overflow")?;
    let stride = width
        .checked_mul(4)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or("source WebP row overflow")?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(len)
        .map_err(|_| "source WebP output allocation")?;
    rgba.resize(len, 0);
    let decoded = unsafe {
        ffi::WebPDecodeRGBAInto(bytes.as_ptr(), bytes.len(), rgba.as_mut_ptr(), len, stride)
    };
    if decoded.is_null() {
        return Err("source WebP pixel decoding failed".into());
    }
    Ok(Decoded {
        width,
        height,
        alpha: features.has_alpha != 0,
        rgba,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_empty_and_encoded_limit_before_decoder() {
        assert!(decode(&[], Limits::default()).is_err());
        assert!(decode(
            &[255, 216, 255],
            Limits {
                encoded_bytes: 2,
                ..Limits::default()
            }
        )
        .is_err());
    }
    #[test]
    fn refuses_corrupt_payload() {
        for bytes in [
            &[255, 216, 255, 217][..],
            &[137, 80, 78, 71, 13, 10, 26, 10][..],
        ] {
            assert!(decode(bytes, Limits::default()).is_err());
        }
    }
}
