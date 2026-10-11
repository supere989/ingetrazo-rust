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
use std::collections::{BTreeSet, HashMap};

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
}
#[derive(Debug, Clone)]
struct SharedSeed {
    token: uuid::Uuid,
    commands: usize,
    owners: Vec<SharedOwner>,
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
    SharedSeed {
        token,
        commands,
        owners,
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
    /// The matrix is None for an exact grab-time restore. Commit metadata is
    /// derived from originals once; previews never reauthor texture maps/axes.
    pub fn apply(&mut self, scene: &mut Scene, matrix: Option<&ScaleMatrix>, commit: bool) {
        self.refresh_images(scene);
        self.apply_with_indices(scene, matrix, commit, &self.image_indices);
    }
    pub fn apply_cold(&self, scene: &mut Scene, matrix: Option<&ScaleMatrix>, commit: bool) {
        let indices = scene
            .image_planes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.as_u128(), i))
            .collect();
        self.apply_with_indices(scene, matrix, commit, &indices);
    }
    fn apply_with_indices(
        &self,
        scene: &mut Scene,
        matrix: Option<&ScaleMatrix>,
        commit: bool,
        indices: &HashMap<u128, usize>,
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
                changed |= mesh.place_vertices(owned.vertices.iter().map(|seed| {
                    let mut p = seed.position;
                    if let Some(m) = matrix {
                        for _ in 0..count {
                            p = m.point(p);
                        }
                    }
                    (seed.index, seed.identity, p)
                }));
                if commit {
                    for (id, t) in &owned.textures {
                        if let Some(f) = mesh.faces.get_mut(*id) {
                            if let Some(t) = t {
                                let mut t = t.clone();
                                if let Some(m) = matrix {
                                    for _ in 0..count {
                                        remap_uv(&mut t, m);
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
                        g.native_document["axes"] = serde_json::json!(m
                            .compose(axes.unwrap_or(DMat4::IDENTITY))
                            .to_cols_array());
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
                            if let Some(m) = matrix {
                                remap_uv(&mut t, m);
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
fn remap_uv(texture: &mut serde_json::Value, matrix: &ScaleMatrix) {
    let Some(values) = texture
        .get("uvw")
        .and_then(|v| v.as_array())
        .filter(|a| a.len() == 8)
    else {
        return;
    };
    let Some(mut values) = values
        .iter()
        .map(|v| v.as_f64())
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let m = matrix.matrix();
    let inv = m.inverse().as_mat4().as_dmat4();
    if !inv.is_finite() {
        return;
    }
    let translation = m.w_axis.truncate();
    for base in [0, 4] {
        let g = inv.transpose().transform_vector3(DVec3::new(
            values[base],
            values[base + 1],
            values[base + 2],
        ));
        let c = values[base + 3] - g.dot(translation);
        values[base..base + 4].copy_from_slice(&[g.x, g.y, g.z, c]);
    }
    texture["uvw"] = serde_json::json!(values);
}

#[cfg(test)]
mod share_tests {
    use super::*;
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
