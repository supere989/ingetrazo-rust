// SPDX-License-Identifier: GPL-3.0-or-later
//! Source frame camera and world-to-paper projection shared by derived anchors and overlays.
use glam::DVec3;
use ingetrazo_core::{camera::OrbitCamera, composition::ViewFrame, scene::Scene};
use ingetrazo_render::vector_hlr::OriginalQtPaperBasis;
pub(crate) fn camera(scene: &Scene, live: &OrbitCamera, frame: &ViewFrame) -> OrbitCamera {
    let saved = frame
        .view_key
        .strip_prefix("scene:")
        .and_then(|name| scene.saved_views.iter().find(|v| v.name == name));
    let snapshot = scene;
    // Source SavedView.apply changes camera fields but inherits the live up
    // vector. Applying to an empty layer slice avoids changing model visibility.
    let mut base = live.clone();
    if let Some(saved) = saved {
        saved.apply(&mut base, &mut []);
    }
    let mut camera = crate::composer::frame_camera_with_base(snapshot, frame, &base);
    // Original roll_camera is a no-op at zero degrees; preserve the source up
    // vector instead of recomputing an equivalent f64 screen-up before Qt storage.
    if frame.rot_deg == 0. {
        let bound = frame
            .view_key
            .strip_prefix("scene:")
            .is_some_and(|name| snapshot.saved_views.iter().any(|v| v.name == name));
        camera.up = if matches!(frame.view_key.as_str(), "std:top" | "std:bottom") {
            glam::DVec3::Y
        } else if frame
            .cam_pitch
            .is_some_and(|p| p.abs() < 89.5_f64.to_radians())
        {
            glam::DVec3::Z
        } else if bound {
            base.up
        } else {
            live.up
        };
    }
    camera
}
pub(crate) struct Projector {
    camera: OrbitCamera,
    basis: OriginalQtPaperBasis,
    frame: ViewFrame,
}
impl Projector {
    pub(crate) fn new(
        scene: &Scene,
        live: &OrbitCamera,
        frame: &ViewFrame,
    ) -> Result<Self, String> {
        if [
            frame.rect.x,
            frame.rect.y,
            frame.rect.w,
            frame.rect.h,
            frame.scale_n,
        ]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || frame.rect.w <= 0.
            || frame.rect.h <= 0.
            || frame.scale_n <= 0.
        {
            return Err("Invalid frame projection geometry".into());
        }
        let camera = camera(scene, live, frame);
        let basis = OriginalQtPaperBasis::from_camera(&camera);
        if !camera.view_matrix().is_finite() {
            return Err("Invalid frame projection camera".into());
        }
        Ok(Self {
            camera,
            basis,
            frame: frame.clone(),
        })
    }
    pub(crate) fn camera(&self) -> &OrbitCamera {
        &self.camera
    }
    pub(crate) fn forward(&self) -> DVec3 {
        self.basis.forward
    }
    pub(crate) fn camera_point(&self, p: DVec3) -> [f64; 3] {
        let d = p - self.basis.eye;
        [
            d.dot(self.basis.right),
            d.dot(self.basis.up),
            d.dot(self.basis.forward),
        ]
    }
    pub(crate) fn camera_to_local(&self, [x, y, z]: [f64; 3]) -> Result<[f64; 2], String> {
        let f = &self.frame;
        let ar = f.rect.w / f.rect.h;
        let q = if f.perspective {
            let hh = (self.camera.fov_deg.to_radians() / 2.).tan() * z.max(1e-4);
            [
                ((x / (hh * ar) + 1.) * f.rect.w / 2.).clamp(-10. * f.rect.w, 11. * f.rect.w),
                ((1. - y / hh) * f.rect.h / 2.).clamp(-10. * f.rect.h, 11. * f.rect.h),
            ]
        } else {
            let hh = ingetrazo_core::composition::model_height_for_frame(f.rect.h, f.scale_n) / 2.;
            let hw = hh * ar;
            let k = if hh != 0. { f.rect.h / (2. * hh) } else { 0. };
            [(x + hw) * k, (hh - y) * k]
        };
        if q.iter().any(|v| !v.is_finite()) {
            return Err("Nonfinite frame projection".into());
        }
        Ok(q)
    }
    pub(crate) fn point(&self, p: DVec3) -> Result<[f64; 2], String> {
        self.camera_to_local(self.camera_point(p))
    }
    pub(crate) fn page_point(&self, p: DVec3) -> Result<[f64; 2], String> {
        let q = self.point(p)?;
        Ok([self.frame.rect.x + q[0], self.frame.rect.y + q[1]])
    }
    pub(crate) fn contains_page(&self, q: [f64; 2]) -> bool {
        let r = &self.frame.rect;
        q[0] >= r.x - 0.5 && q[0] <= r.x + r.w + 0.5 && q[1] >= r.y - 0.5 && q[1] <= r.y + r.h + 0.5
    }
}
