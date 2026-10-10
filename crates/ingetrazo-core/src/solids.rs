// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Solid tools and boolean arithmetic on manifold 3D meshes.
//!
//! Provides Union, Subtract, Intersect, Trim, Outer Shell, and Split operations
//! adhering to classic CAD solid modeling standards:
//! - Watertight closed 2-manifold boundary representation.
//! - Non-destructive material and attribution retention.

use crate::mesh::Mesh;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SolidOp {
    Union,
    Subtract,
    Intersect,
    OuterShell,
    Trim,
    Split,
}

impl SolidOp {
    pub fn name(&self) -> &'static str {
        match self {
            SolidOp::Union => "Union",
            SolidOp::Subtract => "Subtract",
            SolidOp::Intersect => "Intersect",
            SolidOp::OuterShell => "Outer Shell",
            SolidOp::Trim => "Trim",
            SolidOp::Split => "Split",
        }
    }
}

/// Check if a mesh constitutes a closed manifold solid, returning (is_solid, volume).
pub fn solid_report(mesh: &Mesh) -> (bool, Option<f64>) {
    if mesh.vertices.iter().any(|v| !v.position.is_finite())
        || mesh.faces.iter().any(|f| {
            f.vertices
                .iter()
                .chain(f.holes.iter().flatten())
                .any(|&v| v >= mesh.vertices.len())
        })
        || mesh.edges.iter().any(|e| {
            e.v0 >= mesh.vertices.len()
                || e.v1 >= mesh.vertices.len()
                || e.faces.iter().any(|&f| f >= mesh.faces.len())
        })
        || !mesh.is_manifold_solid()
    {
        return (false, None);
    }
    let vol = mesh.volume();
    (vol.is_some(), vol)
}

use crate::groups::Group;
use crate::mesh::MeshFace;
use glam::DMat4;
use manifold_rust::{
    manifold::Manifold,
    types::{BooleanEngine, Error, MeshGL64},
};
use std::collections::HashSet;

struct Sources {
    faces: Vec<MeshFace>,
    edges: Vec<(glam::DVec3, glam::DVec3, crate::mesh::MeshEdge)>,
    soft_pairs: HashSet<(usize, usize)>,
}

fn kernel(mesh: &Mesh, sources: &mut Sources) -> Result<Manifold, String> {
    if !solid_report(mesh).0 {
        return Err("Both targets must be closed manifold solids".into());
    }
    if mesh.vertices.iter().any(|v| !v.position.is_finite()) {
        return Err("Solid coordinates must be finite".into());
    }
    let mut input = MeshGL64::default();
    input.num_prop = 3;
    input.vert_properties = mesh
        .vertices
        .iter()
        .flat_map(|v| v.position.to_array())
        .collect();
    let base = sources.faces.len();
    sources.faces.extend(mesh.faces.iter().cloned());
    for edge in &mesh.edges {
        if !edge.attrs.is_empty()
            || edge.soft
            || edge.curve_id.is_some()
            || edge.layer.is_some()
            || edge.hidden
        {
            sources.edges.push((
                mesh.vertices[edge.v0].position,
                mesh.vertices[edge.v1].position,
                edge.clone(),
            ));
        }
        if edge.soft && edge.faces.len() == 2 {
            let a = base + edge.faces[0];
            let b = base + edge.faces[1];
            sources.soft_pairs.insert((a.min(b), a.max(b)));
        }
    }
    let vertex_ids: std::collections::HashMap<_, _> = mesh
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| (crate::math::PointKey::from_vec3(v.position), i))
        .collect();
    let mut signed_volume = 0.0;
    for fid in 0..mesh.faces.len() {
        let triangles = mesh.triangulate_face(fid);
        if triangles.is_empty() {
            return Err("Solid has an untriangulatable face".into());
        }
        for (a, b, c) in triangles {
            signed_volume += a.dot(b.cross(c)) / 6.0;
            for point in [a, b, c] {
                let id = *vertex_ids
                    .get(&crate::math::PointKey::from_vec3(point))
                    .ok_or("Triangulation introduced an unknown vertex")?;
                input.tri_verts.push(id as u64);
            }
            input.face_id.push((base + fid) as u64);
        }
    }
    // Normalize globally reversed winding while retaining oriented cavity shells.
    if signed_volume < 0.0 {
        for triangle in input.tri_verts.chunks_exact_mut(3) {
            triangle.swap(0, 2);
        }
    }
    let solid = Manifold::from_mesh_gl64(&input);
    check_status(&solid)?;
    if solid.has_self_intersections() {
        return Err("Solid has self-intersecting surfaces".into());
    }
    Ok(solid)
}

