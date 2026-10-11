//! Immutable compact soft-edge planes, shared by prototype. One eye transform
//! per placement; unchanged camera/publication borrows accepted native endpoints.
use glam::{DMat4, DVec3, Mat4};
use ingetrazo_core::{groups::Group, mesh::Mesh, scene::Scene};
use std::{collections::HashMap, sync::Arc};
const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug)]
struct Edge {
    points: [[f32; 3]; 2],
    normals: [DVec3; 2],
    anchors: [[f32; 3]; 2],
    single: bool,
}
#[derive(Debug)]
struct Placement {
    edges: Arc<Vec<Edge>>,
    world: DMat4,
    inverse: DMat4,
}
#[derive(Debug, Default)]
pub(crate) struct Profiles {
    placements: Vec<Placement>,
    bytes: usize,
}
impl Profiles {
    pub fn retained_bytes(&self) -> usize {
        self.bytes + self.placements.capacity() * std::mem::size_of::<Placement>()
    }
    pub fn build(scene: &Scene, suppressed: &[usize]) -> Result<Self, String> {
        let mut out = Self::default();
        let mut prototypes = HashMap::new();
        out.append(
            &scene.mesh,
            Mat4::IDENTITY,
            scene,
            suppressed,
            None,
            &mut prototypes,
        )?;
        for g in &scene.groups {
            out.walk(g, Mat4::IDENTITY, scene, 0, &mut prototypes)?;
        }
        Ok(out)
    }
    fn walk(
        &mut self,
        g: &Group,
        parent: Mat4,
        scene: &Scene,
        depth: usize,
        prototypes: &mut HashMap<u128, Arc<Vec<Edge>>>,
    ) -> Result<(), String> {
        if depth > 128 {
            return Err("Metal profiles nesting exceeds128".into());
        }
        if g.hidden || !scene.is_layer_visible(g.layer.as_deref()) {
            return Ok(());
        }
        let world = parent * g.transform.as_mat4();
        if !g.billboard {
            self.append(
                &g.mesh,
                world,
                scene,
                &[],
                Some(g.mesh_share.unwrap_or(g.id).as_u128()),
                prototypes,
            )?;
        }
        for child in &g.children {
            self.walk(child, world, scene, depth + 1, prototypes)?;
        }
        Ok(())
    }
    fn append(
        &mut self,
        mesh: &Mesh,
        world: Mat4,
        scene: &Scene,
        suppressed: &[usize],
        key: Option<u128>,
        prototypes: &mut HashMap<u128, Arc<Vec<Edge>>>,
    ) -> Result<(), String> {
        if !world.is_finite() {
            return Err("Metal profile transform nonfinite".into());
        }
        let inverse = world.inverse();
        if !inverse.is_finite() {
            return Err("Metal profile transform singular".into());
        }
        let cached = key.and_then(|key| prototypes.get(&key)).cloned();
        let edges = if let Some(edges) = cached {
            edges
        } else {
            let mut edges = Vec::new();
            for (id, e) in mesh.edges.iter().enumerate() {
                if !e.soft
                    || e.hidden
                    || e.faces.is_empty()
                    || (key.is_some() && e.faces.len() > 2)
                    || suppressed.binary_search(&id).is_ok()
                    || (key.is_none()
                        && !scene.is_layer_visible(
                            e.layer
                                .as_deref()
                                .or_else(|| e.attrs.get("layer").and_then(|v| v.as_str())),
                        ))
                {
                    continue;
                }
                let point = |v: usize| -> Result<[f32; 3], String> {
                    let p = mesh
                        .vertices
                        .get(v)
                        .ok_or("Metal profile vertex invalid")?
                        .position
                        .as_vec3();
                    if !p.is_finite() {
                        return Err("Metal profile position overflow".into());
                    }
                    Ok(p.to_array())
                };
                let plane = |id: usize| -> Result<(DVec3, [f32; 3]), String> {
                    let f = mesh
                        .faces
                        .get(id)
                        .filter(|f| f.vertices.len() >= 3)
                        .ok_or("Metal profile face invalid")?;
                    // Group prototype chunks use actual Face.normal/centroid;
                    // loose silhouettes deliberately use first-three planes.
                    if key.is_some() {
                        let mut normal = DVec3::ZERO;
                        let mut center = DVec3::ZERO;
                        for i in 0..f.vertices.len() {
                            let a = DVec3::from_array(point(f.vertices[i])?.map(f64::from));
                            let b = DVec3::from_array(
                                point(f.vertices[(i + 1) % f.vertices.len()])?.map(f64::from),
                            );
                            normal.x += (a.y - b.y) * (a.z + b.z);
                            normal.y += (a.z - b.z) * (a.x + b.x);
                            normal.z += (a.x - b.x) * (a.y + b.y);
                            center += a;
                        }
                        let n = normal.as_vec3();
                        let length = n.length();
                        let n = if length < 1e-9 {
                            glam::Vec3::Z
                        } else {
                            n / length
                        };
                        return Ok((
                            n.as_dvec3(),
                            (center / (f.vertices.len() as f64)).as_vec3().to_array(),
                        ));
                    }
                    let t = [
                        point(f.vertices[0])?,
                        point(f.vertices[1])?,
                        point(f.vertices[2])?,
                    ]
                    .map(|p| DVec3::from_array(p.map(f64::from)));
                    Ok(((t[1] - t[0]).cross(t[2] - t[0]), t[0].as_vec3().to_array()))
                };
                let a = plane(e.faces[0])?;
                let b = if e.faces.len() == 2 {
                    plane(e.faces[1])?
                } else {
                    a
                };
                if edges.len() == edges.capacity() {
                    let available = MAX_BYTES
                        .checked_sub(self.retained_bytes())
                        .ok_or("Metal profile byte budget")?
                        / std::mem::size_of::<Edge>();
                    let next = edges.capacity().max(16).saturating_mul(2).min(available);
                    if next <= edges.len() {
                        return Err("Metal profile records exceed64MiB".into());
                    }
                    edges
                        .try_reserve_exact(next - edges.len())
                        .map_err(|_| "Metal profile record allocation refused")?;
                }
                edges.push(Edge {
                    points: [point(e.v0)?, point(e.v1)?],
                    normals: [a.0, b.0],
                    anchors: [a.1, b.1],
                    single: e.faces.len() != 2,
                });
            }
            self.bytes = self
                .bytes
                .checked_add(edges.capacity() * std::mem::size_of::<Edge>())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or("Metal profile records exceed64MiB")?;
            let edges = Arc::new(edges);
            if let Some(key) = key {
                prototypes
                    .try_reserve(1)
                    .map_err(|_| "Metal profile identity allocation refused")?;
                prototypes.insert(key, edges.clone());
            }
            edges
        };
        if edges.is_empty() {
            return Ok(());
        }
        let bytes = self
            .retained_bytes()
            .checked_add(std::mem::size_of::<Placement>())
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Metal profile placements exceed64MiB")?;
        let _ = bytes;
        if self.placements.len() == self.placements.capacity() {
            let max = (MAX_BYTES - self.bytes) / std::mem::size_of::<Placement>();
            let next = self.placements.capacity().max(8).saturating_mul(2).min(max);
            if next <= self.placements.len() {
                return Err("Metal profile placement budget exhausted".into());
            }
            self.placements
                .try_reserve_exact(next - self.placements.len())
                .map_err(|_| "Metal profile placement allocation refused")?;
        }
        self.placements.push(Placement {
            edges,
            world: world.as_dmat4(),
            inverse: inverse.as_dmat4(),
        });
        Ok(())
    }
    pub fn selection(&self, eye: DVec3, out: &mut Vec<(usize, usize)>) -> Result<(), String> {
        if !eye.is_finite() {
            return Err("Metal profile eye nonfinite".into());
        }
        let count = self
            .placements
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.edges.len()))
            .filter(|n| {
                n.checked_mul(std::mem::size_of::<(usize, usize)>())
                    .is_some_and(|bytes| bytes <= MAX_BYTES)
            })
            .ok_or("Metal profile selection exceeds64MiB")?;
        out.clear();
        out.try_reserve_exact(count)
            .map_err(|_| "Metal profile selection allocation refused")?;
        for (placement, p) in self.placements.iter().enumerate() {
            let eye = p.inverse.transform_point3(eye);
            for (edge, e) in p.edges.iter().enumerate() {
                let side = |i: usize| {
                    e.normals[i].dot(DVec3::from_array(e.anchors[i].map(f64::from)) - eye)
                };
                if e.single || (side(0) < 0.) != (side(1) < 0.) {
                    out.push((placement, edge));
                }
            }
        }
        Ok(())
    }
    pub fn endpoints(&self, selection: &[(usize, usize)]) -> Result<Vec<[f32; 3]>, String> {
        let capacity = selection
            .len()
            .checked_mul(2)
            .filter(|n| n.checked_mul(12).is_some_and(|bytes| bytes <= MAX_BYTES))
            .ok_or("Metal profile endpoints exceed64MiB")?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(capacity)
            .map_err(|_| "Metal profile endpoints allocation refused")?;
        for &(placement, edge) in selection {
            let p = self
                .placements
                .get(placement)
                .ok_or("Metal profile placement identity invalid")?;
            let e = p
                .edges
                .get(edge)
                .ok_or("Metal profile edge identity invalid")?;
            for v in e.points {
                let v = p
                    .world
                    .transform_point3(DVec3::from_array(v.map(f64::from)))
                    .as_vec3();
                if !v.is_finite() {
                    return Err("Metal profile world endpoint overflow".into());
                }
                positions.push(v.to_array());
            }
        }
        Ok(positions)
    }
    #[cfg(test)]
    pub fn select(&self, eye: DVec3) -> Result<Vec<[f32; 3]>, String> {
        let mut selected = Vec::new();
        self.selection(eye, &mut selected)?;
        self.endpoints(&selected)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_group_newell_centroid_and_nonmanifold_policy() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/metal-defaults/source.json"
        ))
        .unwrap();
        let row = &fixture["group"];
        let mut mesh = Mesh::new();
        for v in row["vertices"].as_array().unwrap() {
            mesh.add_vertex(DVec3::from_array(std::array::from_fn(|i| {
                v[i].as_f64().unwrap()
            })));
        }
        for f in row["faces"].as_array().unwrap() {
            mesh.add_face(
                f.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize)
                    .collect(),
                vec![],
            )
            .unwrap();
        }
        let edge = mesh.edges.iter().position(|e| e.faces.len() == 2).unwrap();
        mesh.edges[edge].soft = true;
        let mut scene = Scene::new();
        scene
            .groups
            .push(Group::with_mesh("collinear prefix", mesh));
        let packet = Profiles::build(&scene, &[]).unwrap();
        let record = &packet.placements[0].edges[0];
        for i in 0..2 {
            let expected = &row["planes"][i];
            assert_eq!(
                record.normals[i].to_array(),
                std::array::from_fn(|c| expected["normal"][c].as_f64().unwrap())
            );
            assert_eq!(
                record.anchors[i],
                std::array::from_fn(|c| expected["centroid"][c].as_f64().unwrap() as f32)
            );
        }
        scene.groups[0].mesh.edges[edge].faces.push(0);
        assert!(Profiles::build(&scene, &[]).unwrap().placements.is_empty());
    }
    #[test]
    fn repeated_prototype_shares_planes_and_keeps_one_pose_record() {
        let mut scene = Scene::new();
        let mut mesh = Mesh::new();
        let v = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(v.to_vec(), vec![]).unwrap();
        mesh.edges[0].soft = true;
        let mut group = Group::with_mesh("prototype", mesh);
        group.mesh_share = Some(group.id);
        let mut instance = group.duplicate_instance();
        instance.transform = DMat4::from_translation(DVec3::new(2., 3., 4.));
        scene.groups = vec![group, instance];
        let packet = Profiles::build(&scene, &[]).unwrap();
        assert_eq!(packet.placements.len(), 2);
        assert!(Arc::ptr_eq(
            &packet.placements[0].edges,
            &packet.placements[1].edges
        ));
        assert!(std::mem::size_of::<Edge>() <= 112);
        let rows = packet.select(DVec3::new(0., 0., 10.)).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[2], [2., 3., 4.]);
        assert_eq!(rows[3], [3., 3., 4.]);
    }
    #[test]
    fn original_folded_face_camera_selection() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/metal-defaults/source.json"
        ))
        .unwrap();
        let mut scene = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z].map(|p| scene.mesh.add_vertex(p));
        scene
            .mesh
            .add_face(vec![ids[0], ids[1], ids[2]], vec![])
            .unwrap();
        scene
            .mesh
            .add_face(vec![ids[1], ids[0], ids[3]], vec![])
            .unwrap();
        for e in &mut scene.mesh.edges {
            e.soft = e.faces.len() == 2;
        }
        let profiles = Profiles::build(&scene, &[]).unwrap();
        for row in fixture["profiles"].as_array().unwrap() {
            let eye = DVec3::from_array(std::array::from_fn(|i| row["eye"][i].as_f64().unwrap()));
            let actual = profiles.select(eye).unwrap();
            let expected: Vec<f32> = row["positions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            assert_eq!(bytemuck::cast_slice::<[f32; 3], f32>(&actual), expected);
        }
    }
    #[test]
    fn single_face_soft_visibility_and_camera_change() {
        let mut s = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| s.mesh.add_vertex(p));
        s.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        s.mesh.edges[0].soft = true;
        let p = Profiles::build(&s, &[]).unwrap();
        assert_eq!(p.select(DVec3::Z).unwrap().len(), 2);
        assert_eq!(p.select(-DVec3::Z).unwrap().len(), 2);
        assert!(Profiles::build(&s, &[0])
            .unwrap()
            .select(DVec3::Z)
            .unwrap()
            .is_empty());
        s.mesh.edges[0].hidden = true;
        assert!(Profiles::build(&s, &[])
            .unwrap()
            .select(DVec3::Z)
            .unwrap()
            .is_empty());
        assert!(p.select(DVec3::NAN).is_err());
    }
}
