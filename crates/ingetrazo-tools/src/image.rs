//! Imported reference-image placement: transient corners, one deferred Add.
use crate::base::{ImagePlacementState, Tool, ToolContext, ToolType};
use crate::shape_plane as q;
use glam::{DVec3, Vec3};
use ingetrazo_core::{image_plane::ImagePlane, scene::Scene};
pub struct ImageTool {
    path: Option<String>,
    aspect: f64,
    label: String,
    start: Option<DVec3>,
    hover: Option<DVec3>,
    plane: Option<(DVec3, DVec3)>,
    free: bool,
    pending: Option<ImagePlane>,
    axes: ingetrazo_core::drawing_axes::DrawingAxes,
}
impl Default for ImageTool {
    fn default() -> Self {
        Self {
            path: None,
            aspect: 1.,
            label: String::new(),
            start: None,
            hover: None,
            plane: None,
            free: false,
            pending: None,
            axes: Default::default(),
        }
    }
}
fn normalized(v: DVec3) -> DVec3 {
    let v = v.as_vec3();
    let len = q::length(v.as_dvec3());
    if (len - 1.).abs() <= 1e-5 {
        return v.as_dvec3();
    }
    if len.abs() <= 1e-5 {
        return DVec3::ZERO;
    }
    Vec3::new(v.x / len as f32, v.y / len as f32, v.z / len as f32).as_dvec3()
}
impl ImageTool {
    pub fn loaded(path: String, aspect: f64, label: String) -> Self {
        Self {
            path: Some(path),
            aspect: if aspect > 0. && aspect.is_finite() {
                aspect
            } else {
                1.
            },
            label,
            ..Default::default()
        }
    }
    fn reset(&mut self) {
        self.start = None;
        self.plane = None;
        self.free = false;
    }
    fn axes(&self) -> (DVec3, DVec3) {
        let Some((_, normal)) = self.plane else {
            return (DVec3::X, DVec3::Y);
        };
        let n = normalized(normalized(normal));
        let mut u = q::sub(self.axes.x, q::mul(n, q::dot(self.axes.x, n)));
        if q::length(u) < 0.1 {
            u = q::sub(self.axes.y, q::mul(n, q::dot(self.axes.y, n)));
        }
        let u = normalized(u);
        (u, normalized(q::cross(n, u)))
    }
    fn frame(&self, a: DVec3, b: DVec3) -> (DVec3, DVec3) {
        let (au, av) = self.axes();
        let d = q::sub(b, a);
        let du = q::dot(d, au);
        let dv = q::dot(d, av);
        if self.free || du.abs() < 1e-4 {
            return (q::mul(au, du), q::mul(av, dv));
        }
        (
            q::mul(au, du),
            q::mul(
                av,
                (if dv < 0. { -1. } else { 1. }) * du.abs() * self.aspect,
            ),
        )
    }
    fn commit(&mut self, origin: DVec3, u: DVec3, v: DVec3) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        let mut image = ImagePlane::new(path, origin, u, v);
        image.aspect = self.aspect;
        image.name = self.label.clone();
        self.pending = Some(image);
    }
}
impl Tool for ImageTool {
    fn tool_type(&self) -> ToolType {
        ToolType::Image
    }
    fn name(&self) -> &'static str {
        "Image"
    }
    fn vcb_label(&self) -> &'static str {
        "Width"
    }
    fn on_activate(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_deactivate(&mut self, _: &mut Scene) {
        self.reset();
        self.hover = None;
        self.path = None;
        self.pending = None;
    }
    fn on_cancel(&mut self, _: &mut Scene) {
        self.reset();
    }
    fn on_click(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        if self.path.is_none() {
            return;
        }
        self.axes = scene.drawing_axes();
        self.free = ctx.shift_held;
        let p = q::q(ctx.world);
        if let Some(start) = self.start {
            let (u, v) = self.frame(start, p);
            if q::length(u) >= 1e-4 && q::length(v) >= 1e-4 {
                self.commit(start, u, v);
            }
        } else {
            self.start = Some(p);
            if let Some(hit) = ingetrazo_core::screen_pick::hover_face_placement(scene, &ctx.ray) {
                // Source face_plane_world stores the face centroid, not the
                // ray intersection. Placed components accumulate mapped Qt points.
                let point = if hit.group_path.is_empty() {
                    q::q(hit.mesh.face_centroid(hit.face))
                } else {
                    let vertices = &hit.mesh.faces[hit.face].vertices;
                    let mut sum = DVec3::ZERO;
                    for &vertex in vertices {
                        sum = q::add(sum, ingetrazo_core::change_axes::map(
                            hit.world, hit.mesh.vertices[vertex].position));
                    }
                    let count = vertices.len() as f32;
                    (sum.as_vec3() / count).as_dvec3()
                };
                self.plane = Some((point, q::q(hit.normal)));
            }
        }
    }
    fn on_move(&mut self, scene: &mut Scene, ctx: &ToolContext) {
        self.axes = scene.drawing_axes();
        self.hover = Some(q::q(ctx.world));
        self.free = ctx.shift_held;
    }
    fn snap_anchor(&self) -> Option<DVec3> {
        self.start
    }
    fn drawing_plane(&self) -> Option<(DVec3, DVec3)> {
        self.plane
    }
    fn image_creation_request(&self) -> Option<&ImagePlane> {
        self.pending.as_ref()
    }
    fn take_image_creation(&mut self) -> Option<ImagePlane> {
        self.pending.take()
    }
    fn finish_image_creation(&mut self) {
        self.reset();
        self.path = None;
    }
    fn image_placement_state(&self) -> Option<ImagePlacementState<'_>> {
        Some(ImagePlacementState {
            path: self.path.as_deref(),
            aspect: self.aspect,
            start: self.start,
            hover: self.hover,
            work_plane: self.plane,
            free_aspect: self.free,
        })
    }
    fn image_size_label(&self) -> Option<(f64, f64, DVec3, bool)> {
        let (a, b) = (self.start?, self.hover?);
        self.path.as_ref()?;
        let (u, v) = self.frame(a, b);
        Some((
            q::length(u),
            q::length(v),
            q::add(a, q::mul(q::add(u, v), 0.5)),
            self.free,
        ))
    }
    fn rubber_band_lines(&self) -> Vec<(DVec3, DVec3)> {
        let (Some(a), Some(b)) = (self.start, self.hover) else {
            return vec![];
        };
        if self.path.is_none() {
            return vec![];
        }
        let (u, v) = self.frame(a, b);
        let p = [a, q::add(a, u), q::add(q::add(a, u), v), q::add(a, v)];
        (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect()
    }
    fn on_vcb_input(&mut self, scene: &mut Scene, text: &str) -> Result<(), String> {
        let (Some(a), Some(b)) = (self.start, self.hover) else {
            return Err("Click a corner and move before entering image size".into());
        };
        if self.path.is_none() {
            return Err("Import an image first".into());
        }
        let v = ingetrazo_core::units::vcb::parse_values(
            text,
            ingetrazo_core::units::model_scale(scene),
            false,
        )?;
        if !(1..=2).contains(&v.len()) || v.iter().any(|x| !x.is_finite() || *x <= 0.) {
            return Err("Enter a positive width or width;height".into());
        }
        let (au, av) = self.axes();
        let d = q::sub(b, a);
        let su = if q::dot(d, au) < 0. { -1. } else { 1. };
        let sv = if q::dot(d, av) < 0. { -1. } else { 1. };
        let h = if v.len() == 2 {
            v[1]
        } else {
            v[0] * self.aspect
        };
        self.commit(a, q::mul(au, su * v[0]), q::mul(av, sv * h));
        Ok(())
    }
}
