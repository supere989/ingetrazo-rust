//! Qt 6.12 QTriangulatingStroker float-coordinate primitive construction.
//! No coverage rasterization: these immutable vertices are intended for the
//! existing single-sample annotation attachment, with opaque layer admission.

/// Separate source casts/additions matter: Qt stores each coordinate and the
/// normal in float before constructing its square-cap triangle strip.
/// Keep both original endpoint pairs: collapsing the eight source vertices
/// into four outer corners changes float edge segments at narrow strokes.
pub(crate) fn square_cap_line(a: [f64; 2], b: [f64; 2], width: f64) -> Result<Option<[[f32; 2]; 8]>, String> {
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
        [a[0] + nx, a[1] + ny],
        [a[0] - nx, a[1] - ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
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
    fn exact_narrow_strip_matches_actual_qt_vbo() {
        let expected = [
            [20.02494239807129,20.602949142456055],
            [20.147050857543945,20.27494239807129],
            [20.188945770263672,20.664003372192383],
            [20.311054229736328,20.335996627807617],
            [180.68894958496094,80.41400146484375],
            [180.81105041503906,80.08599853515625],
            [180.8529510498047,80.47505950927734],
            [180.9750518798828,80.14705657958984],
        ];
        let actual = square_cap_line([20.25,20.5],[180.75,80.25],0.35).unwrap().unwrap();
        assert_eq!(actual.map(|p|p.map(f32::to_bits)), expected.map(|p:[f32;2]|p.map(f32::to_bits)));
    }
    #[test]
    fn square_cap_float_sequence_and_collapsed_source_points() {
        assert_eq!(square_cap_line([2., 3.], [8., 3.], 2.).unwrap().unwrap(),
            [[1., 4.], [1., 2.], [2., 4.], [2., 2.], [8., 4.], [8., 2.], [9., 4.], [9., 2.]]);
        assert_eq!(square_cap_line([2., 3.], [2., 3.], 2.).unwrap(), None);
        assert_eq!(square_cap_line([16_777_216., 3.], [16_777_217., 3.], 2.).unwrap(), None);
        assert!(square_cap_line([f64::MAX, 0.], [0., 0.], 1.).is_err());
        assert!(square_cap_line([-f32::MAX as f64, 0.], [f32::MAX as f64, 0.], 1.).is_err());
        assert!(square_cap_line([f64::NAN, 0.], [0., 0.], 1.).is_err());
    }
}
