//! Native Qt-compatible orthomosaic storage. No intermediate image file or process.
//! Qt 6.12 QWebpHandler uses ARGB import, quality 92 and alpha_quality 92.
use libwebp_sys as ffi;
use std::{ffi::c_void, mem::MaybeUninit};

struct Picture(ffi::WebPPicture);
impl Drop for Picture {
    fn drop(&mut self) {
        unsafe { ffi::WebPPictureFree(&mut self.0) }
    }
}
struct Output {
    bytes: Vec<u8>,
    limit: usize,
}
// The encoder invokes this synchronously while both Picture and Output live.
// Allocation refusal is returned to C; no Rust panic may cross the callback.
unsafe extern "C" fn write_bytes(
    data: *const u8,
    len: usize,
    picture: *const ffi::WebPPicture,
) -> i32 {
    if picture.is_null() || (data.is_null() && len != 0) {
        return 0;
    }
    let ptr = unsafe { (*picture).custom_ptr.cast::<Output>() };
    if ptr.is_null() {
        return 0;
    }
    let output = unsafe { &mut *ptr };
    let Some(total) = output.bytes.len().checked_add(len) else {
        return 0;
    };
    if total > output.limit || output.bytes.try_reserve(len).is_err() {
        return 0;
    }
    if len != 0 {
        output
            .bytes
            .extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
    }
    1
}
/// Encode the already reduced RGBA orthomosaic using the original Qt settings.
/// Input is borrowed; the writer fills the final Vec directly. libwebp owns
/// only its required picture/encoder work, which is released on every return.
pub fn encode_rgba92(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    const MAX_REDUCED_SIDE: u32 = 8192;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("WebP dimensions overflow")?;
    if width == 0
        || height == 0
        || width > MAX_REDUCED_SIDE
        || height > MAX_REDUCED_SIDE
        || rgba.len() != expected
    {
        return Err("WebP requires a reduced RGBA image with dimensions at most 8192".into());
    }
    let mut picture = MaybeUninit::<ffi::WebPPicture>::uninit();
    let mut config = MaybeUninit::<ffi::WebPConfig>::uninit();
    unsafe {
        if ffi::WebPPictureInitInternal(picture.as_mut_ptr(), ffi::WEBP_ENCODER_ABI_VERSION as i32)
            == 0
        {
            return Err("WebP picture ABI initialization failed".into());
        }
    }
    let mut picture = Picture(unsafe { picture.assume_init() });
    unsafe {
        if ffi::WebPConfigInitInternal(
            config.as_mut_ptr(),
            ffi::WebPPreset::WEBP_PRESET_DEFAULT,
            75.,
            ffi::WEBP_ENCODER_ABI_VERSION as i32,
        ) == 0
        {
            return Err("WebP configuration ABI initialization failed".into());
        }
    }
    let mut config = unsafe { config.assume_init() };
    config.lossless = 0;
    config.quality = 92.;
    config.alpha_quality = 92;
    picture.0.width = width as i32;
    picture.0.height = height as i32;
    picture.0.use_argb = 1;
    if unsafe { ffi::WebPPictureImportRGBA(&mut picture.0, rgba.as_ptr(), (width * 4) as i32) } == 0
    {
        return Err("WebP picture allocation failed".into());
    }
    let mut output = Output {
        bytes: Vec::new(),
        limit: expected
            .checked_add(65536)
            .ok_or("WebP output budget overflow")?,
    };
    picture.0.writer = Some(write_bytes);
    picture.0.custom_ptr = (&mut output as *mut Output).cast::<c_void>();
    if unsafe { ffi::WebPEncode(&config, &mut picture.0) } == 0 {
        return Err(format!("WebP encoding failed: {:?}", picture.0.error_code));
    }
    Ok(output.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_shape_before_codec_allocation() {
        assert!(encode_rgba92(&[], 0, 1).is_err());
        assert!(encode_rgba92(&[0; 4], 2, 1).is_err());
        assert!(encode_rgba92(&[], u32::MAX, u32::MAX).is_err());
    }
    #[test]
    fn callback_refuses_over_budget_without_partial_publication() {
        let mut p = MaybeUninit::<ffi::WebPPicture>::uninit();
        assert_ne!(
            unsafe {
                ffi::WebPPictureInitInternal(p.as_mut_ptr(), ffi::WEBP_ENCODER_ABI_VERSION as i32)
            },
            0
        );
        let mut p = Picture(unsafe { p.assume_init() });
        let mut out = Output {
            bytes: vec![1],
            limit: 2,
        };
        p.0.custom_ptr = (&mut out as *mut Output).cast();
        assert_eq!(unsafe { write_bytes([2, 3].as_ptr(), 2, &p.0) }, 0);
        assert_eq!(out.bytes, [1]);
    }
}
