//! Source QVector3D placement arithmetic, private to Dimension controller.
use glam::{DVec3, Vec3};
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
pub fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        0.
    } else {
        let s = divide(v, m);
        m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
    }
}
fn divide(v: Vec3, n: f32) -> Vec3 {
    Vec3::new(v.x / n, v.y / n, v.z / n)
}
fn frame(a: Vec3, b: Vec3, axis: Option<&str>) -> Option<(Vec3, Vec3)> {
    let u = match axis {
        Some("x") => Vec3::X,
        Some("y") => Vec3::Y,
        Some("z") => Vec3::Z,
        _ => return None,
    };
    let ab = b - a;
    let n = length(ab);
    if n < 1e-9 {
        return None;
    }
    let d = divide(ab, n);
    let e = d - u * dot(d, u);
    let n = length(e);
    if n < 1e-6 {
        return None;
    }
    Some((u, divide(e, n)))
}
pub fn proposed(a: DVec3, b: DVec3, cursor: DVec3) -> (DVec3, Option<String>) {
    let (a, b, p) = (a.as_vec3(), b.as_vec3(), cursor.as_vec3());
    let ab = b - a;
    let n = length(ab);
    if n < 1e-9 {
        return ((p - a).as_dvec3(), None);
    }
    let d = divide(ab, n);
    let aligned = (p - a) - d * dot(p - a, d);
    let w = p - (a + b) * 0.5;
    let along = f64::from(dot(w, d).abs());
    let perp = f64::from(length(w - d * dot(w, d)));
    if along <= 20f64.to_radians().tan() * perp {
        return (aligned.as_dvec3(), None);
    }
    let mut best = None;
    let mut beyond = 0;
    for name in ["x", "y", "z"] {
        if let Some((u, e)) = frame(a, b, Some(name)) {
            let along = f64::from(dot(d, u).abs());
            if along < 15f64.to_radians().sin() || along > 15f64.to_radians().cos() {
                continue;
            }
            let t = dot(p - a, e);
            let span = dot(ab, e);
            let low = 0f32.min(span);
            let high = 0f32.max(span);
            let over = if t < low {
                low - t
            } else if t > high {
                t - high
            } else {
                0.
            };
            if over <= 1e-9 {
                continue;
            }
            beyond += 1;
            if best.as_ref().map(|(old, _, _)| over > *old).unwrap_or(true) {
                best = Some((over, name.to_owned(), e * t));
            }
        }
    }
    if beyond == 1 {
        if let Some((_, axis, offset)) = best {
            return (offset.as_dvec3(), Some(axis));
        }
    }
    (aligned.as_dvec3(), None)
}
pub fn lines_value(a: DVec3, b: DVec3, offset: DVec3, axis: Option<&str>) -> (DVec3, DVec3, f64) {
    let (a, b, offset) = (a.as_vec3(), b.as_vec3(), offset.as_vec3());
    let ab = b - a;
    if let Some((u, e)) = frame(a, b, axis) {
        let ta = dot(offset, e);
        let tb = ta - dot(ab, e);
        (
            (a + e * ta).as_dvec3(),
            (b + e * tb).as_dvec3(),
            f64::from(dot(ab, u).abs()),
        )
    } else {
        let n = length(ab);
        let off = if n < 1e-9 {
            offset
        } else {
            let d = divide(ab, n);
            offset - d * dot(offset, d)
        };
        ((a + off).as_dvec3(), (b + off).as_dvec3(), f64::from(n))
    }
}

pub fn normalized(v: Vec3) -> Vec3 {
    // QVector3D::normalized computes its squared norm in double precision.
    let n = f64::from(v.x) * f64::from(v.x)
        + f64::from(v.y) * f64::from(v.y)
        + f64::from(v.z) * f64::from(v.z);
    if (n - 1.).abs() <= 1e-12 {
        return v;
    }
    if n.abs() <= 1e-12 {
        return Vec3::ZERO;
    }
    let n = n.sqrt();
    Vec3::new(
        (f64::from(v.x) / n) as f32,
        (f64::from(v.y) / n) as f32,
        (f64::from(v.z) / n) as f32,
    )
}
pub fn axes(frame: glam::DMat4) -> [Vec3; 3] {
    let c = frame.to_cols_array_2d();
    let mut x = Vec3::new(c[0][0] as f32, c[0][1] as f32, c[0][2] as f32);
    let mut y = Vec3::new(c[1][0] as f32, c[1][1] as f32, c[1][2] as f32);
    let z = Vec3::new(c[2][0] as f32, c[2][1] as f32, c[2][2] as f32);
    if length(x) < 1e-12 {
        x = Vec3::X;
    }
    x = normalized(x);
    y -= x * dot(y, x);
    if length(y) < 1e-12 {
        y = z.cross(x);
    }
    y = normalized(y);
    let mut zz = normalized(x.cross(y));
    if dot(zz, z) < 0. {
        zz = -zz;
    }
    [x, y, zz]
}
