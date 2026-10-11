//! Qt 6.12 QTriangulatingStroker float-coordinate primitive construction.
//! No coverage rasterization: these immutable vertices are intended for the
//! existing single-sample annotation attachment, with opaque layer admission.

/// Separate source casts/additions matter: Qt stores each coordinate and the
/// normal in float before constructing its square-cap triangle strip.
pub(crate) fn square_cap_line(a: [f64; 2], b: [f64; 2], width: f64) -> Result<Option<[[f32; 2]; 4]>, String> {
    if !a.into_iter().chain(b).chain([width]).all(f64::is_finite) || width < 0. {
        return Err("Invalid widget line input".into());
    }
    let a = a.map(|v| v as f32);
    let b = b.map(|v| v as f32);
    let width = if width == 0. { 1. } else { width as f32 };
    if !a.into_iter().chain(b).chain([width]).all(f32::is_finite) {
        return Err("Widget line float conversion overflow".into());
    }
    // Qt removes consecutive points equal after conversion to float.
    if a == b { return Ok(None); }
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let half = width / 2.;
    let scale = half / dx.hypot(dy);
    let nx = -dy * scale;
    let ny = dx * scale;
    let start = [a[0] - ny, a[1] + nx];
    let end = [b[0] + ny, b[1] - nx];
    let vertices = [
        [start[0] + nx, start[1] + ny],
        [start[0] - nx, start[1] - ny],
        [end[0] + nx, end[1] + ny],
        [end[0] - nx, end[1] - ny],
    ];
    if !vertices.iter().flatten().all(|v| v.is_finite()) {
        return Err("Widget line float arithmetic overflow".into());
    }
    Ok(Some(vertices))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_cap_float_sequence_and_collapsed_source_points() {
        assert_eq!(square_cap_line([2., 3.], [8., 3.], 2.).unwrap().unwrap(),
            [[1., 4.], [1., 2.], [9., 4.], [9., 2.]]);
        assert_eq!(square_cap_line([2., 3.], [2., 3.], 2.).unwrap(), None);
        assert_eq!(square_cap_line([16_777_216., 3.], [16_777_217., 3.], 2.).unwrap(), None);
        assert!(square_cap_line([f64::MAX, 0.], [0., 0.], 1.).is_err());
        assert!(square_cap_line([-f32::MAX as f64, 0.], [f32::MAX as f64, 0.], 1.).is_err());
        assert!(square_cap_line([f64::NAN, 0.], [0., 0.], 1.).is_err());
    }
}
