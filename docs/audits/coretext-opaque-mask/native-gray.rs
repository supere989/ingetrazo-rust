fn native_bgrx_coverage(p: &[u8]) -> u8 {
    ((11 * (255 - u32::from(p[2])) + 16 * (255 - u32::from(p[1]))
        + 5 * (255 - u32::from(p[0]))) / 32) as u8
}

fn main(){use std::io::Write;let mut out=std::io::stdout().lock();for r in 0..=255u8{for g in 0..=255u8{for b in [0,1,127,254,255]{for x in [0,255]{out.write_all(&[native_bgrx_coverage(&[b,g,r,x])]).unwrap();}}}}}
