//! Source-exact BC7 mode 5 admission, never a lossy image conversion.
//! Khronos Data Format BPTC: 7-bit RGB endpoints, independent 8-bit scalar
//! endpoints, two 2-bit index streams (texel-zero anchors), optional rotation.
//! This bounded encoder tries component bounds and four rotations;
//! unrepresentable blocks retain RGBA. No fitting tolerance is involved.
use crate::texture_layout::Format;
const WEIGHTS: [u16; 4] = [0, 21, 43, 64];
fn interpolate(a: u8, b: u8, index: usize) -> u8 {
    (((64 - WEIGHTS[index]) * a as u16 + WEIGHTS[index] * b as u16 + 32) >> 6) as u8
}
fn expand(q: u8) -> u8 {
    (q << 1) | (q >> 6)
}
/// None means no byte savings, unavailable allocation, or inexact encoding.
/// Bad dimensions/length are errors, distinct from ordinary RGBA fallback.
pub fn exact_bc7(width: u32, height: u32, rgba: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let raw = Format::Rgba8.mip_bytes(width, height)?;
    if raw != rgba.len() as u64 {
        return Err("BC7 input differs from RGBA dimensions".into());
    }
    let packed = Format::Bc7.mip_bytes(width, height)?;
    if packed >= raw {
        return Ok(None);
    }
    let size = usize::try_from(packed).map_err(|_| "BC7 allocation extent overflow")?;
    let mut output = Vec::new();
    if output.try_reserve_exact(size).is_err() {
        return Ok(None);
    }
    for by in 0..height.div_ceil(4) {
        for bx in 0..width.div_ceil(4) {
            let mut pixels = [[0; 4]; 16];
            for (i, pixel) in pixels.iter_mut().enumerate() {
                let x = (bx * 4 + i as u32 % 4).min(width - 1);
                let y = (by * 4 + i as u32 / 4).min(height - 1);
                let offset = (y as usize * width as usize + x as usize) * 4;
                pixel.copy_from_slice(&rgba[offset..offset + 4]);
            }
            let Some(block) = encode_block(&pixels) else {
                return Ok(None);
            };
            // Verify actual packed representation and anchors with one-block scratch.
            if decode_mode5(&block) != Some(pixels) {
                return Ok(None);
            }
            output.extend_from_slice(&block);
        }
    }
    Ok(Some(output))
}
fn encode_block(pixels: &[[u8; 4]; 16]) -> Option<[u8; 16]> {
    for rotation in 0..4 {
        let mut rotated = *pixels;
        if rotation != 0 {
            for p in &mut rotated {
                p.swap(3, rotation - 1);
            }
        }
        let mut lo = [255; 4];
        let mut hi = [0; 4];
        for p in rotated {
            for c in 0..4 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
        }
        if (0..3).any(|c| expand(lo[c] >> 1) != lo[c] || expand(hi[c] >> 1) != hi[c]) {
            continue;
        }
        let palette: [[u8; 4]; 4] =
            std::array::from_fn(|i| std::array::from_fn(|c| interpolate(lo[c], hi[c], i)));
        let mut color = [0u8; 16];
        let mut alpha = [0u8; 16];
        let mut exact = true;
        for (i, p) in rotated.iter().enumerate() {
            match (
                palette.iter().position(|q| q[..3] == p[..3]),
                palette.iter().position(|q| q[3] == p[3]),
            ) {
                (Some(c), Some(a)) => {
                    color[i] = c as u8;
                    alpha[i] = a as u8;
                }
                _ => {
                    exact = false;
                    break;
                }
            }
        }
        if !exact {
            continue;
        }
        if color[0] >= 2 {
            for c in 0..3 {
                std::mem::swap(&mut lo[c], &mut hi[c]);
            }
            for i in &mut color {
                *i = 3 - *i;
            }
        }
        if alpha[0] >= 2 {
            std::mem::swap(&mut lo[3], &mut hi[3]);
            for i in &mut alpha {
                *i = 3 - *i;
            }
        }
        let mut bits = 0u128;
        let mut offset = 0;
        let mut push = |value: u8, count: u32| {
            bits |= (value as u128) << offset;
            offset += count;
        };
        push(32, 6);
        push(rotation as u8, 2);
        for c in 0..3 {
            push(lo[c] >> 1, 7);
            push(hi[c] >> 1, 7);
        }
        push(lo[3], 8);
        push(hi[3], 8);
        for indices in [color, alpha] {
            for (i, index) in indices.iter().enumerate() {
                push(*index, if i == 0 { 1 } else { 2 });
            }
        }
        debug_assert_eq!(offset, 128);
        return Some(bits.to_le_bytes());
    }
    None
}
/// This encoder's mode only; other BC7 modes are refused. Native GPU decoding
/// is independently checked by the Vulkan acceptance fixture.
pub fn decode_mode5(block: &[u8; 16]) -> Option<[[u8; 4]; 16]> {
    let bits = u128::from_le_bytes(*block);
    if bits & 63 != 32 {
        return None;
    }
    let rotation = ((bits >> 6) & 3) as usize;
    let mut offset = 8;
    let mut read = |n: u32| {
        let value = ((bits >> offset) & ((1u128 << n) - 1)) as u8;
        offset += n;
        value
    };
    let mut endpoints = [[0u8; 4]; 2];
    for c in 0..3 {
        for e in &mut endpoints {
            e[c] = expand(read(7));
        }
    }
    for e in &mut endpoints {
        e[3] = read(8);
    }
    let mut indices = [[0usize; 16]; 2];
    for stream in &mut indices {
        for (i, index) in stream.iter_mut().enumerate() {
            *index = read(if i == 0 { 1 } else { 2 }) as usize;
        }
    }
    Some(std::array::from_fn(|i| {
        let mut pixel = std::array::from_fn(|c| {
            interpolate(
                endpoints[0][c],
                endpoints[1][c],
                indices[usize::from(c == 3)][i],
            )
        });
        if rotation != 0 {
            pixel.swap(3, rotation - 1);
        }
        pixel
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_anchor_swaps_independent_alpha_and_all_rotations() {
        for rotation in 0..4 {
            for shift in 0..16 {
                let pixels = std::array::from_fn(|i| {
                    let mut p = [
                        interpolate(0, 255, (i + shift) % 4),
                        interpolate(0, 255, (i + shift) % 4),
                        interpolate(0, 255, (i + shift) % 4),
                        interpolate(23, 234, (i / 4 + shift) % 4),
                    ];
                    if rotation != 0 {
                        p.swap(3, rotation - 1);
                    }
                    p
                });
                let block = encode_block(&pixels).expect("Representable palette");
                assert_eq!(decode_mode5(&block), Some(pixels));
                assert_eq!((block[0] >> 6) as usize, rotation);
            }
        }
    }
    #[test]
    fn non_multiple_extent_padding_and_exact_fallback() {
        let rgba: Vec<u8> = (0..35)
            .flat_map(|i| [255, 255, 255, if i % 3 == 0 { 255 } else { 0 }])
            .collect();
        let encoded = exact_bc7(7, 5, &rgba).unwrap().unwrap();
        assert_eq!(encoded.len(), 64);
        for y in 0..5usize {
            for x in 0..7usize {
                let offset = ((y / 4) * 2 + x / 4) * 16;
                let block = decode_mode5(encoded[offset..offset + 16].try_into().unwrap()).unwrap();
                assert_eq!(
                    block[(y % 4) * 4 + x % 4],
                    rgba[(y * 7 + x) * 4..(y * 7 + x + 1) * 4]
                );
            }
        }
        assert!(exact_bc7(1, 1, &[255; 4]).unwrap().is_none());
        assert!(exact_bc7(4, 4, &[1; 64]).unwrap().is_none());
        assert!(exact_bc7(0, 4, &[]).is_err());
        assert!(exact_bc7(u32::MAX, u32::MAX, &[]).is_err());
        assert!(exact_bc7(4, 4, &[0; 63]).is_err());
    }
    #[test]
    fn late_inexact_block_cannot_publish_partial_image() {
        let mut rgba = vec![255; 8 * 4 * 4];
        rgba[7 * 4..7 * 4 + 4].copy_from_slice(&[1, 3, 5, 7]);
        assert!(exact_bc7(8, 4, &rgba).unwrap().is_none());
        assert!(decode_mode5(&[0; 16]).is_none());
    }
}
