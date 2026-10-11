// SPDX-License-Identifier: GPL-3.0-or-later
//! Grab-time Scale ownership. Pointer motion touches only captured targets.
use super::grips::{Frame, GripBox};
use glam::{DMat4, DVec3, Vec3};
use ingetrazo_core::{
    groups::Group,
    image_plane::{ImagePlane, ScaleMatrix},
    mesh::Mesh,
    scene::{Scene, SelectionItem},
    snap_screen::SnapExclusions,
};
use std::{
    collections::{BTreeSet, HashMap},
    hash::{Hash, Hasher},
};

#[derive(Debug, Clone)]
struct Vertex {
    index: usize,
    identity: u128,
    position: DVec3,
}
#[derive(Debug, Clone)]
struct GroupSeed {
    index: usize,
    identity: u128,
    transform: DMat4,
    placed: bool,
    vertices: Vec<Vertex>,
    axes: Option<serde_json::Value>,
    textures: Vec<(usize, Option<serde_json::Value>)>,
    share: Option<uuid::Uuid>,
}
#[derive(Debug, Clone, Default)]
pub struct Targets {
    vertices: Vec<Vertex>,
    groups: Vec<GroupSeed>,
    pub images: Vec<ImagePlane>,
    image_epoch: u64,
    image_indices: HashMap<u128, usize>,
    pub exclusions: SnapExclusions,
    shared: Vec<SharedSeed>,
}
// Index paths and runtime IDs are frozen at grab: no document walk on motion.
#[derive(Debug, Clone)]
enum Owner {
    Registry(uuid::Uuid),
    Definition(uuid::Uuid),
    Groups {
        definition: Option<uuid::Uuid>,
        path: Vec<(usize, u128)>,
    },
}
#[derive(Debug, Clone)]
struct SharedOwner {
    owner: Owner,
    vertices: Vec<Vertex>,
    textures: Vec<(usize, Option<serde_json::Value>)>,
    payload: usize,
    uv_payloads: Vec<Option<usize>>,
}
#[derive(Debug, Clone)]
struct SharedCoordinates {
    leader: usize,
    mapped: Vec<DVec3>,
}
#[derive(Debug, Clone)]
struct SharedUv {
    original: [f64; 8],
    mapped: [f64; 8],
}
#[derive(Debug, Clone)]
struct SharedSeed {
    token: uuid::Uuid,
    commands: usize,
    owners: Vec<SharedOwner>,
    coordinates: Vec<SharedCoordinates>,
    uvs: Vec<SharedUv>,
}
fn textures(mesh: &Mesh) -> Vec<(usize, Option<serde_json::Value>)> {
    mesh.faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.attrs.get("texture").and_then(|t| t.get("uvw")).is_some())
        .map(|(i, f)| (i, f.attrs.get("texture").cloned()))
        .collect()
}
fn owner_mesh<'a>(scene: &'a mut Scene, owner: &Owner, token: uuid::Uuid) -> Option<&'a mut Mesh> {
    match owner {
        Owner::Registry(id) => scene.mesh_shares.get_mut(id),
        Owner::Definition(id) => scene
            .component_definitions
            .get_mut(id)
            .filter(|d| d.mesh_share == Some(token))
            .map(|d| &mut d.mesh),
        Owner::Groups { definition, path } => {
            let mut groups = if let Some(id) = definition {
                &mut scene.component_definitions.get_mut(id)?.children
            } else {
                &mut scene.groups
            };
            for (step, (index, identity)) in path.iter().enumerate() {
                let g = groups
                    .get_mut(*index)
                    .filter(|g| g.history_id.as_u128() == *identity)?;
                if step + 1 == path.len() {
                    return (g.mesh_share == Some(token)).then_some(&mut g.mesh);
                }
                groups = &mut g.children;
            }
            None
        }
    }
}
fn capture_share(scene: &Scene, token: uuid::Uuid, commands: usize) -> SharedSeed {
    fn record(owners: &mut Vec<SharedOwner>, owner: Owner, m: &Mesh) {
        owners.push(SharedOwner {
            owner,
            vertices: (0..m.vertices.len()).map(|i| vertex(m, i)).collect(),
            textures: textures(m),
            payload: 0,
            uv_payloads: Vec::new(),
        });
    }
    fn walk(
        groups: &[Group],
        def: Option<uuid::Uuid>,
        path: &mut Vec<(usize, u128)>,
        token: uuid::Uuid,
        owners: &mut Vec<SharedOwner>,
    ) {
        for (i, g) in groups.iter().enumerate() {
            path.push((i, g.history_id.as_u128()));
            if g.mesh_share == Some(token) {
                record(
                    owners,
                    Owner::Groups {
                        definition: def,
                        path: path.clone(),
                    },
                    &g.mesh,
                );
            }
            walk(&g.children, def, path, token, owners);
            path.pop();
        }
    }
    let mut owners = Vec::new();
    if let Some(m) = scene.mesh_shares.get(&token) {
        record(&mut owners, Owner::Registry(token), m);
    }
    walk(&scene.groups, None, &mut Vec::new(), token, &mut owners);
    for (id, d) in &scene.component_definitions {
        if d.mesh_share == Some(token) {
            record(&mut owners, Owner::Definition(*id), &d.mesh);
        }
        walk(&d.children, Some(*id), &mut Vec::new(), token, &mut owners);
    }
    // Hash only accelerates cold grouping; bytewise coordinate comparison
    // resolves collisions and never equates divergent owners by token alone.
    let mut buckets: HashMap<u64, Vec<usize>> = HashMap::new();
    let mut coordinates: Vec<SharedCoordinates> = Vec::new();
    let mut uv_lookup: HashMap<[u64; 8], usize> = HashMap::new();
    let mut uvs: Vec<SharedUv> = Vec::new();
    for index in 0..owners.len() {
        let vertices = &owners[index].vertices;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        vertices.len().hash(&mut hash);
        for v in vertices {
            v.position.to_array().map(f64::to_bits).hash(&mut hash);
        }
        let bucket = buckets.entry(hash.finish()).or_default();
        let same = bucket.iter().copied().find(|&id| {
            let leader = &owners[coordinates[id].leader].vertices;
            leader.len() == vertices.len()
                && leader.iter().zip(vertices).all(|(a, b)| {
                    a.position.to_array().map(f64::to_bits)
                        == b.position.to_array().map(f64::to_bits)
                })
        });
        let payload = same.unwrap_or_else(|| {
            let id = coordinates.len();
            coordinates.push(SharedCoordinates {
                leader: index,
                mapped: vec![DVec3::ZERO; vertices.len()],
            });
            bucket.push(id);
            id
        });
        owners[index].payload = payload;
        owners[index].uv_payloads = owners[index]
            .textures
            .iter()
            .map(|(_, texture)| {
                let values = uv_values(texture.as_ref()?)?;
                let key = values.map(f64::to_bits);
                Some(*uv_lookup.entry(key).or_insert_with(|| {
                    let id = uvs.len();
                    uvs.push(SharedUv {
                        original: values,
                        mapped: values,
                    });
                    id
                }))
            })
            .collect();
    }
    SharedSeed {
        token,
        commands,
        owners,
        coordinates,
        uvs,
    }
}
fn selected_vertices(scene: &Scene) -> BTreeSet<usize> {
    let mut result = BTreeSet::new();
    for item in &scene.selection {
        match *item {
            SelectionItem::Vertex(i) if i < scene.mesh.vertices.len() => {
                result.insert(i);
            }
            SelectionItem::Edge(i) => {
                if let Some(e) = scene.mesh.edges.get(i) {
                    result.extend([e.v0, e.v1]);
                }
            }
            SelectionItem::Face(i) => {
                if let Some(f) = scene.mesh.faces.get(i) {
                    result.extend(&f.vertices);
                }
            }
            _ => (),
        }
    }
    result
}
fn vertex(mesh: &Mesh, index: usize) -> Vertex {
    let v = &mesh.vertices[index];
    Vertex {
        index,
        identity: v.uid.as_u128(),
        position: v.position,
    }
}
fn is_placed(g: &Group) -> bool {
    g.transform != DMat4::IDENTITY
        || g.component_definition.is_some()
        || g.native_document.get("xform").is_some_and(|v| !v.is_null())
}
fn qt_dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn qt_normal(v: Vec3) -> Vec3 {
    let n = (v.x as f64).powi(2) + (v.y as f64).powi(2) + (v.z as f64).powi(2);
    if (n - 1.).abs() <= 1e-12 * n.min(1.) {
        v
    } else if n.abs() <= 1e-12 {
        Vec3::ZERO
    } else {
        let n = n.sqrt();
        Vec3::new(
            (v.x as f64 / n) as f32,
            (v.y as f64 / n) as f32,
            (v.z as f64 / n) as f32,
        )
    }
}
fn compose(a: DMat4, b: DMat4) -> DMat4 {
    let a = a.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let b = b.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let mut c = [[0.; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            c[col][row] = (a[0][row] * b[col][0]
                + a[1][row] * b[col][1]
                + a[2][row] * b[col][2]
                + a[3][row] * b[col][3]) as f64;
        }
    }
    DMat4::from_cols_array_2d(&c)
}
fn point(m: DMat4, p: DVec3) -> DVec3 {
    let c = m.to_cols_array_2d().map(|c| c.map(|v| v as f32));
    let p = p.as_vec3();
    Vec3::from_array(std::array::from_fn(|r| {
        p.x * c[0][r] + p.y * c[1][r] + p.z * c[2][r] + c[3][r]
    }))
    .as_dvec3()
}
fn frame(scene: &Scene) -> Option<Frame> {
    let group = if scene.selection.len() == 1 {
        scene.selection.iter().next().and_then(|s| match *s {
            SelectionItem::Group(i) => scene.groups.get(i),
            _ => None,
        })
    } else {
        None
    };
    let own = group.and_then(|g| {
        if is_placed(g) {
            Some(compose(
                g.transform,
                g.axes_matrix().unwrap_or(DMat4::IDENTITY),
            ))
        } else {
            g.axes_matrix()
        }
    });
    let m = own.or(scene.drawing_frame)?;
    let mut x = m.x_axis.truncate().as_vec3();
    let mut y = m.y_axis.truncate().as_vec3();
    let z = m.z_axis.truncate().as_vec3();
    if x.length() < 1e-12 {
        x = Vec3::X;
    }
    x = qt_normal(x);
    y -= x * qt_dot(y, x);
    if y.length() < 1e-12 {
        y = z.cross(x);
    }
    y = qt_normal(y);
    let mut zz = qt_normal(x.cross(y));
    if qt_dot(zz, z) < 0. {
        zz = -zz;
    }
    if [x, y, zz] == [Vec3::X, Vec3::Y, Vec3::Z] {
        return None;
    }
    Some(Frame {
        origin: m.w_axis.truncate().as_vec3().as_dvec3(),
        axes: [x.as_dvec3(), y.as_dvec3(), zz.as_dvec3()],
    })
}
pub fn selection_box(scene: &Scene) -> Option<GripBox> {
    let frame = frame(scene);
    let mut basis = GripBox::new(DVec3::ZERO, DVec3::ONE, frame)?;
    let mut low = DVec3::splat(f64::INFINITY);
    let mut high = DVec3::splat(f64::NEG_INFINITY);
    let mut absorb = |p: DVec3| {
        let p = basis.to_local(p);
        low = low.min(p);
        high = high.max(p);
    };
    for i in selected_vertices(scene) {
        absorb(scene.mesh.vertices[i].position);
    }
    fn group(g: &Group, parent: DMat4, absorb: &mut impl FnMut(DVec3)) {
        let m = compose(parent, g.transform);
        for v in &g.mesh.vertices {
            absorb(point(m, v.position));
        }
        for child in &g.children {
            group(child, m, absorb);
        }
    }
    for item in &scene.selection {
        match *item {
            SelectionItem::Group(i) => {
                if let Some(g) = scene.groups.get(i) {
                    group(g, DMat4::IDENTITY, &mut absorb);
                }
            }
            SelectionItem::ImagePlane(i) => {
                if let Some(im) = scene.image_planes.get(i) {
                    for p in im.corners() {
                        absorb(p);
                    }
                }
            }
            _ => (),
        }
    }
    basis = GripBox::new(low, high, frame)?;
    Some(basis)
}
impl Targets {
    pub fn capture(scene: &Scene) -> Self {
        let mut result = Self::default();
        result.vertices = selected_vertices(scene)
            .into_iter()
            .map(|i| vertex(&scene.mesh, i))
            .collect();
        for v in &result.vertices {
            result
                .exclusions
                .loose_edges
                .extend(&scene.mesh.vertices[v.index].edges);
        }
        for item in &scene.selection {
            match *item {
                SelectionItem::Group(i) => {
                    if let Some(g) = scene.groups.get(i) {
                        let placed = is_placed(g);
                        result.exclusions.groups.insert(g.id);
                        result.groups.push(GroupSeed {
                            index: i,
                            identity: g.history_id.as_u128(),
                            transform: g.transform,
                            placed,
                            vertices: if placed || g.mesh_share.is_some() {
                                Vec::new()
                            } else {
                                (0..g.mesh.vertices.len())
                                    .map(|i| vertex(&g.mesh, i))
                                    .collect()
                            },
                            share: (!placed).then_some(g.mesh_share).flatten(),
                            axes: g.native_document.get("axes").cloned(),
                            textures: if placed || g.mesh_share.is_some() {
                                Vec::new()
                            } else {
                                g.mesh
                                    .faces
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, f)| {
                                        f.attrs.get("texture").and_then(|t| t.get("uvw")).is_some()
                                    })
                                    .map(|(i, f)| (i, f.attrs.get("texture").cloned()))
                                    .collect()
                            },
                        });
                    }
                }
                SelectionItem::ImagePlane(i) => {
                    if let Some(im) = scene.image_planes.get(i) {
                        result.images.push(im.clone());
                    }
                }
                _ => (),
            }
        }
        let mut tokens = HashMap::new();
        for seed in &result.groups {
            if let Some(token) = seed.share {
                *tokens.entry(token).or_insert(0usize) += 1;
            }
        }
        result.shared = tokens
            .into_iter()
            .map(|(token, commands)| capture_share(scene, token, commands))
            .collect();
        result.refresh_images(scene);
        result
    }
    fn refresh_images(&mut self, scene: &Scene) {
        let epoch = scene.image_planes.membership_epoch();
        if epoch != self.image_epoch {
            self.image_indices.clear();
            self.image_indices.extend(
                scene
                    .image_planes
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.id.as_u128(), i)),
            );
            self.image_epoch = epoch;
        }
    }
    pub fn apply_cold_mesh(&self, mesh: &mut Mesh) {
        mesh.place_vertices(
            self.vertices
                .iter()
                .map(|v| (v.index, v.identity, v.position)),
        );
    }
    pub fn has_loose_vertices(&self) -> bool {
        !self.vertices.is_empty()
    }
    pub fn only_placements(&self) -> bool {
        self.images.is_empty()
            && self.vertices.is_empty()
            && !self.groups.is_empty()
            && self.groups.iter().all(|g| g.placed)
    }
    pub fn only_images(&self) -> bool {
        !self.images.is_empty() && self.vertices.is_empty() && self.groups.is_empty()
    }
    pub fn is_empty(&self) -> bool {
        self.images.is_empty() && self.vertices.is_empty() && self.groups.is_empty()
    }
    /// Source shared groups repeat one command per selected owner on commit.
    /// Admit the complete repeated mapping before touching any live owner.
    pub fn shared_commit_finite(&self, matrix: &ScaleMatrix) -> bool {
        self.shared.iter().all(|shared| {
            shared.coordinates.iter().all(|payload| {
                shared.owners[payload.leader].vertices.iter().all(|seed| {
                    let mut point = seed.position;
                    for _ in 0..shared.commands {
                        point = matrix.point(point);
                        if !point.is_finite() {
                            return false;
                        }
                    }
                    true
                })
            })
        })
    }
    /// The matrix is None for an exact grab-time restore. Commit metadata is
    /// derived from originals once; previews never reauthor texture maps/axes.
    pub fn apply(&mut self, scene: &mut Scene, matrix: Option<&ScaleMatrix>, commit: bool) {
        self.refresh_images(scene);
        let uv = commit.then(|| matrix.and_then(UvTransform::new)).flatten();
        for shared in &mut self.shared {
            let count = if commit { shared.commands } else { 1 };
            for payload in &mut shared.coordinates {
                for (mapped, seed) in payload
                    .mapped
                    .iter_mut()
                    .zip(&shared.owners[payload.leader].vertices)
                {
                    *mapped = seed.position;
                    if let Some(matrix) = matrix {
                        for _ in 0..count {
                            *mapped = matrix.point(*mapped);
                        }
                    }
                }
            }
            if let Some(uv) = &uv {
                for payload in &mut shared.uvs {
                    payload.mapped = payload.original;
                    for _ in 0..count {
                        uv.apply(&mut payload.mapped);
                    }
                }
            }
        }
        self.apply_with_indices(
            scene,
            matrix,
            commit,
            &self.image_indices,
            true,
            uv.as_ref(),
        );
    }
    pub fn apply_cold(&self, scene: &mut Scene, matrix: Option<&ScaleMatrix>, commit: bool) {
        let indices = scene
            .image_planes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_u128(), i))
            .collect();
        let uv = commit.then(|| matrix.and_then(UvTransform::new)).flatten();
        self.apply_with_indices(scene, matrix, commit, &indices, false, uv.as_ref());
    }
    fn apply_with_indices(
        &self,
        scene: &mut Scene,
        matrix: Option<&ScaleMatrix>,
        commit: bool,
        indices: &HashMap<u128, usize>,
        shared_prepared: bool,
        uv: Option<&UvTransform>,
    ) {
        let mut changed = false;
        fn map_vertices(
            mesh: &mut Mesh,
            vertices: &[Vertex],
            matrix: Option<&ScaleMatrix>,
        ) -> bool {
            mesh.place_vertices(vertices.iter().map(|seed| {
                (
                    seed.index,
                    seed.identity,
                    matrix.map_or(seed.position, |m| m.point(seed.position)),
                )
            }))
        }
        changed |= map_vertices(&mut scene.mesh, &self.vertices, matrix);
        for shared in &self.shared {
            let count = if commit { shared.commands } else { 1 };
            for owned in &shared.owners {
                let Some(mesh) = owner_mesh(scene, &owned.owner, shared.token) else {
                    continue;
                };
                changed |=
                    mesh.place_vertices(owned.vertices.iter().enumerate().map(|(i, seed)| {
                        let mut p = seed.position;
                        if shared_prepared {
                            p = shared.coordinates[owned.payload].mapped[i];
                        } else if let Some(m) = matrix {
                            for _ in 0..count {
                                p = m.point(p);
                            }
                        }
                        (seed.index, seed.identity, p)
                    }));
                if commit {
                    for ((id, t), payload) in owned.textures.iter().zip(&owned.uv_payloads) {
                        if let Some(f) = mesh.faces.get_mut(*id) {
                            if let Some(t) = t {
                                let mut t = t.clone();
                                if let Some(uv) = uv {
                                    if shared_prepared {
                                        if let Some(payload) = payload {
                                            t["uvw"] =
                                                serde_json::json!(shared.uvs[*payload].mapped);
                                        }
                                    } else {
                                        for _ in 0..count {
                                            remap_uv(&mut t, uv);
                                        }
                                    }
                                }
                                f.attrs.insert("texture".into(), t);
                            } else {
                                f.attrs.remove("texture");
                            }
                        }
                    }
                }
            }
        }
        for seed in &self.groups {
            let Some(g) = scene
                .groups
                .get_mut(seed.index)
                .filter(|g| g.history_id.as_u128() == seed.identity)
            else {
                continue;
            };
            if seed.placed {
                let transform = matrix.map_or(seed.transform, |m| m.compose(seed.transform));
                changed |= g.transform != transform;
                g.transform = transform;
            } else {
                if seed.share.is_none() {
                    changed |= map_vertices(&mut g.mesh, &seed.vertices, matrix);
                }
                if commit {
                    let axes = seed
                        .axes
                        .as_ref()
                        .and_then(|v| v.as_array())
                        .filter(|a| a.len() == 16)
                        .and_then(|a| a.iter().map(|v| v.as_f64()).collect::<Option<Vec<_>>>())
                        .map(|a| DMat4::from_cols_slice(&a));
                    if let Some(m) = matrix {
                        if !g.native_document.is_object() {
                            g.native_document = serde_json::json!({});
                        }
                        g.native_document["axes"] = serde_json::json!(
                            m.compose(axes.unwrap_or(DMat4::IDENTITY)).to_cols_array()
                        );
                    } else if let Some(object) = g.native_document.as_object_mut() {
                        if let Some(axes) = &seed.axes {
                            object.insert("axes".into(), axes.clone());
                        } else {
                            object.remove("axes");
                        }
                    }
                    for (i, texture) in seed.textures.iter().filter(|_| seed.share.is_none()) {
                        let Some(f) = g.mesh.faces.get_mut(*i) else {
                            continue;
                        };
                        if let Some(t) = texture {
                            let mut t = t.clone();
                            if let Some(uv) = uv {
                                remap_uv(&mut t, uv);
                            }
                            f.attrs.insert("texture".into(), t);
                        } else {
                            f.attrs.remove("texture");
                        }
                    }
                }
            }
        }
        for seed in &self.images {
            let frame = matrix.map_or([seed.origin, seed.u, seed.v], |m| m.frame(seed));
            if let Some(&index) = indices.get(&seed.id.as_u128()) {
                scene.image_planes.pose_mut(index).unwrap().set_frame(frame);
            } else {
                scene.image_objects.set_held_frame(seed, frame);
            }
        }
        if changed {
            scene.bump_version();
        }
    }
}
fn uv_values(texture: &serde_json::Value) -> Option<[f64; 8]> {
    let values = texture.get("uvw")?.as_array()?;
    if values.len() != 8 {
        return None;
    }
    let mut result = [0.; 8];
    for (out, value) in result.iter_mut().zip(values) {
        *out = value.as_f64()?;
    }
    Some(result)
}
struct UvTransform {
    inverse_transpose: DMat4,
    translation: DVec3,
}
impl UvTransform {
    fn new(matrix: &ScaleMatrix) -> Option<Self> {
        let m = matrix.matrix();
        let inverse = m.inverse().as_mat4().as_dmat4();
        inverse.is_finite().then(|| Self {
            inverse_transpose: inverse.transpose(),
            translation: m.w_axis.truncate(),
        })
    }
    fn apply(&self, values: &mut [f64; 8]) {
        for base in [0, 4] {
            let g = self.inverse_transpose.transform_vector3(DVec3::new(
                values[base],
                values[base + 1],
                values[base + 2],
            ));
            let c = values[base + 3] - g.dot(self.translation);
            values[base..base + 4].copy_from_slice(&[g.x, g.y, g.z, c]);
        }
    }
}
fn remap_uv(texture: &mut serde_json::Value, transform: &UvTransform) {
    if let Some(mut values) = uv_values(texture) {
        transform.apply(&mut values);
        texture["uvw"] = serde_json::json!(values);
    }
}

