//! Original viewport._fb_size uses Python round (ties to even), independently
//! of Qt widget backing-store rounding and the logical camera aspect.
pub(crate) fn scene_extent(logical: [f64; 2], density: f64) -> Result<[u32; 2], String> {
    if !density.is_finite() || density <= 0. {
        return Err("Viewport density outside admission".into());
    }
    let mut result = [0; 2];
    for (out, value) in result.iter_mut().zip(logical) {
        let physical = value * density;
        if !value.is_finite() || value <= 0. || !physical.is_finite() || physical > u32::MAX as f64 {
            return Err("Viewport extent outside admission".into());
        }
        *out = physical.round_ties_even().max(1.) as u32;
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_fractional_dpr_scene_extents() {
        // Actual pinned Qt observations: scene target may differ from grabFramebuffer.
        for (size, dpr, expected) in [
            ([942.,610.],1.,[942,610]), ([943.,611.],1.,[943,611]), ([801.,601.],1.,[801,601]),
            ([942.,610.],1.25,[1178,762]), ([943.,611.],1.25,[1179,764]), ([801.,601.],1.25,[1001,751]),
            ([942.,610.],1.5,[1413,915]), ([943.,611.],1.5,[1414,916]), ([801.,601.],1.5,[1202,902]),
        ] { assert_eq!(scene_extent(size,dpr).unwrap(),expected); }
        assert!(scene_extent([f64::INFINITY,1.],1.).is_err());
        assert!(scene_extent([1.,1.],f64::NAN).is_err());
    }
}
