// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.
//! Source circle/arc boundary runs and validated least-squares centre fits.
use crate::mesh::Mesh;
use glam::{DMat3, DVec3};

/// Source `fit_circle`: project into the dominant covariance plane, fit all
/// points, and reject radial deviations larger than 1% or two millimetres.
pub fn fit_circle(points: &[DVec3]) -> Option<(DVec3, f64)> {
    if points.len() < 3 || points.iter().any(|p| !p.is_finite()) {
        return None;
    }
    let mean = points.iter().copied().sum::<DVec3>() / points.len() as f64;
    let mut cov = [[0.; 3]; 3];
    for p in points {
        let q = *p - mean;
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += q[i] * q[j];
            }
        }
    }
    let mut axes = DMat3::IDENTITY;
    for _ in 0..48 {
        let mut pair = (0, 1);
        for (i, j) in [(0, 2), (1, 2)] {
            if cov[i][j].abs() > cov[pair.0][pair.1].abs() {
                pair = (i, j);
            }
        }
        let (p, q) = pair;
        if cov[p][q].abs() < 1e-15 * (cov[0][0] + cov[1][1] + cov[2][2]).max(f64::MIN_POSITIVE) {
            break;
        }
        let angle = 0.5 * (2. * cov[p][q]).atan2(cov[q][q] - cov[p][p]);
        let (c, s) = (angle.cos(), angle.sin());
        let mut rotation = DMat3::IDENTITY;
        rotation.col_mut(p)[p] = c;
        rotation.col_mut(q)[q] = c;
        rotation.col_mut(p)[q] = -s;
        rotation.col_mut(q)[p] = s;
        let covariance = DMat3::from_cols(
            DVec3::from_array(cov[0]),
            DVec3::from_array(cov[1]),
            DVec3::from_array(cov[2]),
        );
        let rotated = rotation.transpose() * covariance * rotation;
        cov = rotated.to_cols_array_2d();
        axes *= rotation;
    }
    let mut order = [0, 1, 2];
    order.sort_by(|&a, &b| cov[b][b].total_cmp(&cov[a][a]));
    let u = axes.col(order[0]);
    let v = axes.col(order[1]);
    let scale = points
        .iter()
        .map(|p| (*p - mean).length())
        .fold(0., f64::max);
    if scale < 1e-12 {
        return None;
    }
    let mut matrix = DMat3::ZERO;
    let mut rhs = DVec3::ZERO;
    for p in points {
        let q = (*p - mean) / scale;
        let (x, y) = (q.dot(u), q.dot(v));
        let row = DVec3::new(2. * x, 2. * y, 1.);
        matrix += DMat3::from_cols(row * row.x, row * row.y, row * row.z);
        rhs += row * (x * x + y * y);
    }
    if matrix.determinant().abs() < 1e-15 {
        return None;
    }
    let solution = matrix.inverse() * rhs;
    let radius_squared = solution.z + solution.x * solution.x + solution.y * solution.y;
    if radius_squared <= 0. || !solution.is_finite() {
        return None;
    }
    let radius = radius_squared.sqrt() * scale;
    let center = mean + (u * solution.x + v * solution.y) * scale;
    for p in points {
        let q = *p - mean;
        let x = q.dot(u) - solution.x * scale;
        let y = q.dot(v) - solution.y * scale;
        if (x.hypot(y) - radius).abs() > 0.002_f64.max(0.01 * radius) {
            return None;
        }
    }
    Some((center.as_vec3().as_dvec3(), radius))
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Curve(usize),
    Soft,
    Geometry,
}
fn runs(mesh: &Mesh, ring: &[usize]) -> Vec<(Vec<usize>, bool)> {
    let n = ring.len();
    if n < 3 {
        return vec![];
    }
    let turn = |i: usize| {
        let a = mesh.vertices[ring[(i + n - 1) % n]].position;
        let b = mesh.vertices[ring[i]].position;
        let c = mesh.vertices[ring[(i + 1) % n]].position;
        let u = b - a;
        let v = c - b;
        let (lu, lv) = (u.length(), v.length());
        if lu < 1e-9 || lv < 1e-9 {
            None
        } else {
            Some((
                (u.dot(v) / (lu * lv)).clamp(-1., 1.).acos().to_degrees(),
                lu,
                lv,
            ))
        }
    };
    let keys: Vec<_> = (0..n)
        .map(|i| {
            let edge = mesh.vertices[ring[i]]
                .edges
                .iter()
                .filter_map(|&id| mesh.edges.get(id))
                .find(|e| {
                    (e.v0 == ring[i] && e.v1 == ring[(i + 1) % n])
                        || (e.v1 == ring[i] && e.v0 == ring[(i + 1) % n])
                });
            if let Some(cid) = edge.and_then(|e| e.curve_id) {
                return Some(Key::Curve(cid));
            }
            if edge.is_some_and(|e| e.soft) {
                return Some(Key::Soft);
            }
            let ((t, _, length), (next, later, _)) = (turn(i)?, turn((i + 1) % n)?);
            if t > 2.
                && t < 60.
                && (t - next).abs() < 1.5
                && (later - length).abs() < 0.05 * length.max(1e-9)
            {
                Some(Key::Geometry)
            } else {
                None
            }
        })
        .collect();
    if keys[0].is_some() && keys.iter().all(|k| *k == keys[0]) {
        return vec![(ring.to_vec(), matches!(keys[0], Some(Key::Curve(_))))];
    }
    let start = (0..n)
        .find(|&i| keys[i] != keys[(i + n - 1) % n])
        .unwrap_or(0);
    let mut result = vec![];
    let mut current = vec![];
    let mut key = None;
    let flush = |run: &mut Vec<usize>, key: Option<Key>, result: &mut Vec<(Vec<usize>, bool)>| {
        if key.is_some() && run.len() >= 2 {
            let last = *run.last().unwrap();
            let mut vertices = run.iter().map(|&i| ring[i]).collect::<Vec<_>>();
            vertices.push(ring[(last + 1) % n]);
            result.push((vertices, matches!(key, Some(Key::Curve(_)))));
        }
        run.clear();
    };
    for j in 0..n {
        let i = (start + j) % n;
        if keys[i].is_some() && keys[i] == key {
            current.push(i);
        } else {
            flush(&mut current, key, &mut result);
            key = keys[i];
            if key.is_some() {
                current.push(i);
            }
        }
    }
    flush(&mut current, key, &mut result);
    result
}