fn check_status(solid: &Manifold) -> Result<(), String> {
    if solid.status() != Error::NoError {
        Err(format!(
            "Boolean kernel rejected geometry: {:?}",
            solid.status()
        ))
    } else {
        Ok(())
    }
}

fn output(solid: &Manifold, sources: &Sources, outer_only: bool) -> Result<Mesh, String> {
    check_status(solid)?;
    let native = solid.get_mesh_gl64(-1);
    let points: Vec<glam::DVec3> = native
        .vert_properties
        .chunks_exact(native.num_prop as usize)
        .map(|p| glam::DVec3::new(p[0], p[1], p[2]))
        .collect();
    // Outer Shell discards connected shells with negative signed enclosed volume.
    let mut parent: Vec<usize> = (0..points.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for t in native.tri_verts.chunks_exact(3) {
        let a = root(&mut parent, t[0] as usize);
        let b = root(&mut parent, t[1] as usize);
        let c = root(&mut parent, t[2] as usize);
        parent[b] = a;
        parent[c] = a;
    }
    let mut shell_volume = std::collections::HashMap::<usize, f64>::new();
    for t in native.tri_verts.chunks_exact(3) {
        let r = root(&mut parent, t[0] as usize);
        *shell_volume.entry(r).or_default() +=
            points[t[0] as usize].dot(points[t[1] as usize].cross(points[t[2] as usize])) / 6.0;
    }
    let mut result = Mesh::new();
    let mut provenance = Vec::new();
    for (i, t) in native.tri_verts.chunks_exact(3).enumerate() {
        if outer_only && shell_volume[&root(&mut parent, t[0] as usize)] < 0.0 {
            continue;
        }
        let ids: Vec<_> = t
            .iter()
            .map(|&id| result.add_vertex(points[id as usize]))
            .collect();
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
            return Err("Boolean result is below the document welding tolerance".into());
        }
        let fid = result
            .add_face(ids, vec![])
            .ok_or("Invalid boolean triangle")?;
        let source = native.face_id[i] as usize;
        let original = sources
            .faces
            .get(source)
            .ok_or("Boolean source metadata was lost")?;
        result.faces[fid].attrs = original.attrs.clone();
        result.faces[fid].layer = original.layer.clone();
        provenance.push(source);
    }
    let (fused, fused_sources) = fuse_source_faces(&result, &provenance)?;
    result = fused;
    provenance = fused_sources;
    for edge in &mut result.edges {
        if edge.faces.len() == 2 {
            let a = provenance[edge.faces[0]];
            let b = provenance[edge.faces[1]];
            edge.soft = a == b || sources.soft_pairs.contains(&(a.min(b), a.max(b)));
        }
    }
    // Surviving portions of original edges retain curve/layer/extension metadata.
    for edge in &mut result.edges {
        let p = result.vertices[edge.v0].position;
        let q = result.vertices[edge.v1].position;
        for (a, b, original) in &sources.edges {
            let direction = *b - *a;
            let length2 = direction.length_squared();
            if length2 <= 1e-20 {
                continue;
            }
            let on_segment = |point: glam::DVec3| {
                let t = (point - *a).dot(direction) / length2;
                (-1e-9..=1.0 + 1e-9).contains(&t) && (point - (*a + t * direction)).length() <= 1e-8
            };
            if on_segment(p) && on_segment(q) {
                edge.attrs = original.attrs.clone();
                edge.curve_id = original.curve_id;
                edge.layer = original.layer.clone();
                edge.hidden = original.hidden;
                edge.soft |= original.soft;
                break;
            }
        }
    }
    if !result.faces.is_empty() && !solid_report(&result).0 {
        return Err("Boolean result cannot be represented as a closed document mesh".into());
    }
    if let Some(volume) = result.volume() {
        // The document mesh welds at 0.1 mm, which can alter a valid kernel result.
        // Reject lossy topology/geometry rather than silently accepting it.
        let expected = if outer_only {
            shell_volume.values().filter(|&&v| v > 0.0).sum::<f64>()
        } else {
            solid.volume()
        };
        if (volume - expected).abs() > 1e-8 * expected.max(1.0) {
            return Err("Boolean result loses volume at the document welding tolerance".into());
        }
    }
    Ok(result)
}

