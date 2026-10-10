// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Geometry primitives: Edge, Face (identity-based, independent of topological mesh).

use std::collections::HashMap;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::math::{newell_vector, EPSILON};
use crate::triangulate::{triangulate, Tri3};

/// A line segment between two 3D points with unique entity ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: Uuid,
    pub a: DVec3,
    pub b: DVec3,
}

impl Edge {
    pub fn new(a: DVec3, b: DVec3) -> Self {
        Self {
            id: Uuid::new_v4(),
            a,
            b,
        }
    }

    #[inline]
    pub fn length(&self) -> f64 {
        (self.b - self.a).length()
    }

    #[inline]
    pub fn midpoint(&self) -> DVec3 {
        (self.a + self.b) * 0.5
    }

    #[inline]
    pub fn direction(&self) -> DVec3 {
        (self.b - self.a).normalize_or_zero()
    }

    /// Project a point onto this segment and return closest point.
    pub fn closest_point(&self, p: DVec3) -> DVec3 {
        let ab = self.b - self.a;
        let len_sq = ab.length_squared();
        if len_sq < EPSILON {
            return self.a;
        }
        let t = ((p - self.a).dot(ab) / len_sq).clamp(0.0, 1.0);
        self.a + ab * t
    }

    /// Distance from point `p` to this edge segment.
    pub fn distance_to_point(&self, p: DVec3) -> f64 {
        (p - self.closest_point(p)).length()
    }
}

/// A planar polygon defined by its outer vertex loop and optional hole loops.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub id: Uuid,
    pub vertices: Vec<DVec3>,
    pub holes: Vec<Vec<DVec3>>,
    pub attrs: HashMap<String, serde_json::Value>,
}

impl Face {
    pub fn new(vertices: Vec<DVec3>) -> Self {
        Self {
            id: Uuid::new_v4(),
            vertices,
            holes: Vec::new(),
            attrs: HashMap::new(),
        }
    }

    pub fn with_holes(vertices: Vec<DVec3>, holes: Vec<Vec<DVec3>>) -> Self {
        Self {
            id: Uuid::new_v4(),
            vertices,
            holes,
            attrs: HashMap::new(),
        }
    }

    /// Newell normal vector of the outer loop.
    pub fn normal(&self) -> DVec3 {
        if self.vertices.len() < 3 {
            return DVec3::Z;
        }
        let n = newell_vector(&self.vertices);
        if n.length_squared() < EPSILON {
            DVec3::Z
        } else {
            n.normalize()
        }
    }

    /// Area of the outer loop (holes not subtracted).
    pub fn area(&self) -> f64 {
        if self.vertices.len() < 3 {
            return 0.0;
        }
        0.5 * newell_vector(&self.vertices).length()
    }

    /// Net area with holes subtracted.
    pub fn net_area(&self) -> f64 {
        let mut a = self.area();
        for hole in &self.holes {
            if hole.len() >= 3 {
                a -= 0.5 * newell_vector(hole).length();
            }
        }
        a.max(0.0)
    }

    /// Centroid of the outer polygon loop.
    pub fn centroid(&self) -> DVec3 {
        let count = self.vertices.len();
        if count == 0 {
            return DVec3::ZERO;
        }
        let sum: DVec3 = self.vertices.iter().copied().sum();
        sum / (count as f64)
    }

    /// Triangulate face into 3D triangles covering the face (outer loop minus holes).
    pub fn triangulate(&self) -> Vec<Tri3> {
        triangulate(&self.vertices, &self.holes, Some(self.normal()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_length_and_closest_point() {
        let e = Edge::new(DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0));
        assert_eq!(e.length(), 10.0);
        let pt = e.closest_point(DVec3::new(5.0, 5.0, 0.0));
        assert_eq!(pt, DVec3::new(5.0, 0.0, 0.0));
        assert_eq!(e.distance_to_point(DVec3::new(5.0, 5.0, 0.0)), 5.0);
    }

    #[test]
    fn test_face_normal_and_area() {
        let verts = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 5.0, 0.0),
            DVec3::new(0.0, 5.0, 0.0),
        ];
        let f = Face::new(verts);
        assert!((f.area() - 20.0).abs() < 1e-6);
        assert!((f.normal().z - 1.0).abs() < 1e-6);
        assert_eq!(f.centroid(), DVec3::new(2.0, 2.5, 0.0));
    }
}
