//! Drawing-only axes from the original raw context frame.
//!
//! Matches core/group.py::frame_axes and core/axes.py::plane_axes. The
//! orthogonal unit axes are derived at use time; the encoded affine matrix
//! remains unchanged for native persistence. This does not alter topology,
//! texture coordinates, camera clipping, or explicit world-vector input.
use glam::{DMat4, DVec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawingAxes {
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub z: DVec3,
}

impl Default for DrawingAxes {
    fn default() -> Self {
        Self { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, z: DVec3::Z }
    }
}

impl DrawingAxes {
    pub fn from_frame(frame: Option<DMat4>) -> Self {
        let Some(frame) = frame else { return Self::default(); };
        let origin = frame.transform_point3(DVec3::ZERO);
        let mut x = frame.transform_vector3(DVec3::X);
        let mut y = frame.transform_vector3(DVec3::Y);
        let z = frame.transform_vector3(DVec3::Z);
        if x.length() < 1e-12 { x = DVec3::X; }
        x = x.normalize_or_zero();
        y -= x * y.dot(x);
        if y.length() < 1e-12 { y = z.cross(x); }
        y = y.normalize_or_zero();
        let mut zz = x.cross(y).normalize_or_zero();
        if zz.dot(z) < 0. { zz = -zz; }
        Self { origin, x, y, z: zz }
    }

    pub fn axis(self, name: char) -> Option<DVec3> {
        match name { 'x' => Some(self.x), 'y' => Some(self.y), 'z' => Some(self.z), _ => None }
    }

    /// Drawing red projected into the plane, green when red is almost normal.
    pub fn plane_axes(self, normal: DVec3) -> (DVec3, DVec3) {
        let n = normal.normalize_or_zero();
        let mut u = self.x - n * self.x.dot(n);
        if u.length() < 0.1 { u = self.y - n * self.y.dot(n); }
        u = u.normalize_or_zero();
        (u, n.cross(u).normalize_or_zero())
    }
}
