// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Polygon triangulation, including polygons with holes.
//!
//! Uses ear clipping with hole bridging via earcutr.
//! Convex hole-free faces use a fast O(n) fan.

use glam::DVec3;
use crate::math::{newell_vector, plane_basis, EPSILON};

pub type Point2 = (f64, f64);
pub type Tri3 = (DVec3, DVec3, DVec3);

/// Check if a 2D simple polygon is convex.
pub fn is_convex(poly: &[Point2]) -> bool {
    let n = poly.len();
    if n < 4 {
        return true;
    }
    let mut sign = 0;
    for i in 0..n {
        let o = poly[i];
        let a = poly[(i + 1) % n];
        let b = poly[(i + 2) % n];
        let cross = (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
        if cross > EPSILON {
            if sign < 0 {
                return false;
            }
            sign = 1;
        } else if cross < -EPSILON {
            if sign > 0 {
                return false;
            }
            sign = -1;
        }
    }
    true
}

/// Triangulate a 3D polygon with optional holes.
pub fn triangulate(
    outer: &[DVec3],
    holes: &[Vec<DVec3>],
    normal: Option<DVec3>,
) -> Vec<Tri3> {
    if outer.len() < 3 {
        return Vec::new();
    }

    let n = normal.unwrap_or_else(|| {
        let nw = newell_vector(outer);
        if nw.length_squared() < EPSILON {
            DVec3::Z
        } else {
            nw.normalize()
        }
    });

    let (u_axis, v_axis) = plane_basis(n);
    let project = |p: DVec3| (p.dot(u_axis), p.dot(v_axis));

    // Source ignores short hole rings for triangulation, while Mesh/IGZ retain
    // their authored topology. They must not divert a convex face from its fan.
    if holes.iter().all(|hole| hole.len() < 3) {
        let outer_2d: Vec<Point2> = outer.iter().map(|&p| project(p)).collect();
        if is_convex(&outer_2d) {
            let mut tris = Vec::with_capacity(outer.len() - 2);
            for i in 1..outer.len() - 1 {
                tris.push((outer[0], outer[i], outer[i + 1]));
            }
            return tris;
        }
    }

    // Prepare 2D coordinates for earcutr
    let mut all_points_3d = Vec::from(outer);
    let mut flat_coords: Vec<f64> = Vec::with_capacity((outer.len() + holes.iter().map(|h| h.len()).sum::<usize>()) * 2);
    for &p in outer {
        let (x, y) = project(p);
        flat_coords.push(x);
        flat_coords.push(y);
    }

    let mut hole_indices = Vec::new();
    for hole in holes {
        if hole.len() >= 3 {
            hole_indices.push(all_points_3d.len());
            for &p in hole {
                all_points_3d.push(p);
                let (x, y) = project(p);
                flat_coords.push(x);
                flat_coords.push(y);
            }
        }
    }

    match earcutr::earcut(&flat_coords, &hole_indices, 2) {
        Ok(tri_indices) => {
            let mut result = Vec::with_capacity(tri_indices.len() / 3);
            for chunk in tri_indices.chunks_exact(3) {
                result.push((
                    all_points_3d[chunk[0]],
                    all_points_3d[chunk[1]],
                    all_points_3d[chunk[2]],
                ));
            }
            result
        }
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convex_quad_fan() {
        let quad = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.0),
        ];
        let tris = triangulate(&quad, &[], None);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn test_holed_polygon() {
        let outer = vec![
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 10.0, 0.0),
            DVec3::new(0.0, 10.0, 0.0),
        ];
        let hole = vec![
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(8.0, 2.0, 0.0),
            DVec3::new(8.0, 8.0, 0.0),
            DVec3::new(2.0, 8.0, 0.0),
        ];
        let tris = triangulate(&outer, &[hole], None);
        assert!(tris.len() >= 4);
    }
}

/// Original no-z-order earcut traversal for interchange formats which serialize
/// triangle order. The ordinary mesh/render earcutr route remains unchanged.
pub fn source_ordered_earcut(
    points: &[glam::DVec2],
    holes: &[usize],
) -> Result<Vec<[usize; 3]>, String> {
    crate::drafting_earcut::earcut(points, holes)
}
