//! Command-owned group axes. Re-expression is prepared before publication.
use crate::{groups::Group, scene::Scene};
use glam::{DMat4, DVec3, Vec3};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub target: Uuid,
    pub frame: DMat4,
    pub old_axes: Option<serde_json::Value>,
    pub delta: Option<DMat4>,
    pub instances: Vec<Uuid>,
    pub undo: bool,
}
pub fn multiply(a: DMat4, b: DMat4) -> DMat4 {
    let a = a.to_cols_array_2d().map(|c| c.map(|x| x as f32));
    let b = b.to_cols_array_2d().map(|c| c.map(|x| x as f32));
    DMat4::from_cols_array_2d(&std::array::from_fn(|c| {
        std::array::from_fn(|r| {
            (a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3]) as f64
        })
    }))
}
pub fn map(m: DMat4, p: DVec3) -> DVec3 {
    let m = m.to_cols_array_2d().map(|c| c.map(|x| x as f32));
    let p = p.as_vec3();
    Vec3::from_array(std::array::from_fn(|r| {
        m[0][r] * p.x + m[1][r] * p.y + m[2][r] * p.z + m[3][r]
    }))
    .as_dvec3()
}
pub fn inverse(m: DMat4) -> Result<DMat4, String> {
    let d = m.determinant();
    if !d.is_finite() || d == 0. {
        return Err("Axes frame is not invertible".into());
    }
    let v = m.inverse().to_cols_array().map(|x| (x as f32) as f64);
    let m = DMat4::from_cols_array(&v);
    if !m.is_finite() {
        return Err("Axes inverse is nonfinite".into());
    }
    Ok(m)
}
pub fn find(groups: &[Group], id: Uuid) -> Option<&Group> {
    for g in groups {
        if g.history_id == id {
            return Some(g);
        }
        if let Some(g) = find(&g.children, id) {
            return Some(g);
        }
    }
    None
}
pub fn find_mut(groups: &mut [Group], id: Uuid) -> Option<&mut Group> {
    for g in groups {
        if g.history_id == id {
            return Some(g);
        }
        if let Some(g) = find_mut(&mut g.children, id) {
            return Some(g);
        }
    }
    None
}
fn instance(g: &Group) -> bool {
    g.component_definition.is_some()
        || g.native_document.get("xform").is_some_and(|v| !v.is_null())
        || g.transform != DMat4::IDENTITY
}

