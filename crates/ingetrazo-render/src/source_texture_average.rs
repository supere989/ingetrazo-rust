// SPDX-License-Identifier: GPL-3.0-or-later
//! Cold 4×4 source image average used by the original Shaded pass.
//! Qt 6.12 qimage.cpp selects either premultiplied area scaling or the
//! high precision QPainter bilinear path; averaging decoded texels directly
//! is different, especially for alpha and nonuniform image dimensions.
//! No resized full image is allocated. Output is the sixteen straight pixels.

pub fn source_average_pixels(
    rgba: &[u8],
    width: u32,
    height: u32,
    alpha: bool,
) -> Result<[[u8; 4]; 16], String> {
    if width == 0 || height == 0 || (width as u64).checked_mul(height as u64).and_then(|n|n.checked_mul(4)) != Some(rgba.len() as u64) {
        return Err("Invalid average texture dimensions".into());
    }
    let high = alpha && width <= 8 && height <= 8 && (width as u64 * height as u64) < 1_000_000;
    let pixel = |x: usize, y: usize| {
        let p = &rgba[(y * width as usize + x) * 4..][..4];
        [
            p[0] as u64,
            p[1] as u64,
            p[2] as u64,
            if alpha { p[3] as u64 } else { 255 },
        ]
    };
    let mut out = [[0; 4]; 16];
    for y in 0..4 {
        for x in 0..4 {
            let p = if high {
                // QPainter smooth pixmap filtering uses premultiplied RGBA64. Vertical
                // and horizontal products truncate separately, then pixelColor unpremults.
                let axis = |d: usize, size: u32| {
                    // QSpanData::setupMatrix prepends a1/65536 translation before
                    // inversion. The fixed bilinear sample is one unit earlier.
                    // https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qpaintengine_raster.cpp
                    let pos = (2 * d + 1) as i64 * size as i64 * 8192 - 32768 - 1;
                    let i = pos >> 16;
                    (
                        i.max(0).min(size as i64 - 1) as usize,
                        (i + 1).max(0).min(size as i64 - 1) as usize,
                        (pos & 65535) as u64,
                    )
                };
                let (ix, jx, wx) = axis(x, width);
                let (iy, jy, wy) = axis(y, height);
                let premul = |p: [u64; 4]| {
                    let a = p[3] * 257;
                    let mut v = [0; 4];
                    v[3] = a;
                    for c in 0..3 {
                        let n = p[c] * 257 * a;
                        let hi = n >> 16;
                        v[c] = hi + (hi >> 15);
                    }
                    v
                };
                let tl = premul(pixel(ix, iy));
                let tr = premul(pixel(jx, iy));
                let bl = premul(pixel(ix, jy));
                let br = premul(pixel(jx, jy));
                let mix = |a: u64, b: u64, w: u64| {
                    if w == 0 {
                        a
                    } else {
                        ((a * (65536 - w)) >> 16) + ((b * w) >> 16)
                    }
                };
                let mut v = [0; 4];
                for c in 0..4 {
                    v[c] = mix(mix(tl[c], bl[c], wy), mix(tr[c], br[c], wy), wx);
                }
                let a = v[3];
                if a == 0 {
                    [0; 4]
                } else {
                    let rgb = std::array::from_fn(|c| {
                        if c == 3 {
                            ((a + 128 - ((a + 128) >> 8)) >> 8).min(255)
                        } else {
                            (v[c] as f32 * source_inverse_alpha(a))
                                .round_ties_even()
                                .min(255.) as u64
                        }
                    });
                    rgb
                }
            } else {
                // qimagescale.cpp's area weights use 14 fractional bits; upscale axes
                // use 8. Keep its intermediate truncation order rather than float weights.
                let premul = |p: [u64; 4]| {
                    let mut v = p;
                    for c in 0..3 {
                        let n = p[c] * p[3];
                        v[c] = (n + (n >> 8) + 128) >> 8;
                    }
                    v
                };
                let axis = |d: usize, size: u32| {
                    if size <= 4 {
                        let pos = (2 * d + 1) as i64 * size as i64 * 8192 - 32768;
                        let i = (pos >> 16).max(0) as usize;
                        let w = if pos < 0 || i >= size as usize - 1 {
                            0
                        } else {
                            ((pos & 65535) >> 8) as u64
                        };
                        (i.min(size as usize - 1), w, 0)
                    } else {
                        let pos = d as u64 * ((size as u64) << 16) / 4;
                        let cp = (((4u64) << 14) + size as u64 - 1) / size as u64;
                        (
                            (pos >> 16) as usize,
                            ((65536 - (pos & 65535)) * cp) >> 16,
                            cp,
                        )
                    }
                };
                let (ix, wx, cx) = axis(x, width);
                let (iy, wy, cy) = axis(y, height);

                let h = |yy: usize| {
                    if cx == 0 {
                        let a = premul(pixel(ix, yy));
                        let b = premul(pixel((ix + 1).min(width as usize - 1), yy));
                        std::array::from_fn(|c| (a[c] * (256 - wx) + b[c] * wx) >> 8)
                    } else {
                        sum_axis(ix, wx, cx, |xx| {
                            premul(pixel(xx.min(width as usize - 1), yy))
                        })
                    }
                };
                let mut v = if cy == 0 {
                    let a = h(iy);
                    let b = h((iy + 1).min(height as usize - 1));
                    std::array::from_fn(|c| (a[c] * (256 - wy) + b[c] * wy) >> 8)
                } else if cx == 0 {
                    {
                        let a = sum_axis(iy, wy, cy, |yy| {
                            premul(pixel(ix, yy.min(height as usize - 1)))
                        });
                        let b = sum_axis(iy, wy, cy, |yy| {
                            premul(pixel(
                                (ix + 1).min(width as usize - 1),
                                yy.min(height as usize - 1),
                            ))
                        });
                        std::array::from_fn(|c| ((a[c] * (256 - wx) + b[c] * wx) >> 8) >> 14)
                    }
                } else {
                    sum_axis(iy, wy, cy, |yy| {
                        h(yy.min(height as usize - 1)).map(|c| c >> 4)
                    })
                    .map(|c| c >> 24)
                };
                if cy == 0 && cx != 0 {
                    v = v.map(|c| c >> 14);
                }
                let a = v[3];
                if a == 0 {
                    [0; 4]
                } else {
                    if a != 255 {
                        for c in 0..3 {
                            v[c] = (v[c] as f32 * source_inverse_alpha(a)).round_ties_even() as u64;
                        }
                    }
                    v
                }
            };
            out[y * 4 + x] = p.map(|c| c as u8);
        }
    }
    Ok(out)
}

