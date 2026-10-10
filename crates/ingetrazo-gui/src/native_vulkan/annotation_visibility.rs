//! Source comparison preparation. GPU depth resolution remains separately admitted.
//! The source compares a Qt float32 forward-distance against a Python float64
//! threshold. Rounding that threshold to nearest float32 changes boundary results.
pub(crate) fn source_threshold(along_point: f32) -> Result<f32, &'static str> {
    if !along_point.is_finite() { return Err("Nonfinite annotation forward distance"); }
    let point = along_point as f64;
    let threshold = point - 0.02_f64.max(0.003 * point.abs());
    Ok(ceil_f32(threshold))
}
fn ceil_f32(value: f64) -> f32 {
    let nearest = value as f32;
    if (nearest as f64) < value { nearest.next_up() } else { nearest }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gpu_float_comparison_preserves_source_double_boundary() {
        for point in [-1e30_f32,-1000.,-1.,-0.021,0.,0.019,0.021,1.,100.,1000.,1e30] {
            let source = point as f64 - 0.02_f64.max(0.003 * (point as f64).abs());
            let boundary = source_threshold(point).unwrap();
            for surface in [boundary.next_down(),boundary,boundary.next_up(),point] {
                assert_eq!(surface < boundary, (surface as f64) < source,
                    "point={point:?}, surface={surface:?}, source={source:?}");
            }
        }
        assert!(source_threshold(f32::NAN).is_err());
        assert!(source_threshold(f32::INFINITY).is_err());
    }
    #[test]
    fn nearest_rounding_would_hide_a_source_visible_boundary() {
        let threshold = 1.0_f64 + f32::EPSILON as f64 * 0.25;
        let nearest = threshold as f32;
        assert!((nearest as f64) < threshold);
        assert!(!(nearest < nearest));
        assert!(nearest < ceil_f32(threshold));
    }
}
