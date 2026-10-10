//! Position-only Qt 6.12 float unprojection arithmetic. Ordinary rendering and
//! other tools retain their existing camera rays. See navigation source audit.
use glam::{DVec3, Vec3};
use ingetrazo_core::{camera::OrbitCamera, math::Ray};
type Matrix = [[f32; 4]; 4]; // column-major, matching QMatrix4x4
fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        0.
    } else {
        let s = v / m;
        m * (s.x * s.x + s.y * s.y + s.z * s.z).sqrt()
    }
}
fn normalized(v: Vec3) -> Vec3 {
    let n = length(v);
    if (n - 1.).abs() <= 1e-5 {
        v
    } else if n.abs() <= 1e-5 {
        Vec3::ZERO
    } else {
        v / n
    }
}
fn identity() -> Matrix {
    let mut m = [[0.; 4]; 4];
    for i in 0..4 {
        m[i][i] = 1.;
    }
    m
}
fn multiply(a: Matrix, b: Matrix) -> Matrix {
    let mut m = [[0.; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            m[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    m
}
fn det2(m: &[[f64; 4]; 4], c: [usize; 2], r: [usize; 2]) -> f64 {
    m[c[0]][r[0]] * m[c[1]][r[1]] - m[c[0]][r[1]] * m[c[1]][r[0]]
}
fn det3(m: &[[f64; 4]; 4], c: [usize; 3], r: [usize; 3]) -> f64 {
    m[c[0]][r[0]] * det2(m, [c[1], c[2]], [r[1], r[2]])
        - m[c[1]][r[0]] * det2(m, [c[0], c[2]], [r[1], r[2]])
        + m[c[2]][r[0]] * det2(m, [c[0], c[1]], [r[1], r[2]])
}
fn without3(excluded: usize) -> [usize; 2] {
    let mut values = [0; 2];
    let mut n = 0;
    for i in 0..3 {
        if i != excluded {
            values[n] = i;
            n += 1;
        }
    }
    values
}
fn without4(excluded: usize) -> [usize; 3] {
    let mut values = [0; 3];
    let mut n = 0;
    for i in 0..4 {
        if i != excluded {
            values[n] = i;
            n += 1;
        }
    }
    values
}
fn inverse(m: Matrix, affine: bool) -> Option<Matrix> {
    let d = m.map(|column| column.map(f64::from));
    let mut inv = [[0.; 4]; 4];
    if affine {
        let det = det3(&d, [0, 1, 2], [0, 1, 2]);
        if det == 0. {
            return None;
        }
        let reciprocal = 1. / det;
        for c in 0..3 {
            for r in 0..3 {
                let cs = without3(r);
                let rs = without3(c);
                let sign = if (c + r) % 2 == 0 { 1. } else { -1. };
                inv[c][r] = (sign * det2(&d, [cs[0], cs[1]], [rs[0], rs[1]]) * reciprocal) as f32;
            }
        }
        for r in 0..3 {
            inv[3][r] = -inv[0][r] * m[3][0] - inv[1][r] * m[3][1] - inv[2][r] * m[3][2];
        }
        inv[3][3] = 1.;
    } else {
        let det = d[0][0] * det3(&d, [1, 2, 3], [1, 2, 3])
            - d[1][0] * det3(&d, [0, 2, 3], [1, 2, 3])
            + d[2][0] * det3(&d, [0, 1, 3], [1, 2, 3])
            - d[3][0] * det3(&d, [0, 1, 2], [1, 2, 3]);
        if det == 0. {
            return None;
        }
        let reciprocal = 1. / det;
        for c in 0..4 {
            for r in 0..4 {
                let cs = without4(r);
                let rs = without4(c);
                let sign = if (c + r) % 2 == 0 { 1. } else { -1. };
                inv[c][r] = (sign
                    * det3(&d, [cs[0], cs[1], cs[2]], [rs[0], rs[1], rs[2]])
                    * reciprocal) as f32;
            }
        }
    }
    Some(inv)
}
fn map(m: &Matrix, p: Vec3) -> Vec3 {
    let mut out = [0.; 4];
    for r in 0..4 {
        out[r] = p.x * m[0][r] + p.y * m[1][r] + p.z * m[2][r] + m[3][r];
    }
    let v = Vec3::new(out[0], out[1], out[2]);
    if out[3] == 1. {
        v
    } else {
        v / out[3]
    }
}
/// Reproduce the original viewport's float near/far unprojection, including
/// QVector3D's final float plane intersection. No rounded-position fitting.
pub fn cursor_ray(camera: &OrbitCamera, x: f64, y: f64, width: f64, height: f64) -> Option<Ray> {
    if ![
        x,
        y,
        width,
        height,
        camera.distance,
        camera.fov_deg,
        camera.aspect,
        camera.znear,
        camera.zfar,
    ]
    .iter()
    .all(|v| v.is_finite())
        || width <= 0.
        || height <= 0.
    {
        return None;
    }
    let target = camera.target.as_vec3();
    let offset = Vec3::new(
        (camera.distance * camera.pitch.cos() * camera.yaw.cos()) as f32,
        (camera.distance * camera.pitch.cos() * camera.yaw.sin()) as f32,
        (camera.distance * camera.pitch.sin()) as f32,
    );
    let eye = target + offset;
    let f = normalized(target - eye);
    let original_up = camera.up.as_vec3();
    let up = if f.dot(original_up).abs() < 0.9999 {
        original_up
    } else {
        let sign = if camera.pitch > 0. { -1. } else { 1. };
        Vec3::new(
            (sign * camera.yaw.cos()) as f32,
            (sign * camera.yaw.sin()) as f32,
            0.,
        )
    };
    let horizontal = Vec3::new(f.x, f.y, 0.);
    let level = if camera.two_point && camera.perspective && length(horizontal) >= 0.2 {
        Some(horizontal / length(horizontal))
    } else {
        None
    };
    let center = level.map(|h| eye + h).unwrap_or(target);
    let forward = normalized(center - eye);
    let side = normalized(forward.cross(if level.is_some() { Vec3::Z } else { up }));
    let up_vector = side.cross(forward);
    let mut view = identity();
    for (r, v) in [side, up_vector, -forward].iter().enumerate() {
        view[0][r] = v.x;
        view[1][r] = v.y;
        view[2][r] = v.z;
        view[3][r] = view[0][r] * (-eye.x) + view[1][r] * (-eye.y) + view[2][r] * (-eye.z);
    }
    let mut projection = [[0.; 4]; 4];
    let far = camera.zfar as f32;
    if camera.perspective {
        let near_double = camera.znear.min((camera.distance * 0.02).max(1e-4));
        let near = near_double as f32;
        let clip = far - near;
        if let Some(level) = level {
            let half_h = near_double * (camera.fov_deg.to_radians() / 2.).tan();
            let half_w = (half_h * camera.aspect) as f32;
            let rel = target - eye;
            let depth = rel.dot(level);
            let shift = f64::from(rel.z) * near_double / f64::from(depth);
            let bottom = (-half_h + shift) as f32;
            let top = (half_h + shift) as f32;
            projection[0][0] = 2. * near / (half_w - (-half_w));
            projection[1][1] = 2. * near / (top - bottom);
            projection[2][1] = (top + bottom) / (top - bottom);
        } else {
            let radians = ((camera.fov_deg as f32) / 2.) * (std::f32::consts::PI / 180.);
            let cotan = radians.cos() / radians.sin();
            projection[0][0] = cotan / (camera.aspect as f32);
            projection[1][1] = cotan;
        }
        projection[2][2] = -(near + far) / clip;
        projection[3][2] = -(2. * near * far) / clip;
        projection[2][3] = -1.;
    } else {
        let half_h = (camera.distance * (camera.fov_deg.to_radians() / 2.).tan()) as f32;
        let half_w =
            (camera.distance * (camera.fov_deg.to_radians() / 2.).tan() * camera.aspect) as f32;
        projection[0][0] = 2. / (half_w - (-half_w));
        projection[1][1] = 2. / (half_h - (-half_h));
        projection[2][2] = -2. / (far - (-far));
        projection[3][3] = 1.;
    }
    let inv = inverse(multiply(projection, view), !camera.perspective)?;
    let ndc = Vec3::new(
        (2. * x / width - 1.) as f32,
        (1. - 2. * y / height) as f32,
        -1.,
    );
    let near = map(&inv, ndc);
    let far = map(&inv, Vec3::new(ndc.x, ndc.y, 1.));
    let direction = far - near;
    if length(direction) < 1e-9 {
        return None;
    }
    let direction = normalized(direction);
    if !near.is_finite() || !direction.is_finite() {
        return None;
    }
    // Ray::new normalizes again in f64; preserve the source's f32 direction.
    Some(Ray {
        origin: near.as_dvec3(),
        dir: direction.as_dvec3(),
    })
}
pub fn plane_hit(ray: &Ray, point: DVec3, normal: DVec3) -> Option<DVec3> {
    let origin = ray.origin.as_vec3();
    let direction = ray.dir.as_vec3();
    let normal = normal.as_vec3();
    let length = length(normal);
    if length < 1e-9 {
        return None;
    }
    let n = normal / length;
    let denominator = n.dot(direction);
    if f64::from(denominator.abs()) < 6_f64.to_radians().sin() {
        return None;
    }
    let t = n.dot(point.as_vec3() - origin) / denominator;
    if t < 0. {
        return None;
    }
    let point = origin + direction * t;
    point.is_finite().then(|| point.as_dvec3())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_qt_iso_ground_and_ordinary_ray_are_distinct_without_camera_mutation() {
        let mut camera = OrbitCamera::new();
        camera.distance = 10.;
        camera.aspect = 4. / 3.;
        camera.perspective = false;
        camera.set_standard_view(ingetrazo_core::camera::StandardView::Iso);
        let before = camera.clone();
        let ray = cursor_ray(&camera, 430., 330., 800., 600.).unwrap();
        assert_eq!(
            plane_hit(&ray, DVec3::ZERO, DVec3::Z),
            Some(DVec3::new(0.87890625, -0.29296875, 0.))
        );
        let ordinary = camera.screen_to_ray(430., 330., 800., 600.);
        let ordinary_hit = ingetrazo_core::math::Plane::new(DVec3::ZERO, DVec3::Z)
            .intersect_ray(&ordinary)
            .map(|t| ordinary.at(t))
            .unwrap();
        assert!(ordinary_hit.distance(plane_hit(&ray, DVec3::ZERO, DVec3::Z).unwrap()) > 0.0002);
        assert_eq!(camera, before);
    }
    #[test]
    fn rejected_ray_inputs_and_grazing_or_behind_plane_leave_camera_unchanged() {
        let camera = OrbitCamera::new();
        assert!(cursor_ray(&camera, f64::NAN, 0., 800., 600.).is_none());
        assert!(cursor_ray(&camera, 0., 0., 0., 600.).is_none());
        let ray = Ray {
            origin: DVec3::Z,
            dir: DVec3::X,
        };
        assert!(plane_hit(&ray, DVec3::ZERO, DVec3::Z).is_none());
        let ray = Ray {
            origin: DVec3::Z,
            dir: DVec3::Z,
        };
        assert!(plane_hit(&ray, DVec3::ZERO, DVec3::Z).is_none());
        let mut invalid = camera;
        invalid.distance = f64::INFINITY;
        assert!(cursor_ray(&invalid, 0., 0., 800., 600.).is_none());
    }
}