fn sum_axis(start: usize, w: u64, cp: u64, mut sample: impl FnMut(usize) -> [u64; 4]) -> [u64; 4] {
    let mut v = sample(start).map(|c| c * w);
    let mut remaining = 16384 - w;
    let mut i = start + 1;
    while remaining > cp {
        let p = sample(i);
        for c in 0..4 {
            v[c] += p[c] * cp;
        }
        remaining -= cp;
        i += 1;
    }
    let p = sample(i);
    for c in 0..4 {
        v[c] += p[c] * remaining;
    }
    v
}

#[cfg(target_arch = "x86_64")]
fn source_inverse_alpha(alpha: u64) -> f32 {
    // qdrawhelper_sse4.cpp reciprocal_mul_ps: Qt x86 conversion uses one
    // Newton refinement of RCPPS, not division; its ties affect output bytes.
    unsafe {
        use std::arch::x86_64::*;
        let a = _mm_set1_ps(alpha as f32);
        let i = _mm_rcp_ps(a);
        let i = _mm_cvtss_f32(i);
        let a = alpha as f32;
        (-i).mul_add(i * a, i + i) * 255.
    }
}
#[cfg(not(target_arch = "x86_64"))]
fn source_inverse_alpha(alpha: u64) -> f32 {
    255.0 / alpha as f32
}


#[cfg(test)]mod tests {
 use super::*;
 #[test]fn malformed_dimensions_are_refused_before_sampling(){assert!(source_average_pixels(&[],0,1,false).is_err());assert!(source_average_pixels(&[],u32::MAX,u32::MAX,false).is_err());assert!(source_average_pixels(&[0;3],1,1,false).is_err());}
 #[test]fn uniform_opaque_and_transparent_zero(){assert_eq!(source_average_pixels(&[37,93,157,255],1,1,false).unwrap(),[[37,93,157,255];16]);assert_eq!(source_average_pixels(&[37,93,157,0],1,1,true).unwrap(),[[0;4];16]);}
}