fn transformed(src: &crate::mesh::Mesh, m: DMat4) -> Result<crate::mesh::Mesh, String> {
    use crate::mesh::{Mesh, MeshVertex};
    if src
        .edges
        .iter()
        .any(|e| e.v0 >= src.vertices.len() || e.v1 >= src.vertices.len())
        || src.faces.iter().any(|f| {
            f.vertices
                .iter()
                .chain(f.holes.iter().flatten())
                .any(|&i| i >= src.vertices.len())
        })
    {
        return Err("Axes geometry has invalid vertex references".into());
    }
    let inv = inverse(m)?;
    let bulk = src
        .edges
        .len()
        .saturating_mul(2)
        .saturating_add(src.faces.len().saturating_mul(4))
        >= 1024;
    let mapped = |p: DVec3| {
        if bulk {
            m.transform_point3(p).as_vec3().as_dvec3()
        } else {
            map(m, p)
        }
    };
    let mut mesh = Mesh::new();
    let mut registry = std::collections::HashMap::<[u64; 3], usize>::new();
    fn key(p: DVec3) -> [u64; 3] {
        p.to_array().map(|x| {
            let x = (x * 10000.).round_ties_even();
            if x == 0. {
                0
            } else {
                x.to_bits()
            }
        })
    }
    fn vertex(
        mesh: &mut Mesh,
        registry: &mut std::collections::HashMap<[u64; 3], usize>,
        p: DVec3,
    ) -> Result<usize, String> {
        if !p.is_finite() {
            return Err("Axes geometry overflow".into());
        }
        let k = key(p);
        if let Some(&i) = registry.get(&k) {
            return Ok(i);
        }
        let q = p.to_array().map(|x| (x * 10000.).round_ties_even());
        for dx in [-1., 0., 1.] {
            for dy in [-1., 0., 1.] {
                for dz in [-1., 0., 1.] {
                    if dx == 0. && dy == 0. && dz == 0. {
                        continue;
                    }
                    let n = [q[0] + dx, q[1] + dy, q[2] + dz].map(|x| {
                        if x == 0. {
                            0
                        } else {
                            x.to_bits()
                        }
                    });
                    if let Some(&i) = registry.get(&n) {
                        if ((mesh.vertices[i].position.as_vec3() - p.as_vec3()).length_squared()
                            as f64)
                            < 1e-8
                        {
                            return Ok(i);
                        }
                    }
                }
            }
        }
        let i = mesh.vertices.len();
        mesh.vertices.push(MeshVertex::new(p));
        registry.insert(k, i);
        Ok(i)
    }
    // Source small walk authors faces before copying edge flags. Large bulk path
    // flattens endpoints before face rings; keep that observable row ordering.
    if bulk {
        for e in &src.edges {
            let a = vertex(
                &mut mesh,
                &mut registry,
                mapped(src.vertices[e.v0].position),
            )?;
            let b = vertex(
                &mut mesh,
                &mut registry,
                mapped(src.vertices[e.v1].position),
            )?;
            mesh.add_edge(a, b);
        }
    }
    for face in &src.faces {
        let ring = face
            .vertices
            .iter()
            .map(|&i| vertex(&mut mesh, &mut registry, mapped(src.vertices[i].position)))
            .collect::<Result<Vec<_>, _>>()?;
        let holes = face
            .holes
            .iter()
            .map(|h| {
                h.iter()
                    .map(|&i| vertex(&mut mesh, &mut registry, mapped(src.vertices[i].position)))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(id) = mesh.add_face(ring, holes) {
            mesh.faces[id].layer = face.layer.clone();
            let attrs = &mut mesh.faces[id].attrs;
            *attrs = face.attrs.clone();
            if let Some(texture) = attrs.get_mut("texture").and_then(|v| v.as_object_mut()) {
                if let Some(values) = texture
                    .get("uvw")
                    .and_then(|v| v.as_array())
                    .filter(|v| v.len() == 8)
                {
                    let values = values
                        .iter()
                        .map(|v| v.as_f64())
                        .collect::<Option<Vec<_>>>();
                    if let Some(v) = values {
                        let a = inv.to_cols_array_2d();
                        let t = m.w_axis.truncate();
                        let mut out = v.clone();
                        for b in [0, 4] {
                            let gp = DVec3::new(
                                a[0][0] * v[b] + a[0][1] * v[b + 1] + a[0][2] * v[b + 2],
                                a[1][0] * v[b] + a[1][1] * v[b + 1] + a[1][2] * v[b + 2],
                                a[2][0] * v[b] + a[2][1] * v[b + 1] + a[2][2] * v[b + 2],
                            );
                            out[b] = gp.x;
                            out[b + 1] = gp.y;
                            out[b + 2] = gp.z;
                            out[b + 3] = v[b + 3] - (gp.x * t.x + gp.y * t.y + gp.z * t.z);
                        }
                        texture.insert("uvw".into(), serde_json::json!(out));
                    }
                }
            }
        }
    }
    for e in &src.edges {
        let a = vertex(
            &mut mesh,
            &mut registry,
            mapped(src.vertices[e.v0].position),
        )?;
        let b = vertex(
            &mut mesh,
            &mut registry,
            mapped(src.vertices[e.v1].position),
        )?;
        if let Some(id) = mesh.add_edge(a, b) {
            mesh.edges[id].soft = e.soft;
            mesh.edges[id].curve_id = e.curve_id;
        }
    }
    mesh.rebuild_spatial_index();
    Ok(mesh)
}
fn group_index<'a>(groups: &'a [Group], index: &mut std::collections::HashMap<Uuid, &'a Group>) {
    for g in groups {
        index.insert(g.history_id, g);
        group_index(&g.children, index)
    }
}
fn apply_poses(groups: &mut [Group], poses: &std::collections::HashMap<Uuid, DMat4>) {
    for g in groups {
        if let Some(pose) = poses.get(&g.history_id) {
            g.transform = *pose;
        }
        apply_poses(&mut g.children, poses)
    }
}
impl Command {
    pub fn prepare(scene: &Scene, target: Uuid, frame: DMat4) -> Result<Self, String> {
        let g = find(&scene.groups, target).ok_or("Axes target no longer exists")?;
        if !frame.is_finite() {
            return Err("Axes frame is nonfinite".into());
        }
        inverse(frame)?;
        let mut c = Self {
            target,
            frame,
            old_axes: g.native_document.get("axes").cloned(),
            delta: None,
            instances: vec![],
            undo: false,
        };
        if instance(g) {
            let d = multiply(inverse(frame)?, g.transform);
            inverse(d)?;
            c.delta = Some(d);
            fn visit(groups: &[Group], target: &Group, ids: &mut Vec<Uuid>) {
                for g in groups {
                    if (g.history_id == target.history_id
                        || target.mesh_share.is_some() && g.mesh_share == target.mesh_share)
                        && instance(g)
                    {
                        ids.push(g.history_id)
                    }
                    visit(&g.children, target, ids)
                }
            }
            visit(&scene.groups, g, &mut c.instances);
        }
        Ok(c)
    }
    pub fn apply(&self, scene: &mut Scene) -> Result<(), String> {
        let g = find(&scene.groups, self.target).ok_or("Axes target no longer exists")?;
        if self.delta.is_none() {
            let value = if self.undo {
                self.old_axes.clone()
            } else {
                Some(serde_json::json!(self.frame.to_cols_array()))
            };
            let g = find_mut(&mut scene.groups, self.target).unwrap();
            if !g.native_document.is_object() {
                g.native_document = serde_json::json!({});
            }
            if let Some(v) = value {
                g.native_document["axes"] = v;
            } else {
                g.native_document.as_object_mut().unwrap().remove("axes");
            }
            scene.bump_version();
            return Ok(());
        }
        let d = self.delta.unwrap();
        let (mesh_matrix, placement_matrix) = if self.undo {
            (inverse(d)?, d)
        } else {
            (d, inverse(d)?)
        };
        let mesh = transformed(&g.mesh, mesh_matrix)?;
        let share = g.mesh_share;
        let definition = g.component_definition;
        let mut index = std::collections::HashMap::new();
        group_index(&scene.groups, &mut index);
        let direct_children: std::collections::HashSet<_> =
            g.children.iter().map(|c| c.history_id).collect();
        let placements: Vec<_> = self
            .instances
            .iter()
            .filter(|id| index.contains_key(id))
            .map(|id| {
                let g = index.get(id).ok_or("Axes instance no longer exists")?;
                let base = if direct_children.contains(id) && instance(g) {
                    multiply(mesh_matrix, g.transform)
                } else {
                    g.transform
                };
                let m = multiply(base, placement_matrix);
                if !m.is_finite() {
                    return Err("Axes placement overflow".into());
                }
                Ok((*id, m))
            })
            .collect::<Result<_, String>>()?;
        let children: Vec<_> = g
            .children
            .iter()
            .filter(|g| instance(g))
            .map(|g| {
                let m = multiply(mesh_matrix, g.transform);
                if !m.is_finite() {
                    return Err("Axes child placement overflow".into());
                }
                Ok((g.history_id, m))
            })
            .collect::<Result<_, String>>()?;
        let mut poses: std::collections::HashMap<Uuid, DMat4> = children.into_iter().collect();
        poses.extend(placements);
        drop(index);
        if let Some(id) = share {
            scene.publish_mesh_share(id, mesh)?;
        } else {
            find_mut(&mut scene.groups, self.target).unwrap().mesh = mesh;
        }
        apply_poses(&mut scene.groups, &poses);
        if let Some(id) = definition {
            let target = find(&scene.groups, self.target).unwrap();
            let mesh = target.mesh.clone();
            let children = target.children.clone();
            if let Some(def) = scene.component_definitions.get_mut(&id) {
                def.mesh = mesh;
                def.children = children;
            }
        }
        if share.is_none() {
            scene.bump_version();
        }
        Ok(())
    }
    pub fn inverse(&self) -> Self {
        let mut c = self.clone();
        c.undo = !c.undo;
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_cancel_metadata_and_refusal() {
        let mut s = Scene::new();
        let mut g = Group::new("Classic");
        g.native_document = serde_json::json!({"opaque":{"keep":7}});
        let id = g.history_id;
        s.groups.push(g);
        let before = s.clone();
        assert!(Command::prepare(&s, id, DMat4::ZERO).is_err());
        assert_eq!(s, before);
        let c = Command::prepare(&s, id, DMat4::from_translation(DVec3::new(2., 3., 4.))).unwrap();
        c.apply(&mut s).unwrap();
        assert_eq!(s.groups[0].mesh, before.groups[0].mesh);
        assert_eq!(s.groups[0].transform, DMat4::IDENTITY);
        assert_eq!(
            s.groups[0].axes_matrix().unwrap().w_axis.truncate(),
            DVec3::new(2., 3., 4.)
        );
        c.inverse().apply(&mut s).unwrap();
        assert_eq!(
            s.groups[0].native_document,
            before.groups[0].native_document
        );
        let c = Command::prepare(&s, id, DMat4::IDENTITY).unwrap();
        s.groups.clear();
        let old = s.clone();
        assert!(c.apply(&mut s).is_err());
        assert_eq!(s, old);
    }
}

/// Compensate genuine instances parked outside the active edit view. Active
/// identities are replaced from that view at Finish, so never transform twice.
pub fn compensate_parked(
    command: &Command,
    active: &Scene,
    parked: &mut Scene,
) -> Result<(), String> {
    let Some(d) = command.delta else {
        return Ok(());
    };
    let target = find(&active.groups, command.target).ok_or("Axes active target is missing")?;
    let Some(share) = target.mesh_share else {
        return Ok(());
    };
    if !parked.mesh_shares.contains_key(&share) {
        return Err("Axes parked shared owner is missing".into());
    }
    let mesh = active
        .mesh_shares
        .get(&share)
        .ok_or("Axes active shared owner is missing")?
        .clone();
    let placement = if command.undo { d } else { inverse(d)? };
    let mut ids = std::collections::HashSet::new();
    fn collect(gs: &[Group], ids: &mut std::collections::HashSet<Uuid>) {
        for g in gs {
            ids.insert(g.history_id);
            collect(&g.children, ids)
        }
    }
    collect(&active.groups, &mut ids);
    let mut changes = Vec::new();
    fn prepare(
        gs: &[Group],
        share: Uuid,
        ids: &std::collections::HashSet<Uuid>,
        m: DMat4,
        out: &mut Vec<(Uuid, DMat4)>,
    ) -> Result<(), String> {
        for g in gs {
            if g.mesh_share == Some(share) && instance(g) && !ids.contains(&g.history_id) {
                let pose = multiply(g.transform, m);
                if !pose.is_finite() {
                    return Err("Axes parked placement overflow".into());
                }
                out.push((g.history_id, pose));
            }
            prepare(&g.children, share, ids, m, out)?;
        }
        Ok(())
    }
    prepare(&parked.groups, share, &ids, placement, &mut changes)?;
    let poses = changes.into_iter().collect();
    parked.publish_mesh_share(share, mesh)?;
    apply_poses(&mut parked.groups, &poses);
    Ok(())
}

pub fn extend_parked_instances(command: &mut Command, active: &Scene, parked: &Scene) {
    let Some(target) = find(&active.groups, command.target) else {
        return;
    };
    let Some(share) = target.mesh_share else {
        return;
    };
    fn visit(
        gs: &[Group],
        share: Uuid,
        ids: &mut Vec<Uuid>,
        seen: &mut std::collections::HashSet<Uuid>,
    ) {
        for g in gs {
            if g.mesh_share == Some(share) && instance(g) && seen.insert(g.history_id) {
                ids.push(g.history_id)
            }
            visit(&g.children, share, ids, seen)
        }
    }
    let mut seen = command.instances.iter().copied().collect();
    visit(&parked.groups, share, &mut command.instances, &mut seen);
}
