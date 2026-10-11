//! Float camera arithmetic used by original logical-pixel inference. This
//! does not replace the native camera or render/picking matrices.
use crate::{camera::OrbitCamera, math::source_normalized};
use glam::{Mat4, Vec3, Vec4};
fn dot(a: Vec3, b: Vec3) -> f32 {
    (a.x * b.x + a.y * b.y) + a.z * b.z
}
fn length(v: Vec3) -> f32 {
    let m = v.abs().max_element();
    if m == 0. {
        return 0.;
    }
    let n = Vec3::new(v.x / m, v.y / m, v.z / m);
    m * dot(n, n).sqrt()
}
fn look_at(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
    let forward = source_normalized(center - eye);
    let side = source_normalized(forward.cross(up));
    let up = side.cross(forward);
    Mat4::from_cols(
        Vec4::new(side.x, up.x, -forward.x, 0.),
        Vec4::new(side.y, up.y, -forward.y, 0.),
        Vec4::new(side.z, up.z, -forward.z, 0.),
        Vec4::new(dot(side, -eye), dot(up, -eye), dot(-forward, -eye), 1.),
    )
}
pub(super) fn matrix(c: &OrbitCamera) -> Mat4 {
    let target = c.target.as_vec3();
    let eye = target
        + Vec3::new(
            (c.distance * c.pitch.cos() * c.yaw.cos()) as f32,
            (c.distance * c.pitch.cos() * c.yaw.sin()) as f32,
            (c.distance * c.pitch.sin()) as f32,
        );
    let delta = target - eye;
    let forward = if dot(delta, delta) > 1e-18 {
        source_normalized(delta)
    } else {
        Vec3::Y
    };
    let horizontal = Vec3::new(forward.x, forward.y, 0.);
    let h = length(horizontal);
    let level = (c.two_point && c.perspective && h >= 0.2)
        .then(|| Vec3::new(horizontal.x / h, horizontal.y / h, 0.));
    let view = if let Some(level) = level {
        look_at(eye, eye + level, Vec3::Z)
    } else {
        let stored = c.up.as_vec3();
        let up = if dot(forward, stored).abs() < 0.9999 {
            stored
        } else {
            let sign = if c.pitch > 0. { -1. } else { 1. };
            Vec3::new((sign * c.yaw.cos()) as f32, (sign * c.yaw.sin()) as f32, 0.)
        };
        look_at(eye, target, up)
    };
    let near = c.znear.min((c.distance * 0.02).max(1e-4));
    let n = near as f32;
    let f = c.zfar as f32;
    let projection = if c.perspective {
        if let Some(level) = level {
            let half_h = near * (c.fov_deg.to_radians() / 2.).tan();
            let half_w = half_h * c.aspect;
            let shift = delta.z as f64 * near / dot(delta, level) as f64;
            let (left, right, bottom, top) = (
                -half_w as f32,
                half_w as f32,
                (-half_h + shift) as f32,
                (half_h + shift) as f32,
            );
            let (width, height, clip) = (right - left, top - bottom, f - n);
            Mat4::from_cols(
                Vec4::new(2. * n / width, 0., 0., 0.),
                Vec4::new(0., 2. * n / height, 0., 0.),
                Vec4::new(
                    (left + right) / width,
                    (top + bottom) / height,
                    -(n + f) / clip,
                    -1.,
                ),
                Vec4::new(0., 0., -2. * n * f / clip, 0.),
            )
        } else {
            let radians = (c.fov_deg as f32 / 2.) * (std::f32::consts::PI / 180.);
            let cot = radians.cos() / radians.sin();
            let clip = f - n;
            Mat4::from_cols(
                Vec4::new(cot / c.aspect as f32, 0., 0., 0.),
                Vec4::new(0., cot, 0., 0.),
                Vec4::new(0., 0., -(n + f) / clip, -1.),
                Vec4::new(0., 0., -(2. * n * f) / clip, 0.),
            )
        }
    } else {
        let half_h = c.distance * (c.fov_deg.to_radians() / 2.).tan();
        let half_w = half_h * c.aspect;
        let width = half_w as f32 - (-half_w as f32);
        let height = half_h as f32 - (-half_h as f32);
        let clip = f - (-f);
        Mat4::from_cols(
            Vec4::new(2. / width, 0., 0., 0.),
            Vec4::new(0., 2. / height, 0., 0.),
            Vec4::new(0., 0., -2. / clip, 0.),
            Vec4::W,
        )
    };
    projection * view
}