#[cfg(test)]
mod share_tests {
    use super::*;
    #[test]
    fn shared_cohorts_preserve_sequential_rounding_identity_and_owner_metadata() {
        let mut scene = Scene::new();
        let mut mesh = Mesh::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| mesh.add_vertex(p));
        mesh.add_face(ids.to_vec(), vec![]);
        mesh.faces[0].attrs.insert(
            "texture".into(),
            serde_json::json!({"path":"registry","uvw":[0.25,0,0,0.1,0,0.5,0,0.2],"vendor":7}),
        );
        let token = uuid::Uuid::new_v4();
        scene.register_mesh_share(token, mesh.clone()).unwrap();
        for i in 0..16 {
            let mut m = mesh.clone();
            for v in &mut m.vertices {
                v.uid = uuid::Uuid::new_v4();
            }
            if i == 14 {
                m.vertices[0].position.x = -0.;
            }
            if i == 15 {
                m.vertices[1].position.x = f64::from_bits(1f64.to_bits() + 1);
            }
            m.faces[0].attrs.get_mut("texture").unwrap()["path"] =
                serde_json::json!(format!("owner-{i}"));
            if i == 15 {
                m.faces[0].attrs.get_mut("texture").unwrap()["uvw"][0] = serde_json::json!(0.75);
            }
            let mut g = Group::with_mesh("owner", m);
            g.mesh_share = Some(token);
            scene.groups.push(g);
            scene.select(SelectionItem::Group(i));
        }
        let original = scene.clone();
        let mut targets = Targets::capture(&scene);
        assert_eq!(
            targets.shared[0].coordinates.len(),
            3,
            "Signed zero and one-bit differences stay separate"
        );
        assert_eq!(targets.shared[0].uvs.len(), 2);
        let capacities = targets.shared[0]
            .coordinates
            .iter()
            .map(|p| (p.mapped.as_ptr(), p.mapped.capacity()))
            .collect::<Vec<_>>();
        let matrix = ScaleMatrix::new(
            DVec3::new(0.125, -0.25, 0.),
            DVec3::new(1.05, 0.95, 1.),
            None,
        );
        targets.apply(&mut scene, Some(&matrix), true);
        for (g, old) in scene.groups.iter().zip(&original.groups) {
            for (v, seed) in g.mesh.vertices.iter().zip(&old.mesh.vertices) {
                let mut expected = seed.position;
                for _ in 0..16 {
                    expected = matrix.point(expected);
                }
                assert_eq!(
                    v.position.to_array().map(f64::to_bits),
                    expected.to_array().map(f64::to_bits)
                );
                assert_eq!(v.uid, seed.uid);
            }
            let mut expected = old.mesh.faces[0].attrs["texture"].clone();
            for _ in 0..16 {
                // Literal pre-optimization cold remap: independently recompute
                // the inverse and apply each coefficient step, never matrix power.
                let mut values = uv_values(&expected).unwrap();
                let m = matrix.matrix();
                let inv = m.inverse().as_mat4().as_dmat4();
                for base in [0, 4] {
                    let g = inv.transpose().transform_vector3(DVec3::new(
                        values[base],
                        values[base + 1],
                        values[base + 2],
                    ));
                    let c = values[base + 3] - g.dot(m.w_axis.truncate());
                    values[base..base + 4].copy_from_slice(&[g.x, g.y, g.z, c]);
                }
                expected["uvw"] = serde_json::json!(values);
            }
            assert_eq!(g.mesh.faces[0].attrs["texture"], expected);
        }
        targets.apply(&mut scene, None, true);
        for (g, old) in scene.groups.iter().zip(&original.groups) {
            assert_eq!(g.mesh, old.mesh);
            for (v, old) in g.mesh.vertices.iter().zip(&old.mesh.vertices) {
                assert_eq!(
                    v.position.to_array().map(f64::to_bits),
                    old.position.to_array().map(f64::to_bits)
                );
            }
        }
        for _ in 0..32 {
            targets.apply(&mut scene, Some(&matrix), false);
            targets.apply(&mut scene, None, false);
        }
        assert_eq!(
            targets.shared[0]
                .coordinates
                .iter()
                .map(|p| (p.mapped.as_ptr(), p.mapped.capacity()))
                .collect::<Vec<_>>(),
            capacities,
            "Warm motion and restore reuse captured scratch"
        );
    }

    #[test]
    fn nested_and_definition_owners_share_geometry_without_carrying_axes() {
        let mut scene = Scene::new();
        let mut mesh = Mesh::new();
        let v = [
            mesh.add_vertex(DVec3::ZERO),
            mesh.add_vertex(DVec3::X),
            mesh.add_vertex(DVec3::Y),
        ];
        mesh.add_face(v.to_vec(), vec![]);
        let token = uuid::Uuid::new_v4();
        scene.register_mesh_share(token, mesh.clone()).unwrap();
        let mut selected = Group::with_mesh("selected", mesh.clone());
        selected.mesh_share = Some(token);
        let mut child = Group::with_mesh("nested", mesh.clone());
        child.mesh_share = Some(token);
        child.native_document =
            serde_json::json!({"axes":DMat4::from_translation(DVec3::X*8.).to_cols_array()});
        let child_before = child.clone();
        let mut parent = Group::new("parent");
        parent.children.push(child);
        scene.groups = vec![selected, parent];
        let def = uuid::Uuid::new_v4();
        scene.component_definitions.insert(
            def,
            ingetrazo_core::components::ComponentDefinition {
                mesh_share: Some(token),
                mesh: mesh.clone(),
                children: vec![],
            },
        );
        scene.select(SelectionItem::Group(0));
        let mut targets = Targets::capture(&scene);
        let matrix = ScaleMatrix::new(DVec3::ZERO, DVec3::splat(2.), None);
        targets.apply(&mut scene, Some(&matrix), true);
        assert_eq!(scene.groups[0].mesh, scene.groups[1].children[0].mesh);
        assert_eq!(scene.mesh_shares[&token], scene.groups[0].mesh);
        assert_eq!(scene.component_definitions[&def].mesh, scene.groups[0].mesh);
        assert_eq!(
            scene.groups[1].children[0].native_document,
            child_before.native_document
        );
        assert_eq!(
            scene.groups[1].children[0].history_id,
            child_before.history_id
        );
        targets.apply(&mut scene, None, true);
        assert_eq!(scene.mesh_shares[&token], mesh);
        assert_eq!(scene.groups[1].children[0], child_before);
    }
}
