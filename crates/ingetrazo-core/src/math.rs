// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Mathematical types and geometric utility functions.
//!
//! Z-up convention:
//! - X: Red (East)
//! - Y: Green (North)
//! - Z: Blue (Up)

pub use glam::{DMat4, DQuat, DVec3};
use serde::{Deserialize, Serialize};

pub const KEY_DECIMALS: u32 = 4;
pub const KEY_SCALE: f64 = 10_000.0; // 10^4
pub const PLANAR_TOLERANCE: f64 = 1e-3;
pub const SPLIT_TOLERANCE: f64 = 1e-4;
pub const EPSILON: f64 = 1e-9;

/// QVector3D normalization for source-authorable inference vectors only.
/// Keep the scaled float length and component division order; native model
/// coordinates and generic double geometry continue to use DVec3.
pub(crate) fn source_normalized(v: glam::Vec3) -> glam::Vec3 {
    let m = v.abs().max_element();
    if m == 0. { return glam::Vec3::ZERO; }
    let n = glam::Vec3::new(v.x / m, v.y / m, v.z / m);
    let len = m * ((n.x * n.x + n.y * n.y) + n.z * n.z).sqrt();
    if (len - 1.).abs() <= 1e-5 { v }
    else if len.abs() <= 1e-5 { glam::Vec3::ZERO }
    else { glam::Vec3::new(v.x / len, v.y / len, v.z / len) }
}

/// Quantized integer coordinate key for spatial hashing and vertex welding (~0.1 mm precision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PointKey(pub i64, pub i64, pub i64);

impl PointKey {
    #[inline]
    pub fn from_vec3(p: DVec3) -> Self {
        Self(
            (p.x * KEY_SCALE).round() as i64,
            (p.y * KEY_SCALE).round() as i64,
            (p.z * KEY_SCALE).round() as i64,
        )
    }

    #[inline]
    pub fn to_vec3(self) -> DVec3 {
        DVec3::new(
            self.0 as f64 / KEY_SCALE,
            self.1 as f64 / KEY_SCALE,
            self.2 as f64 / KEY_SCALE,
        )
    }
}

/// Check if two points coincide within the welding tolerance.
#[inline]
pub fn same_position(p: DVec3, q: DVec3) -> bool {
    PointKey::from_vec3(p) == PointKey::from_vec3(q)
}

/// A 3D Ray starting at `origin` extending along normalized `dir`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: DVec3,
    pub dir: DVec3,
}

impl Ray {
    pub fn new(origin: DVec3, dir: DVec3) -> Self {
        Self {
            origin,
            dir: dir.normalize_or_zero(),
        }
    }

    #[inline]
    pub fn at(&self, t: f64) -> DVec3 {
        self.origin + self.dir * t
    }
}

/// A 3D Plane defined by a point and a unit normal vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub point: DVec3,
    pub normal: DVec3,
}

impl Plane {
    pub fn new(point: DVec3, normal: DVec3) -> Self {
        Self {
            point,
            normal: normal.normalize_or_zero(),
        }
    }

    pub fn from_three_points(a: DVec3, b: DVec3, c: DVec3) -> Option<Self> {
        let n = (b - a).cross(c - a);
        if n.length_squared() < EPSILON {
            None
        } else {
            Some(Self::new(a, n.normalize()))
        }
    }

    /// Signed distance from a point to the plane (positive = front side along normal).
    #[inline]
    pub fn distance_to(&self, p: DVec3) -> f64 {
        (p - self.point).dot(self.normal)
    }

    /// Project a point orthogonally onto the plane.
    #[inline]
    pub fn project_point(&self, p: DVec3) -> DVec3 {
        p - self.normal * self.distance_to(p)
    }

    /// Intersect a ray with this plane. Returns distance `t` along the ray if not parallel.
    pub fn intersect_ray(&self, ray: &Ray) -> Option<f64> {
        let denom = ray.dir.dot(self.normal);
        if denom.abs() < EPSILON {
            return None;
        }
        let t = (self.point - ray.origin).dot(self.normal) / denom;
        Some(t)
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: DVec3,
    pub max: DVec3,
}

impl BoundingBox {
    pub fn empty() -> Self {
        Self {
            min: DVec3::splat(f64::INFINITY),
            max: DVec3::splat(f64::NEG_INFINITY),
        }
    }

    pub fn from_points<'a, I>(points: I) -> Self
    where
        I: IntoIterator<Item = &'a DVec3>,
    {
        let mut bb = Self::empty();
        for &p in points {
            bb.extend(p);
        }
        bb
    }

    pub fn extend(&mut self, p: DVec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> DVec3 {
        (self.max - self.min).max(DVec3::ZERO)
    }

    pub fn diagonal(&self) -> f64 {
        self.size().length()
    }

    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }
}

/// Compute Newell's raw normal vector for an arbitrary 3D polygon loop.
/// The vector's direction is the face normal and its length is 2x the area.
pub fn newell_vector(vertices: &[DVec3]) -> DVec3 {
    let count = vertices.len();
    if count < 3 {
        return DVec3::ZERO;
    }
    let mut n = DVec3::ZERO;
    for i in 0..count {
        let curr = vertices[i];
        let next = vertices[(i + 1) % count];
        n.x += (curr.y - next.y) * (curr.z + next.z);
        n.y += (curr.z - next.z) * (curr.x + next.x);
        n.z += (curr.x - next.x) * (curr.y + next.y);
    }
    n
}

/// Get orthogonal basis vectors (u, v) on a plane whose normal is `n`.
pub fn plane_basis(normal: DVec3) -> (DVec3, DVec3) {
    let n = normal.normalize_or_zero();
    let ref_axis = if n.z.abs() < 0.9 {
        DVec3::Z
    } else {
        DVec3::Y
    };
    let u = ref_axis.cross(n).normalize();
    let v = n.cross(u).normalize();
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_key_welding() {
        let p1 = DVec3::new(1.00001, 2.00004, 3.00002);
        let p2 = DVec3::new(1.00002, 2.00003, 3.00001);
        assert_eq!(PointKey::from_vec3(p1), PointKey::from_vec3(p2));
        assert!(same_position(p1, p2));
    }

    #[test]
    fn test_newell_normal_and_area() {
        let square = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 3.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ];
        let n = newell_vector(&square);
        let normal = n.normalize();
        let area = 0.5 * n.length();
        assert!((normal.z - 1.0).abs() < 1e-6);
        assert!((area - 6.0).abs() < 1e-6);
    }
}

/// Möller–Trumbore ray-triangle intersection test.
pub fn ray_triangle_intersect(ray: &Ray, v0: DVec3, v1: DVec3, v2: DVec3) -> Option<(f64, DVec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = ray.dir.cross(edge2);
    let a = edge1.dot(h);
    if a.abs() < 1e-9 {
        return None;
    }
    let f = 1.0 / a;
    let s = ray.origin - v0;
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(edge1);
    let v = f * ray.dir.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = f * edge2.dot(q);
    if t > 1e-9 {
        Some((t, ray.at(t)))
    } else {
        None
    }
}