pub fn face_centers(
    mesh: &Mesh,
    face: usize,
    world: glam::DMat4,
) -> Vec<(DVec3, f64, (usize, usize))> {
    let Some(face) = mesh.faces.get(face) else {
        return vec![];
    };
    let mut centers = vec![];
    for (loop_id, ring) in std::iter::once(&face.vertices)
        .chain(face.holes.iter())
        .enumerate()
    {
        for (run_id, (vertices, drawn)) in runs(mesh, ring).into_iter().enumerate() {
            if vertices.len() < if drawn { 3 } else { 5 } {
                continue;
            }
            let points = vertices
                .iter()
                .map(|&id| world.transform_point3(mesh.vertices[id].position))
                .collect::<Vec<_>>();
            if let Some((center, radius)) = fit_circle(&points) {
                centers.push((center, radius, (loop_id, run_id)));
            }
        }
    }
    centers
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fits_circle_and_arc_but_rejects_bent_polyline() {
        let center = DVec3::new(3., 2., 1.);
        let u = DVec3::new(1., 1., 0.).normalize();
        let v = DVec3::Z;
        for sweep in [std::f64::consts::TAU, std::f64::consts::FRAC_PI_2] {
            let points = (0..24)
                .map(|i| {
                    let a = sweep * i as f64 / 23.;
                    center + (u * a.cos() + v * a.sin()) * 2.
                })
                .collect::<Vec<_>>();
            let (c, r) = fit_circle(&points).unwrap();
            assert!(c.distance(center) < 1e-5);
            assert!((r - 2.).abs() < 1e-8);
        }
        assert!(fit_circle(&[
            DVec3::ZERO,
            DVec3::X,
            DVec3::new(2., 1., 0.),
            DVec3::new(4., 1., 0.),
            DVec3::new(5., -2., 0.)
        ])
        .is_none());
    }
    #[test]
    fn bare_circle_and_hole_runs_have_centers_not_quad_or_non_circular_ring() {
        let mut mesh = Mesh::new();
        let ring = (0..24)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / 24.;
                mesh.add_vertex(DVec3::new(a.cos(), a.sin(), 0.))
            })
            .collect();
        let f = mesh.add_face(ring, vec![]).unwrap();
        let centers = face_centers(&mesh, f, glam::DMat4::IDENTITY);
        assert_eq!(centers.len(), 1);
        assert!(centers[0].0.length() < 1e-5);
        let quad =
            [DVec3::ZERO, DVec3::X, DVec3::X + DVec3::Y, DVec3::Y].map(|p| mesh.add_vertex(p));
        let f = mesh.add_face(quad.to_vec(), vec![]).unwrap();
        assert!(face_centers(&mesh, f, glam::DMat4::IDENTITY).is_empty());
    }
}
