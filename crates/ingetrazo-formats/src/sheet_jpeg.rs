//! Qt-compatible baseline libjpeg scanlines; bounded pixels and writer errors.
use super::sheet_image::Metrics;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    io::Write,
};
// Retain the pinned native dependency's link metadata alongside the C adapter.
use turbojpeg_sys as _;
unsafe extern "C" {
    fn ingetrazo_sheet_jpeg_new(
        width: u32,
        height: u32,
        density_unit: u32,
        density_x: u32,
        density_y: u32,
        quality: u32,
        write: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> c_int,
        opaque: *mut c_void,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut c_void;
    fn ingetrazo_sheet_jpeg_row(
        state: *mut c_void,
        pixels: *const u8,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn ingetrazo_sheet_jpeg_finish(
        state: *mut c_void,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn ingetrazo_sheet_jpeg_abort(state: *mut c_void);
}
struct Output<'a, W> {
    writer: &'a mut W,
    error: Option<String>,
}
unsafe extern "C" fn output<W: Write>(opaque: *mut c_void, bytes: *const u8, len: usize) -> c_int {
    let state = unsafe { &mut *opaque.cast::<Output<'_, W>>() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state
            .writer
            .write_all(unsafe { std::slice::from_raw_parts(bytes, len) })
    }));
    match result {
        Ok(Ok(())) => 1,
        Ok(Err(error)) => {
            state.error = Some(error.to_string());
            0
        }
        Err(_) => {
            state.error = Some("JPEG output callback panicked".into());
            0
        }
    }
}
struct Compressor(*mut c_void);
impl Drop for Compressor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ingetrazo_sheet_jpeg_abort(self.0) }
        }
    }
}
fn message(error: &[c_char]) -> String {
    unsafe { CStr::from_ptr(error.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}
pub(crate) fn encode<W: Write>(
    writer: &mut W,
    metrics: Metrics,
    mut provider: impl FnMut(u32, u32) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    // Reference one linked native API so Rust carries its static library metadata.
    let _ = turbojpeg_sys::TJINIT_TJINIT_COMPRESS;
    let [width, height] = metrics.pixels();
    if width > 65535 || height > 65535 {
        return Err("JPEG dimensions exceed 65535".into());
    }
    let dpm = metrics.dots_per_meter();
    let inch = f64::from(dpm) * 2.54 / 100.;
    let cm = f64::from(dpm) / 100.;
    let round = |v: f64| (v + 0.5).floor();
    let (unit, density) = if (inch - round(inch)).abs() < (cm - round(cm)).abs() * 2.54 {
        (1, round(inch) as u32)
    } else {
        (2, (dpm + 50) / 100)
    };
    encode_pixels(writer, width, height, unit, density, 92, true, provider)
}
/// Original QPdfEngine's explicit quality94 RGB image stream. Alpha is emitted
/// separately as the PDF mask; the JPEG writer receives straight source RGB.
pub(crate) fn encode_picture<W: Write>(
    writer: &mut W,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .filter(|n| *n <= 256 * 1024 * 1024)
        .ok_or("PDF picture JPEG byte budget exceeded")?;
    if width == 0 || height == 0 || width > 65535 || height > 65535 || bytes != rgba.len() as u64 {
        return Err("PDF picture JPEG inventory invalid".into());
    }
    encode_pixels(writer, width, height, 1, 96, 94, false, |y, rows| {
        let a = y as usize * width as usize * 4;
        let b = a + rows as usize * width as usize * 4;
        let mut out = Vec::new();
        out.try_reserve_exact(b - a)
            .map_err(|_| "PDF picture row allocation failed")?;
        out.extend_from_slice(&rgba[a..b]);
        Ok(out)
    })
}
fn encode_pixels<W: Write>(
    writer: &mut W,
    width: u32,
    height: u32,
    unit: u32,
    density: u32,
    quality: u32,
    opaque: bool,
    mut provider: impl FnMut(u32, u32) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    if !matches!(quality, 92 | 94) {
        return Err("Unsupported native JPEG quality".into());
    }
    let mut output_state = Output {
        writer,
        error: None,
    };
    let mut error = [0 as c_char; 256];
    let raw = unsafe {
        ingetrazo_sheet_jpeg_new(
            width,
            height,
            unit,
            density,
            density,
            quality,
            output::<W>,
            (&mut output_state as *mut Output<'_, W>).cast(),
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if raw.is_null() {
        return Err(output_state.error.unwrap_or_else(|| message(&error)));
    }
    let mut compressor = Compressor(raw);
    let mut rgb = Vec::new();
    rgb.try_reserve_exact(width as usize * 3)
        .map_err(|_| "JPEG row allocation failed")?;
    rgb.resize(width as usize * 3, 0);
    for y in (0..height).step_by(8) {
        let rows = (height - y).min(8);
        let rgba = provider(y, rows)?;
        if rgba.len() != width as usize * rows as usize * 4
            || (opaque && rgba.chunks_exact(4).any(|p| p[3] != 255))
        {
            return Err("JPEG requires exact opaque native rows".into());
        }
        for row in rgba.chunks_exact(width as usize * 4) {
            for (p, out) in row.chunks_exact(4).zip(rgb.chunks_exact_mut(3)) {
                out.copy_from_slice(&p[..3]);
            }
            if unsafe {
                ingetrazo_sheet_jpeg_row(
                    compressor.0,
                    rgb.as_ptr(),
                    error.as_mut_ptr(),
                    error.len(),
                )
            } != 1
            {
                return Err(output_state.error.take().unwrap_or_else(|| message(&error)));
            }
        }
    }
    let raw = std::mem::replace(&mut compressor.0, std::ptr::null_mut());
    if unsafe { ingetrazo_sheet_jpeg_finish(raw, error.as_mut_ptr(), error.len()) } != 1 {
        return Err(output_state.error.take().unwrap_or_else(|| message(&error)));
    }
    if let Some(error) = output_state.error {
        return Err(error);
    }
    Ok(())
}
