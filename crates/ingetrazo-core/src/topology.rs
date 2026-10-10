// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Topology queries and graph algorithms over the edge network.
//!
//! Includes cycle finding for automatic face creation when drawing edges (the classic push/pull behavior).

use std::collections::{HashMap, HashSet, VecDeque};
use glam::DVec3;

use crate::math::{newell_vector, PLANAR_TOLERANCE, EPSILON};
use crate::mesh::{Mesh, VertexId};

/// Check if a list of 3D points are coplanar within PLANAR_TOLERANCE.
pub fn is_coplanar(points: &[DVec3]) -> bool {
    if points.len() <= 3 {
        return true;
    }
    let n = newell_vector(points);
    if n.length_squared() < EPSILON {
        return true; // Degenerate / collinear
    }
    let normal = n.normalize();
    let d0 = points[0].dot(normal);
    for &p in &points[1..] {
        if (p.dot(normal) - d0).abs() > PLANAR_TOLERANCE {
            return false;
        }
    }
    true
}

/// Find the smallest simple cycle containing virtual segment `a`–`b`.
/// The segment does not need to exist in the mesh yet.
/// Returns ordered vertex IDs forming the cycle [a, ..., b], so connecting back to `a` closes the loop.
pub fn find_smallest_cycle_through(
    mesh: &Mesh,
    va: VertexId,
    vb: VertexId,
    max_len: usize,
) -> Option<Vec<VertexId>> {
    if va == vb || va >= mesh.vertices.len() || vb >= mesh.vertices.len() {
        return None;
    }

    // Build adjacency graph from existing mesh edges, excluding any existing edge directly between va and vb
    let mut adj: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
    for edge in &mesh.edges {
        let (e0, e1) = (edge.v0, edge.v1);
        if (e0 == va && e1 == vb) || (e0 == vb && e1 == va) {
            continue; // Skip direct existing edge to find real loops
        }
        adj.entry(e0).or_default().push(e1);
        adj.entry(e1).or_default().push(e0);
    }

    if !adj.contains_key(&va) || !adj.contains_key(&vb) {
        return None;
    }

    // BFS search from vb to va to find the shortest path
    let mut parent: HashMap<VertexId, VertexId> = HashMap::new();
    let mut visited: HashSet<VertexId> = HashSet::new();
    let mut queue: VecDeque<(VertexId, usize)> = VecDeque::new();

    queue.push_back((vb, 1));
    visited.insert(vb);

    let mut found = false;
    while let Some((curr, depth)) = queue.pop_front() {
        if curr == va {
            found = true;
            break;
        }
        if depth >= max_len {
            continue;
        }
        if let Some(neighbors) = adj.get(&curr) {
            for &next in neighbors {
                if !visited.contains(&next) {
                    visited.insert(next);
                    parent.insert(next, curr);
                    queue.push_back((next, depth + 1));
                }
            }
        }
    }

    if !found {
        return None;
    }

    // Reconstruct path from va back to vb
    let mut path = Vec::new();
    let mut cur = va;
    path.push(cur);
    while cur != vb {
        if let Some(&p) = parent.get(&cur) {
            path.push(p);
            cur = p;
        } else {
            return None;
        }
    }

    // Verify planarity
    let positions: Vec<DVec3> = path.iter().map(|&v| mesh.vertices[v].position).collect();
    if !is_coplanar(&positions) {
        return None;
    }

    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_cycle() {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(DVec3::new(0.0, 0.0, 0.0));
        let v1 = mesh.add_vertex(DVec3::new(1.0, 0.0, 0.0));
        let v2 = mesh.add_vertex(DVec3::new(1.0, 1.0, 0.0));
        let v3 = mesh.add_vertex(DVec3::new(0.0, 1.0, 0.0));

        mesh.add_edge(v0, v1);
        mesh.add_edge(v1, v2);
        mesh.add_edge(v2, v3);

        // Virtual segment closing v3 to v0
        let cycle = find_smallest_cycle_through(&mesh, v3, v0, 32);
        assert!(cycle.is_some());
        let c = cycle.unwrap();
        assert_eq!(c.len(), 4);
    }
}
