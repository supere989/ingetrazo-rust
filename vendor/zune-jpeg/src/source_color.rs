// IJG jdcolor.c 16-bit tables, RGB/RGBA output. Source opt-in only.
pub fn rgba(y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], pos: &mut usize) {
    for i in 0..16 {
        let a = cb[i] as i32 - 128;
        let b = cr[i] as i32 - 128;
        let l = y[i] as i32;
        let r = l + ((91881 * b + 32768) >> 16);
        let g = l + ((-22554 * a - 46802 * b + 32768) >> 16);
        let blue = l + ((116130 * a + 32768) >> 16);
        out[*pos..*pos + 4].copy_from_slice(&[
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            blue.clamp(0, 255) as u8,
            255,
        ]);
        *pos += 4;
    }
}
pub fn horizontal(input: &[i16], _: &[i16], _: &[i16], _: &mut [i16], out: &mut [i16]) {
    for x in 0..input.len() {
        let c = input[x] as i32;
        out[2 * x] = if x == 0 {
            c as i16
        } else {
            ((3 * c + input[x - 1] as i32 + 1) >> 2) as i16
        };
        out[2 * x + 1] = if x + 1 == input.len() {
            c as i16
        } else {
            ((3 * c + input[x + 1] as i32 + 2) >> 2) as i16
        };
    }
}
pub fn vertical(input: &[i16], above: &[i16], below: &[i16], _: &mut [i16], out: &mut [i16]) {
    let n = input.len();
    for x in 0..n {
        out[x] = ((3 * input[x] as i32 + above[x] as i32 + 1) >> 2) as i16;
        out[n + x] = ((3 * input[x] as i32 + below[x] as i32 + 2) >> 2) as i16;
    }
}
pub fn hv(input: &[i16], above: &[i16], below: &[i16], _: &mut [i16], out: &mut [i16]) {
    let n = input.len();
    for (row, far) in [above, below].iter().enumerate() {
        for x in 0..n {
            let c = 3 * input[x] as i32 + far[x] as i32;
            let left = if x == 0 {
                c
            } else {
                3 * input[x - 1] as i32 + far[x - 1] as i32
            };
            let right = if x + 1 == n {
                c
            } else {
                3 * input[x + 1] as i32 + far[x + 1] as i32
            };
            out[row * 2 * n + 2 * x] = ((3 * c + left + 8) >> 4) as i16;
            out[row * 2 * n + 2 * x + 1] = ((3 * c + right + 7) >> 4) as i16;
        }
    }
}

pub fn horizontal_nearest(input: &[i16], _: &[i16], _: &[i16], _: &mut [i16], out: &mut [i16]) {
    for (sample, pair) in input.iter().zip(out.chunks_exact_mut(2)) {
        pair.fill(*sample);
    }
}
pub fn hv_nearest(input: &[i16], _: &[i16], _: &[i16], _: &mut [i16], out: &mut [i16]) {
    for row in out.chunks_exact_mut(input.len() * 2) {
        horizontal_nearest(input, &[], &[], &mut [], row);
    }
}