/// Remove triangulation seams within each source face. Preserve every boundary
/// subdivision vertex so adjoining source faces remain topologically conforming.
fn fuse_source_faces(mesh: &Mesh, provenance: &[usize]) -> Result<(Mesh, Vec<usize>), String> {
    use std::collections::{BTreeMap, HashMap};
    let mut groups = BTreeMap::<usize, Vec<usize>>::new();
    for (face, &source) in provenance.iter().enumerate() {
        groups.entry(source).or_default().push(face);
    }
    let mut result = Mesh::new();
    let mut sources = Vec::new();
    for (source, faces) in groups {
        let mut boundary = HashMap::<(usize, usize), bool>::new();
        for &fid in &faces {
            let ring = &mesh.faces[fid].vertices;
            for i in 0..ring.len() {
                let edge = (ring[i], ring[(i + 1) % ring.len()]);
                if boundary.remove(&(edge.1, edge.0)).is_none() {
                    boundary.insert(edge, true);
                }
            }
        }
        let mut outgoing = BTreeMap::<usize, Vec<usize>>::new();
        for &(a, b) in boundary.keys() {
            outgoing.entry(a).or_default().push(b);
        }
        // Point-connected regions or pinched boundaries need separate triangles;
        // preserving them is safer than guessing an ambiguous polygon topology.
        if outgoing.values().any(|v| v.len() != 1) {
            for &fid in &faces {
                let ids = mesh.faces[fid]
                    .vertices
                    .iter()
                    .map(|&v| result.add_vertex(mesh.vertices[v].position))
                    .collect();
                let id = result.add_face(ids, vec![]).ok_or("Invalid boolean face")?;
                result.faces[id].attrs = mesh.faces[fid].attrs.clone();
                result.faces[id].layer = mesh.faces[fid].layer.clone();
                sources.push(source);
            }
            continue;
        }
        let mut rings = Vec::<Vec<usize>>::new();
        while let Some((&start, _)) = outgoing.iter().next() {
            let mut ring = vec![start];
            let mut vertex = start;
            loop {
                let next = outgoing
                    .remove(&vertex)
                    .ok_or("Unclosed boolean face boundary")?[0];
                if next == start {
                    break;
                }
                ring.push(next);
                if ring.len() > boundary.len() {
                    return Err("Invalid boolean face boundary".into());
                }
                vertex = next;
            }
            rings.push(ring);
        }
        let normal = mesh.face_normal(faces[0]);
        let mut outer = Vec::new();
        let mut holes = Vec::new();
        for ring in rings {
            let positions: Vec<_> = ring.iter().map(|&v| mesh.vertices[v].position).collect();
            if crate::math::newell_vector(&positions).dot(normal) > 0.0 {
                outer.push(ring);
            } else {
                holes.push(ring);
            }
        }
        if outer.is_empty() {
            return Err("Boolean face has no outer boundary".into());
        }
        // Project along the largest normal component for containment tests.
        let axis = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
            0
        } else if normal.y.abs() >= normal.z.abs() {
            1
        } else {
            2
        };
        let project = |p: glam::DVec3| match axis {
            0 => (p.y, p.z),
            1 => (p.x, p.z),
            _ => (p.x, p.y),
        };
        let contains = |ring: &[usize], point: usize| {
            let (x, y) = project(mesh.vertices[point].position);
            let mut inside = false;
            for i in 0..ring.len() {
                let (x0, y0) = project(mesh.vertices[ring[i]].position);
                let (x1, y1) = project(mesh.vertices[ring[(i + 1) % ring.len()]].position);
                if (y0 > y) != (y1 > y) && x < (x1 - x0) * (y - y0) / (y1 - y0) + x0 {
                    inside = !inside;
                }
            }
            inside
        };
        for ring in outer {
            let mut belonging = Vec::new();
            holes.retain(|hole| {
                if contains(&ring, hole[0]) {
                    belonging.push(hole.clone());
                    false
                } else {
                    true
                }
            });
            let ids = ring
                .iter()
                .map(|&v| result.add_vertex(mesh.vertices[v].position))
                .collect();
            let hole_ids = belonging
                .iter()
                .map(|h| {
                    h.iter()
                        .map(|&v| result.add_vertex(mesh.vertices[v].position))
                        .collect()
                })
                .collect();
            let id = result
                .add_face(ids, hole_ids)
                .ok_or("Invalid fused boolean face")?;
            result.faces[id].attrs = mesh.faces[faces[0]].attrs.clone();
            result.faces[id].layer = mesh.faces[faces[0]].layer.clone();
            sources.push(source);
        }
        if !holes.is_empty() {
            return Err("Boolean hole has no containing face".into());
        }
    }
    Ok((result, sources))
}

