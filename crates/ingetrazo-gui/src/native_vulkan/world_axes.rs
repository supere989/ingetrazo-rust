//! Source viewport.py::_axes_vertices; immutable camera-distance/drawing-frame packet.
use super::{BufferId, ColoredVertex};
use glam::DVec3;
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Key {
    distance: u64,
    frame: Option<[u64; 16]>,
}
pub(super) struct Prepared {
    pub key: Key,
    pub buffer: BufferId,
}
pub(super) fn key(
    scene: &ingetrazo_core::scene::Scene,
    camera: &ingetrazo_core::camera::OrbitCamera,
) -> Key {
    Key {
        distance: camera.distance.to_bits(),
        frame: scene
            .drawing_frame
            .map(|m| m.to_cols_array().map(f64::to_bits)),
    }
}
pub(super) fn vertices(
    scene: &ingetrazo_core::scene::Scene,
    camera: &ingetrazo_core::camera::OrbitCamera,
) -> Result<Vec<ColoredVertex>, String> {
    let spacing = (camera.distance * 0.008).max(1e-4);
    if !spacing.is_finite() {
        return Err("Drawing axis distance is nonfinite".into());
    }
    let axes = scene.drawing_axes();
    let directions = [axes.x, axes.y, axes.z];
    let colors = [[0.86, 0.22, 0.27], [0.16, 0.62, 0.36], [0.20, 0.40, 0.78]];
    let mut out = Vec::new();
    out.try_reserve_exact(3126)
        .map_err(|_| "Drawing axis allocation failed")?;
    for (direction, color) in directions.into_iter().zip(colors) {
        let point = |p: DVec3| -> Result<ColoredVertex, String> {
            let position = p.to_array().map(|v| v as f32);
            if position.iter().any(|v| !v.is_finite()) {
                return Err("Drawing axis coordinate outside float domain".into());
            }
            Ok(ColoredVertex { position, color })
        };
        out.push(point(axes.origin)?);
        out.push(point(axes.origin + direction * 1e5)?);
        let dash = spacing * 0.18;
        for k in 0..520 {
            let t0 = f64::from(k) * spacing;
            let t1 = t0 + dash;
            out.push(point(axes.origin - direction * t0)?);
            out.push(point(axes.origin - direction * t1)?);
        }
    }
    Ok(out)
}
