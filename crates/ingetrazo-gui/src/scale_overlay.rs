// SPDX-License-Identifier: GPL-3.0-or-later
//! Source Scale eye-plane clipping; GUI-only, separate from renderer clipping.
use glam::{DMat4, DVec3, Vec3};
pub fn clip_segment_front(matrix: DMat4, a: DVec3, b: DVec3) -> Option<(DVec3, DVec3)> {
    let c = matrix.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let w = |p: Vec3| (c[0][3] * p.x + c[1][3] * p.y + c[2][3] * p.z + c[3][3]) as f64;
    let a = a.as_vec3();
    let b = b.as_vec3();
    let wa = w(a);
    let wb = w(b);
    let eps = 1e-3;
    if !a.is_finite() || !b.is_finite() || !wa.is_finite() || !wb.is_finite() {
        return None;
    }
    if wa <= eps && wb <= eps {
        return None;
    }
    if wa <= eps || wb <= eps {
        let t = ((eps - wa) / (wb - wa)) as f32;
        let cut = (a + (b - a) * t).as_dvec3();
        return if wa <= eps {
            Some((cut, b.as_dvec3()))
        } else {
            Some((a.as_dvec3(), cut))
        };
    }
    Some((a.as_dvec3(), b.as_dvec3()))
}