/// Validate the kernel's strict solid contract, including winding consistency
/// and exact self-intersection rejection, without mutating the mesh.
pub fn validate_solid_mesh(mesh: &Mesh) -> Result<(), String> {
    let mut sources = Sources {
        faces: vec![],
        edges: vec![],
        soft_pairs: HashSet::new(),
    };
    kernel(mesh, &mut sources).map(|_| ())
}

/// Low-level arithmetic: Subtract/Trim compute A−B. Empty meshes are valid.
/// Split requires the multi-result group API.
pub fn perform_solid_op(op: SolidOp, a: &Mesh, b: &Mesh) -> Result<Mesh, String> {
    if op == SolidOp::Split {
        return Err("Split requires run_group_operation".into());
    }
    let mut sources = Sources {
        faces: vec![],
        edges: vec![],
        soft_pairs: HashSet::new(),
    };
    let a = kernel(a, &mut sources)?;
    let b = kernel(b, &mut sources)?;
    let result = match op {
        SolidOp::Union | SolidOp::OuterShell => a.union_with_engine(&b, BooleanEngine::Exact),
        SolidOp::Subtract | SolidOp::Trim => a.difference_with_engine(&b, BooleanEngine::Exact),
        SolidOp::Intersect => a.intersection_with_engine(&b, BooleanEngine::Exact),
        SolidOp::Split => unreachable!(),
    };
    output(&result, &sources, op == SolidOp::OuterShell)
}

/// Prepared non-mutating edit; removed indices refer to the caller's input slice.
pub struct SolidGroupResult {
    pub removed: Vec<usize>,
    pub created: Vec<Group>,
}

