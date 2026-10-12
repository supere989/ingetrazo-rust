//! Original viewport.py horizon/gradient policy. Small per-frame constants;
//! no texture, vertex buffer, CPU rasterization or extra render pass.
use glam::{DVec3, Mat4, Vec3, Vec4};
/// Quantize the source f32 clear in its RGBA8 domain before Metal's f64 API.
/// Retained source section frames clear to [230,232,235,255]; widening 0.9f32
/// first instead produces red 229 on the native Intel target. This adds no pass.
pub fn clear_rgba8(color: [f32; 4]) -> [f64; 4] {
    color.map(|c| {
        if (0. ..=1.).contains(&c) {
            f64::from((c * 255.).round()) / 255.
        } else {
            // Preserve existing invalid-input refusal in World admission.
            f64::from(c)
        }
    })
}
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Background {
    pub colors: [[f32; 4]; 5],
    pub horizon: f32,
    pub padding: [f32; 3],
}
impl Background {
    pub fn prepare(
        eye: DVec3,
        target: DVec3,
        mvp: [[f32; 4]; 4],
        sky: [f32; 4],
        ground: [f32; 4],
    ) -> Result<Self, String> {
        if !eye.is_finite()
            || !target.is_finite()
            || mvp
                .iter()
                .flatten()
                .chain(sky.iter())
                .chain(ground.iter())
                .any(|x| !x.is_finite())
        {
            return Err("Sky inputs nonfinite".into());
        }
        let eye = eye.as_vec3();
        let fwd = target.as_vec3() - eye;
        let dh = Vec3::new(fwd.x, fwd.y, 0.);
        let fallback = if fwd.z < 0. { 2. } else { -2. };
        let raw = if dh.length() < 1e-5 {
            fallback
        } else {
            let far = eye + dh.normalize() * 1e6;
            let c = Mat4::from_cols_array_2d(&mvp) * Vec4::new(far.x, far.y, far.z, 1.);
            if c.w.abs() < 1e-9 {
                fallback
            } else {
                f64::from(c.y) / f64::from(c.w)
            }
        };
        if !raw.is_finite() {
            return Err("Sky horizon overflow".into());
        }
        let hy = raw.clamp(-1., 1.);
        let mix =
            |a: [f64; 3], b: [f64; 3], t: f64| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        // Native StyleConfig stores f32. Preserve the original default constants
        // before the source f64 gradient arithmetic; custom typed colors retain
        // their admitted f32 values (the shared style schema's existing precision).
        let sky = if sky[..3] == [0.925, 0.935, 0.945] {
            [0.925, 0.935, 0.945]
        } else {
            std::array::from_fn(|i| f64::from(sky[i]))
        };
        let ground = if ground[..3] == [0.815, 0.820, 0.815] {
            [0.815, 0.820, 0.815]
        } else {
            std::array::from_fn(|i| f64::from(ground[i]))
        };
        let sh = mix(sky, [1.; 3], 0.55);
        let gh = mix(ground, [1.; 3], 0.30);
        let sa = |y: f64| mix(sh, sky, ((y - raw) / 1.4).clamp(0., 1.));
        let ga = |y: f64| mix(gh, ground, ((raw - y) / 0.9).clamp(0., 1.));
        let colors = [
            sa(hy),
            sa(1.),
            ga(hy),
            ga(-1.),
            mix(mix(sh, gh, 0.5), [0.; 3], 0.12),
        ]
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, 1.]);
        Ok(Self {
            colors,
            horizon: hy as f32,
            padding: [0.; 3],
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_source_clear_bytes_survive_metal_double_api() {
        let source = [0.90, 0.91, 0.92, 1.0];
        assert_eq!(clear_rgba8(source).map(|c| (c * 255.).round() as u8), [230,232,235,255]);
        assert_eq!((f64::from(source[0]) * 255.).round() as u8, 229);
        let invalid = clear_rgba8([f32::NAN, -f32::EPSILON, 1. + f32::EPSILON, f32::INFINITY]);
        assert!(invalid[0].is_nan() && invalid[1] < 0. && invalid[2] > 1. && invalid[3].is_infinite());
    }
    #[test]
    fn original_camera_gradient_bits() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/metal-defaults/source.json"
        ))
        .unwrap();
        for row in fixture["sky"].as_array().unwrap() {
            let vec = |name: &str| {
                DVec3::from_array(std::array::from_fn(|i| row[name][i].as_f64().unwrap()))
            };
            let matrix = std::array::from_fn(|c| {
                std::array::from_fn(|r| row["mvp"][c * 4 + r].as_f64().unwrap() as f32)
            });
            let actual = Background::prepare(
                vec("eye"),
                vec("target"),
                matrix,
                [0.925, 0.935, 0.945, 1.],
                [0.815, 0.820, 0.815, 1.],
            )
            .unwrap();
            for c in 0..5 {
                for i in 0..3 {
                    assert_eq!(
                        actual.colors[c][i].to_bits(),
                        row["colors_bits"][c][i].as_u64().unwrap() as u32,
                        "row {row},color {c},component {i}"
                    );
                }
            }
        }
    }
    #[test]
    fn vertical_fallback_and_gradient_domain() {
        let a = Background::prepare(
            DVec3::Z,
            DVec3::ZERO,
            Mat4::IDENTITY.to_cols_array_2d(),
            [0.8; 4],
            [0.4; 4],
        )
        .unwrap();
        assert_eq!(a.horizon, 1.);
        assert!(a.colors.iter().flatten().all(|x| x.is_finite()));
        assert_eq!(std::mem::size_of::<Background>(), 96);
        let b = Background::prepare(
            DVec3::ZERO,
            DVec3::Z,
            Mat4::IDENTITY.to_cols_array_2d(),
            [0.8; 4],
            [0.4; 4],
        )
        .unwrap();
        assert_eq!(b.horizon, -1.);
        assert!(
            Background::prepare(DVec3::NAN, DVec3::ZERO, [[0.; 4]; 4], [0.; 4], [0.; 4]).is_err()
        );
    }
}
