//! Borrowed production Scene adapter. Static catalog admission is separate from
//! stable-ID model refresh; callers supply explicit geometry/paint/style/layer
//! and topology revisions, never the camera or coarse per-motion Scene version.
use crate::world_placement::{Packet, Pose, PrototypeKey};
use glam::DMat4;
use ingetrazo_core::{camera::OrbitCamera, groups::Group, scene::Scene};
use ingetrazo_render::{
    buffer::MeshRenderBuffer, pipeline::build_inherited_placement_prototype_surfaces, StyleConfig,
};
use std::sync::Arc;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
pub use ingetrazo_render::scene_revision::Revisions;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Signature {
    identity: u128,
    color: Option<[u64; 3]>,
    opacity: Option<u64>,
    mat: Option<String>,
}
#[derive(Debug)]
struct Slot {
    path: Vec<(usize, u128)>,
    id: u128,
    root: usize,
}
#[derive(Debug)]
struct Prototype {
    signature: Signature,
    representative: usize,
    key: PrototypeKey,
}
#[derive(Debug)]
pub struct Catalog {
    pub packet: Arc<Packet>,
    revisions: Revisions,
    slots: Vec<Slot>,
    prototypes: Vec<Prototype>,
    retained_bytes: usize,
    next_variant: u64,
    key_index: Vec<(PrototypeKey, usize)>,
}
fn paint(g: &Group) -> Option<&serde_json::Value> {
    g.native_document
        .get("material")
        .filter(|v| v.as_object().is_some_and(|o| !o.is_empty()))
}
fn signature(g: &Group, root: &Group) -> Result<Signature, String> {
    let p = paint(g).or_else(|| paint(root));
    let mut s = Signature {
        identity: g.mesh_share.unwrap_or(g.id).as_u128(),
        color: None,
        opacity: None,
        mat: None,
    };
    if let Some(p) = p {
        if p.get("texture").is_some_and(|t| !t.is_null()) {
            return Err("Metal placement material texture requires native texture stage".into());
        }
        if let Some(c) = p.get("color").filter(|v| !v.is_null()) {
            let a = c
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or("Metal container color requires finite RGB3")?;
            let mut bits = [0; 3];
            for (i, v) in a.iter().enumerate() {
                let n = v
                    .as_f64()
                    .filter(|n| n.is_finite() && *n >= 0. && *n <= 1.)
                    .ok_or("Metal container color invalid")?;
                bits[i] = if n == 0. { 0 } else { n.to_bits() };
            }
            s.color = Some(bits);
        }
        if let Some(o) = p.get("opacity").filter(|v| !v.is_null()) {
            let n = o
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.999 && *n <= 1.)
                .ok_or("Metal placement translucent material requires alpha stage")?;
            s.opacity = Some(if n == 0. { 0 } else { n.to_bits() });
        }
        if let Some(m) = p.get("mat").filter(|v| !v.is_null()) {
            let m = m
                .as_str()
                .filter(|m| m.len() <= 1024)
                .ok_or("Metal container material identity unsupported or excessive")?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(m.len())
                .map_err(|_| "Material identity allocation refused")?;
            owned.push_str(m);
            s.mat = Some(owned);
        }
    }
    Ok(s)
}
fn own_placement(g: &Group) -> bool {
    if g.native_document.is_object() {
        g.native_document
            .get("proto")
            .and_then(|v| v.as_u64())
            .is_some()
            && g.native_document.get("xform").is_some_and(|v| v.is_array())
            || !g.children.is_empty()
    } else {
        g.component_definition.is_some() || !g.children.is_empty()
    }
}
fn resolve<'a>(scene: &'a Scene, slot: &Slot) -> Result<&'a Group, String> {
    let mut groups = &scene.groups;
    let mut result = None;
    for &(i, id) in &slot.path {
        let g = groups.get(i).ok_or("Metal placement path stale")?;
        if g.id.as_u128() != id {
            return Err("Metal placement ancestor identity changed; rebuild catalog".into());
        }
        result = Some(g);
        groups = &g.children;
    }
    let g = result.ok_or("Metal placement path empty")?;
    if g.id.as_u128() != slot.id {
        return Err("Metal placement stable identity changed; rebuild catalog".into());
    }
    Ok(g)
}
impl Catalog {
    pub fn new(
        scene: &Scene,
        revisions: Revisions,
        previous: Option<&Self>,
    ) -> Result<Self, String> {
        if !scene.mesh.faces.is_empty() || !scene.mesh.edges.is_empty() {
            return Err(
                "Metal placement-only adapter needs consolidated loose geometry stage".into(),
            );
        }
        let mut count = 0usize;
        fn count_groups(gs: &[Group], depth: usize, n: &mut usize) -> Result<(), String> {
            if depth >= MAX_DEPTH && !gs.is_empty() {
                return Err("Metal placement nesting exceeds128".into());
            }
            for g in gs {
                *n = n.checked_add(1).ok_or("Placement count overflow")?;
                count_groups(&g.children, depth + 1, n)?;
            }
            Ok(())
        }
        count_groups(&scene.groups, 0, &mut count)?;
        // Each record retains a bounded path and at most1024 material identity bytes.
        let bound = count
            .checked_mul(
                std::mem::size_of::<Slot>()
                    + std::mem::size_of::<Prototype>()
                    + std::mem::size_of::<(Signature, usize)>()
                    + MAX_DEPTH * std::mem::size_of::<(usize, u128)>()
                    + 1024
                    + std::mem::size_of::<PrototypeKey>() * 2
                    + std::mem::size_of::<(PrototypeKey, usize)>()
                    + 4,
            )
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Metal scene catalog cold metadata exceeds8MiB")?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|_| "Placement slots allocation refused")?;
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(count)
            .map_err(|_| "Placement prototypes allocation refused")?;
        let mut path = [(0usize, 0u128); MAX_DEPTH];
        fn walk(
            gs: &[Group],
            scene: &Scene,
            root: usize,
            owner: &Group,
            depth: usize,
            path: &mut [(usize, u128); MAX_DEPTH],
            inherited: bool,
            slots: &mut Vec<Slot>,
            candidates: &mut Vec<(Signature, usize)>,
        ) -> Result<(), String> {
            for (i, g) in gs.iter().enumerate() {
                path[depth] = (if depth == 0 { root } else { i }, g.id.as_u128());
                if g.hidden || !scene.is_layer_visible(g.layer.as_deref()) {
                    continue;
                }
                let placed = inherited || own_placement(g);
                if g.billboard {
                    return Err("Metal placement billboard requires camera-facing stage".into());
                }
                if !g.mesh.faces.is_empty() {
                    if !placed {
                        return Err(
                            "Metal classic group requires consolidated world shading stage".into(),
                        );
                    }
                    let mut p = Vec::new();
                    p.try_reserve_exact(depth + 1)
                        .map_err(|_| "Placement path allocation refused")?;
                    p.extend_from_slice(&path[..=depth]);
                    let index = slots.len();
                    slots.push(Slot {
                        path: p,
                        id: g.id.as_u128(),
                        root,
                    });
                    candidates.push((signature(g, owner)?, index));
                } else if !g.mesh.edges.is_empty() {
                    return Err("Metal placement wire requires native edge stage".into());
                }
                if !g.children.is_empty() {
                    walk(
                        &g.children,
                        scene,
                        root,
                        owner,
                        depth + 1,
                        path,
                        placed,
                        slots,
                        candidates,
                    )?;
                }
            }
            Ok(())
        }
        for (i, g) in scene.groups.iter().enumerate() {
            path[0] = (i, g.id.as_u128());
            if g.hidden || !scene.is_layer_visible(g.layer.as_deref()) {
                continue;
            }
            walk(
                std::slice::from_ref(g),
                scene,
                i,
                g,
                0,
                &mut path,
                false,
                &mut slots,
                &mut candidates,
            )?;
        }
        candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut prototypes = Vec::new();
        prototypes
            .try_reserve_exact(candidates.len())
            .map_err(|_| "Prototype catalog allocation refused")?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(slots.len())
            .map_err(|_| "Placement bindings allocation refused")?;
        bindings.resize(slots.len(), 0);
        let mut next = previous.map(|p| p.next_variant).unwrap_or(0);
        for (sig, slot) in candidates {
            let index = if prototypes
                .last()
                .is_some_and(|p: &Prototype| p.signature == sig)
            {
                prototypes.len() - 1
            } else {
                let old = previous
                    .filter(|p| p.revisions.paint == revisions.paint)
                    .and_then(|p| {
                        p.prototypes
                            .binary_search_by(|p| p.signature.cmp(&sig))
                            .ok()
                            .map(|i| p.prototypes[i].key.paint_revision)
                    });
                let paint_revision = if let Some(v) = old {
                    v
                } else {
                    next = next
                        .checked_add(1)
                        .ok_or("Placement material revision exhausted")?;
                    next
                };
                prototypes.push(Prototype {
                    key: PrototypeKey {
                        identity: sig.identity,
                        geometry_revision: revisions.geometry,
                        paint_revision,
                        style_revision: revisions.style,
                        layer_revision: revisions.layers,
                    },
                    signature: sig,
                    representative: slot,
                });
                prototypes.len() - 1
            };
            bindings[slot] = u32::try_from(index).map_err(|_| "Prototype count exceedsu32")?;
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(prototypes.len())
            .map_err(|_| "Prototype keys allocation refused")?;
        keys.extend(prototypes.iter().map(|p| p.key));
        let packet = Packet::new(keys, bindings)?;
        let retained_bytes = std::mem::size_of::<Self>()
            + slots.capacity() * std::mem::size_of::<Slot>()
            + slots
                .iter()
                .map(|s| s.path.capacity() * std::mem::size_of::<(usize, u128)>())
                .sum::<usize>()
            + prototypes.capacity() * std::mem::size_of::<Prototype>()
            + prototypes
                .iter()
                .map(|p| p.signature.mat.as_ref().map_or(0, String::capacity))
                .sum::<usize>()
            + packet.retained_payload_bytes();
        if retained_bytes > MAX_BYTES || bound > MAX_BYTES {
            return Err("Metal actual scene catalog capacity exceeds8MiB".into());
        }
        let mut key_index = Vec::new();
        key_index
            .try_reserve_exact(prototypes.len())
            .map_err(|_| "Prototype lookup allocation refused")?;
        key_index.extend(prototypes.iter().enumerate().map(|(i, p)| (p.key, i)));
        key_index.sort_unstable_by_key(|v| v.0);
        let retained_bytes = retained_bytes
            .checked_add(key_index.capacity() * std::mem::size_of::<(PrototypeKey, usize)>())
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Metal catalog lookup capacity exceeds8MiB")?;
        Ok(Self {
            packet,
            revisions,
            slots,
            prototypes,
            retained_bytes,
            next_variant: next,
            key_index,
        })
    }
    pub fn revisions(&self) -> Revisions {
        self.revisions
    }
    pub fn retained_payload_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn refresh_models(
        &self,
        scene: &Scene,
        revisions: Revisions,
        output: &mut Vec<Pose>,
    ) -> Result<(), String> {
        if revisions != self.revisions {
            return Err("Metal scene geometry domains changed; rebuild catalog".into());
        }
        // Validate every identity/matrix before mutation. Capacity admission is cold;
        // an unchanged warm output needs no allocation and no geometry/material scan.
        for s in &self.slots {
            let _ = resolve(scene, s)?;
            let mut m = DMat4::IDENTITY;
            let mut gs = &scene.groups;
            for &(i, _) in &s.path {
                let g = &gs[i];
                m *= g.transform;
                gs = &g.children;
            }
            if !m.is_finite() || m.as_mat4().to_cols_array().iter().any(|v| !v.is_finite()) {
                return Err("Metal placement composed matrix invalid".into());
            }
        }
        if self.slots.len() > output.capacity() {
            let mut candidate = Vec::new();
            candidate
                .try_reserve_exact(self.slots.len())
                .map_err(|_| "Model output allocation refused")?;
            self.fill_models(scene, &mut candidate);
            *output = candidate;
        } else {
            output.clear();
            self.fill_models(scene, output);
        }
        Ok(())
    }
    fn fill_models(&self, scene: &Scene, out: &mut Vec<Pose>) {
        for s in &self.slots {
            let mut m = DMat4::IDENTITY;
            let mut gs = &scene.groups;
            for &(i, _) in &s.path {
                let g = &gs[i];
                m *= g.transform;
                gs = &g.children;
            }
            out.push(Pose {
                mvp: m.as_mat4().to_cols_array_2d(),
                reflected: m.determinant() < 0.,
            });
        }
    }
    pub fn collect(
        &self,
        key: &PrototypeKey,
        scene: &Scene,
        camera: &OrbitCamera,
        style: &StyleConfig,
    ) -> Result<MeshRenderBuffer, String> {
        let lookup = self
            .key_index
            .binary_search_by_key(key, |v| v.0)
            .map_err(|_| "Metal prototype key not in Scene catalog")?;
        let p = &self.prototypes[self.key_index[lookup].1];
        let s = &self.slots[p.representative];
        let g = resolve(scene, s)?;
        let owner = scene
            .groups
            .get(s.root)
            .ok_or("Metal placement owner stale")?;
        let data = build_inherited_placement_prototype_surfaces(
            g,
            paint(owner),
            scene,
            camera,
            style,
            true,
        )?;
        if !data.transparent_mesh.indices.is_empty()
            || !data.textured_meshes.is_empty()
            || !data.billboards.is_empty()
        {
            return Err("Metal Scene placement needs texture/alpha/billboard stage".into());
        }
        Ok(data.opaque_mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingetrazo_core::mesh::Mesh;
    fn revisions() -> Revisions {
        Revisions {
            geometry: 1,
            paint: 1,
            style: 1,
            layers: 1,
            topology: 1,
        }
    }
    fn mesh() -> Mesh {
        let mut m = Mesh::new();
        let vs = [glam::DVec3::ZERO, glam::DVec3::X, glam::DVec3::Y].map(|p| m.add_vertex(p));
        m.add_face(vs.to_vec(), vec![]);
        m
    }
    fn style() -> StyleConfig {
        let mut s = StyleConfig::default();
        s.show_edges = false;
        s.show_profiles = false;
        s.show_sky = false;
        s.show_grid = false;
        s
    }
    #[test]
    fn scene_catalog_owns_each_mesh_once_and_refreshes_nested_stable_ids() {
        let mut scene = Scene::new();
        let mut a = Group::with_mesh("A", mesh());
        a.component_definition = Some(a.id);
        a.mesh_share = Some(a.id);
        let mut b = a.duplicate_instance();
        b.transform = DMat4::from_translation(glam::DVec3::X * 4.);
        let mut root = Group::with_mesh("root", mesh());
        root.transform = DMat4::from_translation(glam::DVec3::Y * 2.);
        root.children = vec![a, b];
        scene.groups.push(root);
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(cat.packet.prototype_count(), 2);
        assert_eq!(cat.packet.placement_count(), 3);
        let mut poses = Vec::new();
        cat.refresh_models(&scene, revisions(), &mut poses).unwrap();
        assert_eq!(poses[2].mvp[3], [4., 2., 0., 1.]);
        let capacity = poses.capacity();
        let packet = cat.packet.clone();
        scene.groups[0].children[1].transform = DMat4::from_translation(glam::DVec3::X * 5.);
        cat.refresh_models(&scene, revisions(), &mut poses).unwrap();
        assert_eq!(poses.capacity(), capacity);
        assert_eq!(poses[2].mvp[3], [5., 2., 0., 1.]);
        assert!(Arc::ptr_eq(&packet, &cat.packet));
        let before = poses.clone();
        scene.groups[0].children[1].id = scene.groups[0].id;
        assert!(cat.refresh_models(&scene, revisions(), &mut poses).is_err());
        assert_eq!(poses, before);
    }
    #[test]
    fn inherited_placement_uses_top_owner_paint_and_local_own_mesh() {
        let mut scene = Scene::new();
        let leaf = Group::with_mesh("leaf", mesh());
        let mut middle = Group::new("middle");
        middle.native_document = serde_json::json!({"material":{"color":[0,1,0]}});
        middle.children.push(leaf);
        let mut root = Group::new("root");
        root.native_document = serde_json::json!({"material":{"color":[1,0,0]}});
        root.children.push(middle);
        scene.groups.push(root);
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(cat.packet.prototype_count(), 1);
        assert_eq!(cat.packet.placement_count(), 1);
        let data = cat
            .collect(
                &cat.prototypes[0].key,
                &scene,
                &OrbitCamera::default(),
                &style(),
            )
            .unwrap();
        assert_eq!(data.indices.len(), 3);
        assert!(data
            .vertices
            .iter()
            .all(|v| v.position[0] <= 1. && v.position[1] <= 1.));
        assert!(data
            .vertices
            .iter()
            .all(|v| v.color[0] > 0. && v.color[1] == 0.));
    }
    #[test]
    fn catalog_refusals_preserve_existing_and_same_mesh_different_paint_stays_distinct() {
        let mut scene = Scene::new();
        let mut a = Group::with_mesh("shared", mesh());
        a.component_definition = Some(a.id);
        a.mesh_share = Some(a.id);
        a.native_document = serde_json::json!({"proto":0,"xform":DMat4::IDENTITY.to_cols_array(),"material":{"color":[1,0,0]}});
        let mut b = a.duplicate_instance();
        b.native_document["material"]["color"] = serde_json::json!([0, 1, 0]);
        scene.groups = vec![a, b];
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(cat.packet.prototype_count(), 2);
        scene.groups[1].native_document["material"]["opacity"] = serde_json::json!(0.5);
        assert!(Catalog::new(&scene, revisions(), Some(&cat)).is_err());
        assert_eq!(cat.packet.prototype_count(), 2);
        scene.groups[1].native_document["material"]
            .as_object_mut()
            .unwrap()
            .remove("opacity");
        let rebuilt = Catalog::new(&scene, revisions(), Some(&cat)).unwrap();
        assert_eq!(
            rebuilt.prototypes[0].key.paint_revision,
            cat.prototypes[0].key.paint_revision
        );
        let mut out = Vec::new();
        cat.refresh_models(&scene, revisions(), &mut out).unwrap();
        let before = out.clone();
        let mut rev = revisions();
        rev.geometry = 2;
        assert!(cat.refresh_models(&scene, rev, &mut out).is_err());
        assert_eq!(out, before);
    }
    #[test]
    fn source_numeric_signature_and_ancestor_identity_controls() {
        let mut scene = Scene::new();
        let mut leaf = Group::with_mesh("leaf", mesh());
        leaf.mesh_share = Some(leaf.id);
        leaf.native_document = serde_json::json!({"material":{"color":[0,1,0],"vendor":71}});
        let mut other = leaf.duplicate_instance();
        other.native_document =
            serde_json::json!({"material":{"color":[-0.0,1.0,0.0],"vendor":99}});
        let mut root = Group::new("root");
        root.children = vec![leaf, other];
        scene.groups.push(root);
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(cat.packet.prototype_count(), 1);
        let mut poses = Vec::new();
        cat.refresh_models(&scene, revisions(), &mut poses).unwrap();
        let prior = poses.clone();
        scene.groups[0].id = scene.groups[0].children[0].id;
        assert!(cat
            .refresh_models(&scene, revisions(), &mut poses)
            .unwrap_err()
            .contains("ancestor"));
        assert_eq!(poses, prior);
    }
    #[test]
    fn catalog_metadata_and_depth_refuse_before_publication() {
        let mut scene = Scene::new();
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        scene.groups = (0..2500).map(|_| Group::new("empty")).collect();
        assert!(Catalog::new(&scene, revisions(), Some(&cat))
            .unwrap_err()
            .contains("8MiB"));
        assert_eq!(cat.packet.placement_count(), 0);
        let mut nested = Group::new("leaf");
        for _ in 0..128 {
            let mut parent = Group::new("parent");
            parent.children.push(nested);
            nested = parent;
        }
        scene.groups = vec![nested];
        assert!(Catalog::new(&scene, revisions(), Some(&cat))
            .unwrap_err()
            .contains("128"));
    }
}

/// CPU catalog and scratch ownership for the production native consumer. Warm
/// prepare keeps these Vec capacities; only accepted frames publish current models.
#[derive(Default)]
pub struct Adapter {
    pub(crate) catalog: Option<Catalog>,
    pub(crate) current: Vec<Pose>,
    pub(crate) scratch: Vec<Pose>,
    pub catalog_builds: u64,
    accepted_pose_revision: Option<u64>,
}
impl Adapter {
    fn needs_model_refresh(&self,cold:bool,revision:Option<u64>)->bool {
        cold || revision.is_none() || self.accepted_pose_revision != revision
    }
    pub fn retained_payload_bytes(&self) -> usize {
        self.catalog
            .as_ref()
            .map_or(0, Catalog::retained_payload_bytes)
            + (self.current.capacity() + self.scratch.capacity()) * std::mem::size_of::<Pose>()
    }
}

#[cfg(target_os = "macos")]
impl crate::world::World {
    /// Runnable borrowed Scene adapter to the actual native instance-buffer draw
    /// path. Explicit domains must be advanced by the host for non-pose mutations.
    /// This stage refuses missing visible passes instead of activating a partial host.
    pub fn prepare_scene_placements(
        &mut self,
        metal: &crate::NativeMetal,
        adapter: &mut Adapter,
        scene: &Scene,
        revisions: Revisions,
        camera: &OrbitCamera,
        style: &StyleConfig,
        camera_matrix: [[f32; 4]; 4],
        extent: (u32, u32),
    ) -> Result<Option<Arc<crate::world::Prepared>>, String> {
        self.prepare_scene_placements_revisioned(metal,adapter,scene,revisions,None,camera,style,camera_matrix,extent)
    }

    /// Revision-aware host entry. None preserves legacy always-refresh behavior.
    /// Some requires an authoritative pose epoch, separate from camera changes.
    pub fn prepare_scene_placements_revisioned(
        &mut self,
        metal: &crate::NativeMetal,
        adapter: &mut Adapter,
        scene: &Scene,
        revisions: Revisions,
        pose_revision: Option<u64>,
        camera: &OrbitCamera,
        style: &StyleConfig,
        camera_matrix: [[f32; 4]; 4],
        extent: (u32, u32),
    ) -> Result<Option<Arc<crate::world::Prepared>>, String> {
        self.validate_context_owner(metal)?;
        if style.show_edges || style.show_profiles || style.show_sky || style.show_grid {
            return Err("Metal Scene adapter needs native edge/profile/sky/grid passes".into());
        }
        if scene.show_section_cuts && !scene.section_planes.is_empty()
            || scene.shadows.enabled
            || !scene.image_planes.is_empty()
            || !scene.dimensions.is_empty()
            || !scene.text_labels.is_empty()
        {
            return Err("Metal Scene adapter needs section/shadow/image/annotation passes".into());
        }
        use crate::world_policy::{Mode, Policy};
        use ingetrazo_render::VisualStyle;
        let mode = match style.visual_style {
            VisualStyle::ShadedWithTextures => Mode::Material,
            VisualStyle::HiddenLine => Mode::HiddenLine,
            VisualStyle::Monochrome => Mode::Monochrome,
            _ => return Err("Metal Scene adapter wireframe/XRay passes not implemented".into()),
        };
        let cold = adapter
            .catalog
            .as_ref()
            .is_none_or(|c| c.revisions != revisions);
        let candidate = if cold {
            Some(Catalog::new(scene, revisions, adapter.catalog.as_ref())?)
        } else {
            None
        };
        let catalog = candidate.as_ref().or(adapter.catalog.as_ref()).unwrap();
        let refresh = adapter.needs_model_refresh(cold,pose_revision);
        if refresh { catalog.refresh_models(scene, revisions, &mut adapter.scratch)?; }
        // Seed both pose Vec capacities during cold preparation. First camera
        // motion must not allocate a second buffer. Keep growth unpublished.
        let mut candidate_current = if catalog.packet.placement_count() > adapter.current.capacity()
        {
            let mut poses = Vec::new();
            poses
                .try_reserve_exact(catalog.packet.placement_count())
                .map_err(|_| "Current model buffer allocation refused")?;
            Some(poses)
        } else {
            None
        };

        if adapter.retained_payload_bytes() > MAX_BYTES * 3 {
            return Err("Metal adapter retained CPU storage exceeds24MiB".into());
        }
        let frame = self.prepare_placements_with_camera(
            metal,
            catalog.packet.clone(),
            if refresh { &adapter.scratch } else { &adapter.current },
            camera_matrix,
            extent,
            style.background_color.map(f64::from),
            Policy {
                mode,
                front: style.front_color,
                back: style.back_color,
            },
            |key| catalog.collect(key, scene, camera, style),
        )?;
        if frame.is_some() {
            if let Some(c) = candidate {
                adapter.catalog = Some(c);
                adapter.catalog_builds += 1;
            }
            if let Some(poses) = candidate_current.take() {
                adapter.current = poses;
            }
            if refresh { std::mem::swap(&mut adapter.current, &mut adapter.scratch); }
            adapter.accepted_pose_revision = pose_revision;
        }
        Ok(frame)
    }
}

#[cfg(test)] mod pose_revision_tests {
    use super::*;
    #[test] fn only_accepted_pose_epoch_reuses_models() {
        let mut adapter=Adapter::default();
        assert!(adapter.needs_model_refresh(false,Some(0)));
        adapter.accepted_pose_revision=Some(0);
        assert!(!adapter.needs_model_refresh(false,Some(0)));
        assert!(adapter.needs_model_refresh(false,Some(1)));
        assert!(adapter.needs_model_refresh(true,Some(0)));
        assert!(adapter.needs_model_refresh(false,None));
        // A refused candidate must not publish its proposed epoch.
        assert_eq!(adapter.accepted_pose_revision,Some(0));
    }
}