/// Group parity: first pick cuts second; Trim keeps cutter; Split yields
/// B−A, A−B, A∩B. Geometry is baked into common world coordinates.
pub fn run_group_operation(op: SolidOp, groups: &[Group]) -> Result<SolidGroupResult, String> {
    if groups.len() < 2 {
        return Err("Select at least two solids".into());
    }
    if matches!(op, SolidOp::Subtract | SolidOp::Trim | SolidOp::Split) && groups.len() != 2 {
        return Err("Subtract, Trim and Split require exactly two solids".into());
    }
    let mut sources = Sources {
        faces: vec![],
        edges: vec![],
        soft_pairs: HashSet::new(),
    };
    let mut solids = Vec::new();
    for group in groups {
        if !group.children.is_empty() {
            return Err("Nested groups are not solid targets".into());
        }
        let determinant = group.transform.determinant();
        if !group.transform.is_finite()
            || !determinant.is_finite()
            || determinant.abs() < 1e-12
            || group.transform.x_axis.w != 0.0
            || group.transform.y_axis.w != 0.0
            || group.transform.z_axis.w != 0.0
            || group.transform.w_axis.w != 1.0
        {
            return Err("Solid transform must be finite and invertible".into());
        }
        let mut world = group.mesh.clone();
        for vertex in &mut world.vertices {
            vertex.position = group.transform.transform_point3(vertex.position);
        }
        // Match Python effective_attrs: explicit face color/texture wins;
        // otherwise bake the container paint before losing the input group.
        if let Some(paint) = group
            .native_document
            .get("material")
            .and_then(|v| v.as_object())
        {
            if !paint.is_empty() {
                for face in &mut world.faces {
                    let own = ["color", "texture"]
                        .iter()
                        .any(|key| face.attrs.get(*key).is_some_and(|v| !v.is_null()));
                    if !own {
                        for key in ["color", "texture", "opacity", "mat"] {
                            if let Some(value) = paint.get(key).filter(|v| !v.is_null()) {
                                face.attrs.insert(key.into(), value.clone());
                            }
                        }
                        if !face
                            .attrs
                            .get("back")
                            .is_some_and(|v| v == &serde_json::Value::Bool(true))
                        {
                            face.attrs.insert("back".into(), true.into());
                        }
                    }
                }
            }
        }
        world.rebuild_spatial_index();
        solids.push(kernel(&world, &mut sources)?);
    }
    let receiver = if matches!(op, SolidOp::Subtract | SolidOp::Trim) {
        1
    } else {
        0
    };
    let pieces = match op {
        SolidOp::Union | SolidOp::OuterShell | SolidOp::Intersect => {
            let mut result = solids[0].clone();
            for solid in &solids[1..] {
                result = if op == SolidOp::Intersect {
                    result.intersection_with_engine(solid, BooleanEngine::Exact)
                } else {
                    result.union_with_engine(solid, BooleanEngine::Exact)
                };
                check_status(&result)?;
            }
            vec![(result, receiver)]
        }
        SolidOp::Subtract | SolidOp::Trim | SolidOp::Split => {
            let overlap = solids[0].intersection_with_engine(&solids[1], BooleanEngine::Exact);
            check_status(&overlap)?;
            if overlap.is_empty() {
                return Err("Solids have no overlap".into());
            }
            if op == SolidOp::Split {
                vec![
                    (
                        solids[1].difference_with_engine(&solids[0], BooleanEngine::Exact),
                        1,
                    ),
                    (
                        solids[0].difference_with_engine(&solids[1], BooleanEngine::Exact),
                        0,
                    ),
                    (overlap, 0),
                ]
            } else {
                vec![(
                    solids[1].difference_with_engine(&solids[0], BooleanEngine::Exact),
                    receiver,
                )]
            }
        }
    };
    let mut created = Vec::new();
    for (piece, owner) in pieces {
        let mesh = output(&piece, &sources, op == SolidOp::OuterShell)?;
        if mesh.faces.is_empty() {
            continue;
        }
        let mut group = Group::with_mesh(op.name(), mesh);
        group.transform = DMat4::IDENTITY;
        group.layer = groups[owner].layer.clone();
        created.push(group);
    }
    if created.is_empty() {
        return Err("Operation produced no solid".into());
    }
    let removed = if op == SolidOp::Trim {
        vec![1]
    } else {
        (0..groups.len()).collect()
    };
    Ok(SolidGroupResult { removed, created })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn make_cube(offset: DVec3) -> Mesh {
        let mut mesh = Mesh::new();
        let v000 = mesh.add_vertex(offset + DVec3::new(0.0, 0.0, 0.0));
        let v100 = mesh.add_vertex(offset + DVec3::new(1.0, 0.0, 0.0));
        let v110 = mesh.add_vertex(offset + DVec3::new(1.0, 1.0, 0.0));
        let v010 = mesh.add_vertex(offset + DVec3::new(0.0, 1.0, 0.0));
        let v001 = mesh.add_vertex(offset + DVec3::new(0.0, 0.0, 1.0));
        let v101 = mesh.add_vertex(offset + DVec3::new(1.0, 0.0, 1.0));
        let v111 = mesh.add_vertex(offset + DVec3::new(1.0, 1.0, 1.0));
        let v011 = mesh.add_vertex(offset + DVec3::new(0.0, 1.0, 1.0));

        mesh.add_face(vec![v000, v010, v110, v100], vec![]);
        mesh.add_face(vec![v001, v101, v111, v011], vec![]);
        mesh.add_face(vec![v000, v100, v101, v001], vec![]);
        mesh.add_face(vec![v100, v110, v111, v101], vec![]);
        mesh.add_face(vec![v110, v010, v011, v111], vec![]);
        mesh.add_face(vec![v010, v000, v001, v011], vec![]);
        mesh
    }

    #[test]
    fn test_solid_report_and_union() {
        let cube1 = make_cube(DVec3::ZERO);
        let cube2 = make_cube(DVec3::new(2.0, 0.0, 0.0));

        let (solid1, vol1) = solid_report(&cube1);
        assert!(solid1);
        assert!((vol1.unwrap() - 1.0).abs() < 1e-4);

        let res = perform_solid_op(SolidOp::Union, &cube1, &cube2).unwrap();
        assert!(res.is_manifold_solid());
        assert!((res.volume().unwrap() - 2.0).abs() < 1e-9);
    }
    fn scaled_cube(offset: DVec3, scale: f64) -> Mesh {
        let mut mesh = make_cube(DVec3::ZERO);
        for vertex in &mut mesh.vertices {
            vertex.position = offset + scale * vertex.position;
        }
        mesh.rebuild_spatial_index();
        mesh
    }

    fn assert_volume(mesh: &Mesh, expected: f64) {
        assert!(mesh.is_manifold_solid());
        assert!(
            (mesh.volume().unwrap() - expected).abs() < 1e-8,
            "volume {:?}, expected {expected}",
            mesh.volume()
        );
    }

    #[test]
    fn overlap_containment_coincident_and_empty() {
        let a = make_cube(DVec3::ZERO);
        let b = make_cube(DVec3::new(0.5, 0.0, 0.0));
        for (op, vol) in [
            (SolidOp::Union, 1.5),
            (SolidOp::Subtract, 0.5),
            (SolidOp::Intersect, 0.5),
        ] {
            assert_volume(&perform_solid_op(op, &a, &b).unwrap(), vol);
        }
        assert_volume(&perform_solid_op(SolidOp::Union, &a, &a).unwrap(), 1.0);
        assert!(perform_solid_op(SolidOp::Subtract, &a, &a)
            .unwrap()
            .faces
            .is_empty());
        let disjoint = make_cube(DVec3::splat(4.0));
        assert!(perform_solid_op(SolidOp::Intersect, &a, &disjoint)
            .unwrap()
            .faces
            .is_empty());
        let inner = scaled_cube(DVec3::splat(0.25), 0.5);
        let hollow = perform_solid_op(SolidOp::Subtract, &a, &inner).unwrap();
        assert_volume(&hollow, 0.875);
        assert_volume(
            &perform_solid_op(SolidOp::Union, &hollow, &disjoint).unwrap(),
            1.875,
        );
        assert_volume(
            &perform_solid_op(SolidOp::OuterShell, &hollow, &disjoint).unwrap(),
            2.0,
        );
    }

    #[test]
    fn group_transforms_trim_split_direction_and_provenance() {
        let mut cutter = Group::with_mesh("cutter", scaled_cube(DVec3::ZERO, 0.5));
        cutter.transform = DMat4::from_translation(DVec3::splat(0.25));
        cutter.layer = Some("cutter-layer".into());
        let mut receiver = Group::with_mesh("receiver", make_cube(DVec3::ZERO));
        receiver.layer = Some("receiver-layer".into());
        for face in &mut cutter.mesh.faces {
            face.attrs.insert("material".into(), "cut".into());
        }
        for face in &mut receiver.mesh.faces {
            face.attrs.insert("material".into(), "outside".into());
        }
        let groups = [cutter, receiver];
        let before = groups.clone();
        let trim = run_group_operation(SolidOp::Trim, &groups).unwrap();
        assert_eq!(trim.removed, vec![1]);
        assert_eq!(groups, before);
        assert_eq!(trim.created[0].layer.as_deref(), Some("receiver-layer"));
        assert_eq!(trim.created[0].transform, DMat4::IDENTITY);
        assert_volume(&trim.created[0].mesh, 0.875);
        let materials: HashSet<_> = trim.created[0]
            .mesh
            .faces
            .iter()
            .map(|f| f.attrs["material"].as_str().unwrap())
            .collect();
        assert_eq!(materials, HashSet::from(["cut", "outside"]));
        let split = run_group_operation(SolidOp::Split, &groups).unwrap();
        // Contained cutter: A−B empty, remaining two pieces partition receiver.
        assert_eq!(split.created.len(), 2);
        assert_volume(&split.created[0].mesh, 0.875);
        assert_volume(&split.created[1].mesh, 0.125);
        let mut transformed = Group::with_mesh("rotated", make_cube(DVec3::ZERO));
        transformed.transform = DMat4::from_rotation_z(std::f64::consts::PI)
            * DMat4::from_scale(DVec3::new(2.0, 1.0, 1.0));
        transformed.transform =
            DMat4::from_translation(DVec3::new(2.0, 1.0, 0.0)) * transformed.transform;
        let target = Group::with_mesh("target", make_cube(DVec3::ZERO));
        let result = run_group_operation(SolidOp::Intersect, &[target, transformed]).unwrap();
        assert_volume(&result.created[0].mesh, 1.0);
    }

    #[test]
    fn rejects_invalid_inputs_without_mutation() {
        let a = make_cube(DVec3::ZERO);
        let original = a.clone();
        let mut broken = a.clone();
        broken.faces[0].vertices[0] = usize::MAX;
        assert!(perform_solid_op(SolidOp::Union, &a, &broken).is_err());
        let mut reversed_face = a.clone();
        reversed_face.faces[0].vertices.reverse();
        assert!(perform_solid_op(SolidOp::Union, &a, &reversed_face).is_err());
        let groups = [
            Group::with_mesh("a", a.clone()),
            Group::with_mesh("b", make_cube(DVec3::splat(4.0))),
        ];
        let before = groups.clone();
        assert!(run_group_operation(SolidOp::Subtract, &groups).is_err());
        assert_eq!(groups, before);
        assert_eq!(a, original);
    }
    #[test]
    fn rotated_overlap_and_shared_face_union() {
        let a = Group::with_mesh("axis cube", make_cube(DVec3::ZERO));
        let mut b = a.clone();
        b.transform = DMat4::from_translation(DVec3::splat(0.5))
            * DMat4::from_rotation_z(std::f64::consts::FRAC_PI_4)
            * DMat4::from_translation(DVec3::splat(-0.5));
        let intersection = run_group_operation(SolidOp::Intersect, &[a.clone(), b]).unwrap();
        // Independent analytic square intersection: four corner triangles removed.
        assert_volume(&intersection.created[0].mesh, 2.0 * 2.0_f64.sqrt() - 2.0);
        let adjacent = make_cube(DVec3::X);
        assert_volume(
            &perform_solid_op(SolidOp::Union, &a.mesh, &adjacent).unwrap(),
            2.0,
        );
    }
    #[test]
    fn container_paint_is_baked_but_explicit_face_paint_wins() {
        let mut a = Group::with_mesh("painted", make_cube(DVec3::ZERO));
        a.native_document = serde_json::json!({"material": {"color": [1, 0, 0], "mat": "red"}});
        a.mesh.faces[0]
            .attrs
            .insert("color".into(), serde_json::json!([0, 0, 1]));
        let b = Group::with_mesh("disjoint", make_cube(DVec3::splat(3.0)));
        let result = run_group_operation(SolidOp::Union, &[a, b]).unwrap();
        let faces = &result.created[0].mesh.faces;
        assert_eq!(
            faces
                .iter()
                .filter(|f| f.attrs.get("mat") == Some(&"red".into()))
                .count(),
            5
        );
        assert_eq!(
            faces
                .iter()
                .filter(|f| f.attrs.get("color") == Some(&serde_json::json!([0, 0, 1])))
                .count(),
            1
        );
    }
}
