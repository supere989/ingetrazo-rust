// Rust translation of libjpeg-turbo 3.2.0 jidctint.c (IJG ISLOW).
// Original IJG license in LICENSE-IJG; libjpeg-turbo BSD-3-Clause in LICENSE-TURBO.
// Eight-bit samples, dequantized input, CONST_BITS=13, PASS1_BITS=2.
fn kernel(s: [i64; 8]) -> [i64; 8] {
    let z1 = (s[2] + s[6]) * 4433;
    let t2 = z1 - s[6] * 15137;
    let t3 = z1 + s[2] * 6270;
    let t0 = (s[0] + s[4]) << 13;
    let t1 = (s[0] - s[4]) << 13;
    let e = [t0 + t3, t1 + t2, t1 - t2, t0 - t3];
    let z1 = (s[7] + s[1]) * -7373;
    let z2 = (s[5] + s[3]) * -20995;
    let z5 = (s[7] + s[3] + s[5] + s[1]) * 9633;
    let z3 = (s[7] + s[3]) * -16069 + z5;
    let z4 = (s[5] + s[1]) * -3196 + z5;
    let o = [
        s[7] * 2446 + z1 + z3,
        s[5] * 16819 + z2 + z4,
        s[3] * 25172 + z2 + z3,
        s[1] * 12299 + z1 + z4,
    ];
    [
        e[0] + o[3],
        e[1] + o[2],
        e[2] + o[1],
        e[3] + o[0],
        e[3] - o[0],
        e[2] - o[1],
        e[1] - o[2],
        e[0] - o[3],
    ]
}
pub fn idct_islow(input: &mut [i32; 64], out: &mut [i16], stride: usize) {
    let mut work = [0i64; 64];
    for x in 0..8 {
        let s = core::array::from_fn(|y| input[y * 8 + x] as i64);
        if s[1..].iter().all(|v| *v == 0) {
            for y in 0..8 {
                work[y * 8 + x] = s[0] << 2;
            }
        } else {
            let k = kernel(s);
            for y in 0..8 {
                work[y * 8 + x] = (k[y] + 1024) >> 11;
            }
        }
    }
    for y in 0..8 {
        let s = core::array::from_fn(|x| work[y * 8 + x]);
        let k = if s[1..].iter().all(|v| *v == 0) {
            [(s[0] + 16) >> 5; 8]
        } else {
            kernel(s).map(|v| (v + 131072) >> 18)
        };
        for x in 0..8 {
            if let Some(p) = out.get_mut(y * stride + x) {
                *p = (k[x] + 128).clamp(0, 255) as i16;
            }
        }
    }
}
