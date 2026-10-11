//! Borrowed production Scene adapter. Static catalog admission is separate from
//! stable-ID model refresh; callers supply explicit geometry/paint/style/layer
//! and topology revisions, never the camera or coarse per-motion Scene version.
use crate::world_placement::{Packet, Pose, PrototypeKey};
use glam::DMat4;
use ingetrazo_core::{camera::OrbitCamera, groups::Group, scene::Scene};
use ingetrazo_render::{
    buffer::MeshRenderBuffer,
    pipeline::{
        build_classic_own_surfaces, build_inherited_placement_prototype_surfaces,
        build_loose_world_surfaces,
    },
    StyleConfig,
};
use std::sync::Arc;
/// Exact missing-stage classifications. Allocation/invalid data/device errors
/// are refusals and must never silently select a fallback renderer.
pub fn unsupported_stage(error:&str)->bool {
    matches!(error,
        "Metal placement-only adapter needs consolidated loose geometry stage" |
        "Metal classic group requires consolidated world shading stage" |
        "Metal placement wire requires native edge stage" |
        "Metal placement billboard requires camera-facing stage" |
        "Metal placement material texture requires native texture stage" |
        "Metal placement translucent material requires alpha stage" |
        "Metal Scene placement needs texture/alpha/billboard stage" |
        "Metal Scene adapter needs native edge/profile/sky/grid passes" |
        "Metal Scene adapter needs section/shadow/image/annotation passes" |
        "Metal Scene adapter wireframe/XRay passes not implemented" |
        "Metal Scene adapter needs selection/guide overlays" |
        "Metal Scene adapter needs geographic/terrain passes" |
        "Metal plan style mixed-material policy requires subsequent stage")
}
// viewport.py ordinary faces use DEFAULT_FACE_COLOR, independent of the
// monochrome/hidden-line front tint. Shaded mode still retains texture runs
// for the cached image-mean consumer. Only cold prototype collection uses this.
fn ordinary_surface_style(style: &StyleConfig) -> StyleConfig {
    let mut value = style.clone();
    if value.visual_style == ingetrazo_render::VisualStyle::ShadedWithTextures {
        value.front_color = [0.96, 0.95, 0.925, 1.];
        value.show_textures = true;
    }
    value
}
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
pub use ingetrazo_render::scene_revision::Revisions;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Signature {
    loose: bool,
    classic_world: Option<[u64; 16]>,
    identity: u128,
    color: Option<[u64; 3]>,
    opacity: Option<u64>,
    mat: Option<String>,
    texture: Option<String>,
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
fn texture_identity_bound(value:&serde_json::Value,depth:usize)->Result<usize,String>{
    if depth>16{return Err("Metal texture identity nesting exceeds16".into());}
    let bytes=match value {
        serde_json::Value::Null|serde_json::Value::Bool(_)=>5,
        serde_json::Value::Number(_)=>64,
        serde_json::Value::String(v)=>v.len().checked_mul(6).and_then(|n|n.checked_add(2)).ok_or("Texture identity size overflow")?,
        serde_json::Value::Array(v)=>{let mut n=2usize;for x in v{n=n.checked_add(texture_identity_bound(x,depth+1)?+1).filter(|n|*n<=4096).ok_or("Metal texture identity exceeds4096 bytes")?;}n},
        serde_json::Value::Object(v)=>{let mut n=2usize;for(k,x)in v{n=n.checked_add(k.len().checked_mul(6).ok_or("Texture key size overflow")?+4).and_then(|n|n.checked_add(texture_identity_bound(x,depth+1).ok()?)).filter(|n|*n<=4096).ok_or("Metal texture identity exceeds4096 bytes")?;}n}
    };
    if bytes>4096{return Err("Metal texture identity exceeds4096 bytes".into());}Ok(bytes)
}
fn signature(g: &Group, root: &Group) -> Result<Signature, String> {
    let p = paint(g).or_else(|| paint(root));
        let mut s = Signature {
        loose: false,
        classic_world: None,
        identity: g.mesh_share.unwrap_or(g.id).as_u128(),
        color: None,
        opacity: None,
        mat: None,
        texture: None,
    };
    if let Some(p) = p {
        if let Some(t)=p.get("texture").filter(|t|!t.is_null()) {
            texture_identity_bound(t,0)?;
            let encoded=serde_json::to_string(t).map_err(|_|"Metal texture identity serialization refused")?;
            if encoded.len()>4096 {return Err("Metal texture identity exceeds4096 bytes".into());}
            s.texture=Some(encoded);
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
                .filter(|n| n.is_finite() && *n >= 0. && *n <= 1.)
                .ok_or("Metal placement material opacity invalid")?;
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
#[cfg(target_os="macos")]
fn sampled_asset<'a>(scene:&'a Scene,path:&str)->Result<std::borrow::Cow<'a,[u8]>,String>{
    if let Some(bytes)=scene.embedded_textures.get(path){if bytes.len()>16*1024*1024{return Err("Metal embedded texture exceeds16MiB".into());}return Ok(std::borrow::Cow::Borrowed(bytes));}
    use std::io::Read;let file=std::fs::File::open(path).map_err(|e|format!("Metal texture resource {path}: {e}"))?;if file.metadata().map_err(|e|e.to_string())?.len()>16*1024*1024{return Err("Metal file texture exceeds16MiB".into());}
    let mut bytes=Vec::new();file.take(16*1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;if bytes.len()>16*1024*1024{return Err("Metal file texture grew beyond16MiB".into());}Ok(std::borrow::Cow::Owned(bytes))
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
        let loose=!scene.mesh.faces.is_empty();
        // Loose wires have a separate cached native line owner, no surface prototype.
        let mut count = usize::from(loose);
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
                    + 1024 + 4096
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
            parent_world: DMat4,
            slots: &mut Vec<Slot>,
            candidates: &mut Vec<(Signature, usize)>,
        ) -> Result<(), String> {
            for (i, g) in gs.iter().enumerate() {
                path[depth] = (if depth == 0 { root } else { i }, g.id.as_u128());
                if g.hidden || !scene.is_layer_visible(g.layer.as_deref()) {
                    continue;
                                }
                let placed = inherited || own_placement(g);
                let world = parent_world * g.transform;
                if !world.is_finite() {
                    return Err("Metal placement composed matrix invalid".into());
                }
                if g.billboard {
                    return Err("Metal placement billboard requires camera-facing stage".into());
                }
                if !g.mesh.faces.is_empty() {
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
                    let mut sig = signature(g, owner)?;
                    if !placed {
                        sig.classic_world =
                            Some(
                                world
                                    .to_cols_array()
                                    .map(|v| if v == 0. { 0 } else { v.to_bits() }),
                            );
                    }
                    candidates.push((sig, index));
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
                        world,
                        slots,
                        candidates,
                    )?;
                }
            }
            Ok(())
        }
        if loose {
            slots.push(Slot{path:Vec::new(),id:0,root:usize::MAX});
            candidates.push((                Signature {
                    loose: true,
                    classic_world: None,
                    identity: 0,
                    color: None,opacity:None,mat:None,texture:None},0));
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
                DMat4::IDENTITY,
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
                .map(|p| p.signature.mat.as_ref().map_or(0, String::capacity)+p.signature.texture.as_ref().map_or(0,String::capacity))
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
            if s.path.is_empty(){continue;}
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
    fn classic_models_changed(&self, scene: &Scene) -> Result<bool, String> {
        // Every binding must be checked: a nonrepresentative shared classic
        // may move while the prototype's representative remains unchanged.
        for (index, slot) in self.slots.iter().enumerate() {
            let p = &self.prototypes[self.packet.bindings[index] as usize];
            if let Some(old) = p.signature.classic_world {
                let _ = resolve(scene, slot)?;
                let mut world = DMat4::IDENTITY;
                let mut gs = &scene.groups;
                for &(i, _) in &slot.path {
                    let g = &gs[i];
                    world *= g.transform;
                    gs = &g.children;
                }
                if !world.is_finite() {
                    return Err("Metal placement composed matrix invalid".into());
                }
                if world
                    .to_cols_array()
                    .map(|v| if v == 0. { 0 } else { v.to_bits() })
                    != old
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn fill_models(&self, scene: &Scene, out: &mut Vec<Pose>) {
        for (slot_index, s) in self.slots.iter().enumerate() {
            let classic = self.prototypes[self.packet.bindings[slot_index] as usize]
                .signature
                .classic_world
                .is_some();
            let mut m = DMat4::IDENTITY;
            let mut gs = &scene.groups;
            for &(i, _) in &s.path {
                let g = &gs[i];
                m *= g.transform;
                gs = &g.children;
                        }
            out.push(Pose {
                mvp: if classic {
                    DMat4::IDENTITY.as_mat4().to_cols_array_2d()
                } else {
                    m.as_mat4().to_cols_array_2d()
                },
                reflected: !classic && m.determinant() < 0.,
            });
        }
    }
    pub fn collect(&self,key:&PrototypeKey,scene:&Scene,camera:&OrbitCamera,style:&StyleConfig)->Result<MeshRenderBuffer,String>{
        let data=self.collect_surfaces(key,scene,camera,style)?;
        if !data.transparent_mesh.indices.is_empty() || !data.textured_meshes.is_empty() || !data.billboards.is_empty() {
            return Err("Metal Scene placement needs texture/alpha/billboard stage".into());
        }
        Ok(data.opaque_mesh)
    }
    pub fn collect_surfaces(
        &self,
        key: &PrototypeKey,
        scene: &Scene,
        camera: &OrbitCamera,
        style: &StyleConfig,
    ) -> Result<ingetrazo_render::pipeline::SceneRenderData, String> {
        // The source shaded mode retains texture geometry but replaces sampling
        // with its cached 4x4 Qt image mean. The general render API stays unchanged.
        let ordinary_style = ordinary_surface_style(style);
        let style = &ordinary_style;
        let lookup = self
            .key_index
            .binary_search_by_key(key, |v| v.0)
            .map_err(|_| "Metal prototype key not in Scene catalog")?;
        let p = &self.prototypes[self.key_index[lookup].1];
        let s = &self.slots[p.representative];
        if s.path.is_empty(){
            let data=build_loose_world_surfaces(scene,style);
            return Ok(data);
        }
        let g = resolve(scene, s)?;
        let owner = scene
            .groups
                        .get(s.root)
            .ok_or("Metal placement owner stale")?;
        if let Some(bits) = p.signature.classic_world {
            let world = DMat4::from_cols_array(&bits.map(f64::from_bits));
            let data = build_classic_own_surfaces(g, paint(owner), world, scene, camera, style)?;
            return Ok(data);
        }
        let data = build_inherited_placement_prototype_surfaces(
            g,
            paint(owner),
            scene,
            camera,
            style,
            true,
        )?;
        if !data.billboards.is_empty() {
            return Err("Metal Scene placement needs texture/alpha/billboard stage".into());
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordinary_face_color_is_independent_of_plan_front_tint() {
        let mut style = ingetrazo_render::StyleConfig::default();
        style.front_color = [0.2, 0.4, 0.8, 1.];
        style.show_textures = false;
        let ordinary = super::ordinary_surface_style(&style);
        assert_eq!(ordinary.front_color, [0.96, 0.95, 0.925, 1.]);
        assert!(ordinary.show_textures);
        assert_eq!(style.front_color, [0.2, 0.4, 0.8, 1.]);
        for mode in [ingetrazo_render::VisualStyle::Monochrome, ingetrazo_render::VisualStyle::HiddenLine] {
            style.visual_style = mode;
            assert_eq!(super::ordinary_surface_style(&style).front_color, style.front_color);
        }
    }

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
        let alpha=Catalog::new(&scene,revisions(),Some(&cat)).unwrap();
        let key=alpha.prototypes.iter().find(|p|p.signature.opacity==Some(0.5f64.to_bits())).unwrap().key;
        let rich=alpha.collect_surfaces(&key,&scene,&OrbitCamera::default(),&style()).unwrap();
        assert!(!rich.transparent_mesh.indices.is_empty());
        assert!(alpha.collect(&key,&scene,&OrbitCamera::default(),&style()).is_err());
        scene.groups[1].native_document["material"]["opacity"]=serde_json::json!(1.5);
        assert!(Catalog::new(&scene,revisions(),Some(&cat)).is_err());
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
    #[test]
    fn classic_world_shading_and_pose_changes_match_shared_world_collector() {
        let mut scene = Scene::new();
        let mut root = Group::with_mesh("classic root", mesh());
        root.transform = DMat4::from_rotation_y(-1.2);
        let mut child = Group::with_mesh("classic child", mesh());
        child.transform =
            DMat4::from_rotation_y(-1.2) * DMat4::from_translation(glam::DVec3::new(3., 0., 0.));
        scene.groups = vec![root, child];
        let catalog = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(catalog.slots.len(), 2);
        let camera = OrbitCamera::default();
        let style = style();
        let reference = ingetrazo_render::pipeline::build_world_surfaces(&scene, &camera, &style);
        let mut records = Vec::new();
        for p in &catalog.prototypes {
            let data = catalog.collect(&p.key, &scene, &camera, &style).unwrap();
            assert_eq!(data.indices.len(), 3);
            records.extend(data.vertices.into_iter().map(|v| {
                (
                    v.position.map(f32::to_bits),
                    v.normal.map(f32::to_bits),
                    v.color.map(f32::to_bits),
                )
            }));
        }
        let mut expected = reference
            .opaque_mesh
            .vertices
            .into_iter()
            .map(|v| {
                (
                    v.position.map(f32::to_bits),
                    v.normal.map(f32::to_bits),
                    v.color.map(f32::to_bits),
                )
            })
            .collect::<Vec<_>>();
        records.sort_unstable();
        expected.sort_unstable();
        assert_eq!(records, expected);
        let mut models = Vec::new();
        catalog
            .refresh_models(&scene, revisions(), &mut models)
            .unwrap();
        assert!(models
            .iter()
            .all(|p| p.mvp == DMat4::IDENTITY.as_mat4().to_cols_array_2d() && !p.reflected));
        assert!(!catalog.classic_models_changed(&scene).unwrap());
        scene.groups[0].transform =
            DMat4::from_scale(glam::DVec3::new(-1., 1., 1.)) * DMat4::from_rotation_y(0.3);
        assert!(catalog.classic_models_changed(&scene).unwrap());
        let rebuilt = Catalog::new(&scene, revisions(), Some(&catalog)).unwrap();
        assert_eq!(
            rebuilt
                .prototypes
                .iter()
                .filter(|p| !catalog.prototypes.iter().any(|old| old.key == p.key))
                .count(),
            1
        );
        scene.groups[0].transform = DMat4::from_cols_array(&[f64::NAN; 16]);
        assert!(catalog.classic_models_changed(&scene).is_err());
    }

    #[test]
    fn classic_face_layer_is_ignored_but_group_layer_and_reflected_winding_are_retained() {
        let mut scene = Scene::new();
        let mut group = Group::with_mesh("classic", mesh());
        group.mesh.faces[0].layer = Some("hidden".into());
        group.transform = DMat4::from_scale(glam::DVec3::new(-1., 1., 1.));
        let mut layer = ingetrazo_core::layers::Layer::new("hidden");
        layer.visible = false;
        scene.layers.push(layer);
        scene.groups.push(group);
        let cat = Catalog::new(&scene, revisions(), None).unwrap();
        let data = cat
            .collect(
                &cat.prototypes[0].key,
                &scene,
                &OrbitCamera::default(),
                &style(),
            )
            .unwrap();
        assert_eq!(data.indices.len(), 3);
        let points = data
            .indices
            .iter()
            .map(|&i| glam::Vec3::from_array(data.vertices[i as usize].position))
            .collect::<Vec<_>>();
        assert!((points[1] - points[0]).cross(points[2] - points[0]).z < 0.);
        scene.groups[0].layer = Some("hidden".into());
        let hidden = Catalog::new(&scene, revisions(), Some(&cat)).unwrap();
        assert_eq!(hidden.packet.placement_count(), 0);
        assert_eq!(cat.packet.placement_count(), 1);
    }

    #[test]
    fn nonrepresentative_classic_pose_invalidates_shared_world_geometry() {
        let mut scene = Scene::new();
        let mut first = Group::with_mesh("first", mesh());
        first.mesh_share = Some(first.id);
        let mut second = Group::with_mesh("second", first.mesh.clone());
        second.mesh_share = first.mesh_share;
        scene.groups = vec![first, second];
        let catalog = Catalog::new(&scene, revisions(), None).unwrap();
        assert_eq!(catalog.packet.prototype_count(), 1);
        scene.groups[1].transform = DMat4::from_translation(glam::DVec3::new(3., 0., 0.));
        assert!(catalog.classic_models_changed(&scene).unwrap());
        let rebuilt = Catalog::new(&scene, revisions(), Some(&catalog)).unwrap();
        assert_eq!(rebuilt.packet.prototype_count(), 2);
        assert!(rebuilt
            .prototypes
            .iter()
            .any(|p| p.key == catalog.prototypes[0].key));
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
    pub accepted_model_refreshes: u64,
    accepted_pose_revision: Option<u64>,
    preview: Option<Arc<super::world_preview::Preview>>,
    preview_poses: Vec<Pose>,
    #[cfg(target_os="macos")] base_lines:[Option<(Revisions,Option<u64>,Vec<usize>,Arc<super::world_preview::Lines>)>;2],
    #[cfg(target_os="macos")] preview_lines:Option<(ingetrazo_render::face_sweep::Key,Arc<super::world_preview::Lines>)>,
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
        self.prepare_scene_preview(metal,adapter,scene,revisions,pose_revision,camera,style,camera_matrix,extent,None,None)
    }

    pub fn prepare_scene_preview(
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
        preview: Option<ingetrazo_render::face_sweep::Input>,
        source_eye: Option<glam::DVec3>,
    ) -> Result<Option<Arc<crate::world::Prepared>>, String> {
        self.validate_context_owner(metal)?;
        if (style.show_edges && (style.show_soft_edges||style.back_edges_hidden)) || style.show_profiles || style.show_sky || style.show_grid {
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
        if scene.terrain.is_some()||!scene.geo_points.is_empty()||!scene.geo_paths.is_empty(){
            return Err("Metal Scene adapter needs geographic/terrain passes".into());
        }
        let preview_selection=preview.is_some_and(|p|scene.selection.iter().all(|item|matches!(item,ingetrazo_core::scene::SelectionItem::Face(id) if *id==p.face)));
        if (!scene.selection.is_empty() && !preview_selection) || !scene.guides.is_empty(){
            return Err("Metal Scene adapter needs selection/guide overlays".into());
        }
        use crate::world_policy::{Mode, Policy};
        use ingetrazo_render::VisualStyle;
        let mode = match style.visual_style {
            VisualStyle::ShadedWithTextures => if style.show_textures{Mode::Material}else{Mode::MaterialAverage},
            VisualStyle::HiddenLine => Mode::HiddenLine,
            VisualStyle::Monochrome => Mode::Monochrome,
                        _ => return Err("Metal Scene adapter wireframe/XRay passes not implemented".into()),
        };
        let pose_changed =
            pose_revision.is_none() || adapter.accepted_pose_revision != pose_revision;
        let cold = match adapter.catalog.as_ref() {
            None => true,
            Some(c) => {
                c.revisions != revisions || (pose_changed && c.classic_models_changed(scene)?)
            }
        };
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
        let preview_candidate = if let Some(input)=preview {
            let face=scene.mesh.faces.get(input.face).ok_or("Metal preview face disappeared")?;
            if input.mesh_revision!=scene.version || input.face_identity!=face.history_id{return Err("Metal preview acquisition changed".into());}
            let eye=source_eye.unwrap_or_else(||camera.eye());
            if adapter.preview.as_ref().is_some_and(|old|old.reusable(input,revisions.geometry,eye,&catalog.packet)) {None}
            else {Some(Arc::new(super::world_preview::Preview::prepare(scene,input,revisions.geometry,eye,&catalog.packet,catalog.prototypes.iter().find(|p|p.signature.loose).map(|p|p.key),adapter.preview.as_deref())?))}
        }else{None};
        let active_preview=preview.and_then(|_|preview_candidate.as_ref().or(adapter.preview.as_ref()));
        let packet=active_preview.map_or_else(||catalog.packet.clone(),|p|p.packet.clone());
        let base_models=if refresh {&adapter.scratch}else{&adapter.current};
        if let Some(p)=active_preview {
            let count=p.packet.placement_count();
            if refresh || adapter.preview_poses.len()!=count {
                adapter.preview_poses.try_reserve(count.saturating_sub(adapter.preview_poses.len())).map_err(|_|"Metal preview model capacity refused")?;
                adapter.preview_poses.clear();adapter.preview_poses.extend_from_slice(base_models);
                if count>base_models.len(){adapter.preview_poses.push(Pose{mvp:DMat4::IDENTITY.as_mat4().to_cols_array_2d(),reflected:false});}
            }
        }
        let base_keys=active_preview.and_then(|p|p.base).map_or([None,None],|(original,variant)|[Some(original),Some(variant)]);
        let base_pins=self.pin_bases(base_keys);
        let ghost_key=super::world_ghost::Key{revisions,poses:pose_revision,objects:scene.show_hidden_objects,geometry:scene.show_hidden_geometry,wire:false};
        let ghost_changed=!ghost_key.reusable(self.ghosts.as_ref().map(|g|g.key));
        let ghosts=if ghost_changed{Arc::new(super::world_ghost::Ghosts::prepare(metal,scene,ghost_key)?)}else{self.ghosts.as_ref().unwrap().clone()};
        let suppressed=active_preview.map_or(&[][..],|p|p.sweep.suppressed_edges.as_slice());
        let base_line_slot=usize::from(!suppressed.is_empty());
        let base_line_changed=style.show_edges && !adapter.base_lines[base_line_slot].as_ref().is_some_and(|(r,poses,ids,_)|*r==revisions && pose_revision.is_some()&&*poses==pose_revision&&ids==suppressed);
        let base_lines=if !style.show_edges {None}else if !base_line_changed {Some(adapter.base_lines[base_line_slot].as_ref().unwrap().3.clone())}else{
            let rows=ingetrazo_render::native_edges::build_hard_edges_suppressed(scene,suppressed);
            Some(Arc::new(super::world_preview::Lines::prepare(metal,&rows.positions)?))
        };
        let preview_line_changed=active_preview.is_some_and(|p|!adapter.preview_lines.as_ref().is_some_and(|(key,_)|*key==p.key));
        let preview_lines=match active_preview {
            None=>None,
            Some(_p) if !preview_line_changed=>Some(adapter.preview_lines.as_ref().unwrap().1.clone()),
            Some(p)=>Some(Arc::new(super::world_preview::Lines::prepare(metal,&p.sweep.lines)?)),
        };
        // Allocate the small cache membership list before GPU publication.
        let mut line_ids=Vec::new();if base_line_changed{line_ids.try_reserve_exact(suppressed.len()).map_err(|_|"Metal line membership allocation refused")?;line_ids.extend_from_slice(suppressed);}
        let mut frame = self.prepare_material_placements_with_overlay(
            metal,
            packet,
            if active_preview.is_some(){&adapter.preview_poses}else{base_models},
            camera_matrix,
            extent,
            style.background_color.map(f64::from),
            Policy {
                mode,
                front: style.front_color,
                back: style.back_color,
            },
            active_preview.filter(|p|p.sweep.kind==ingetrazo_render::face_sweep::Kind::Overlay).map(|p|p.preview_key),
            |key| {
                if let Some(p)=active_preview {
                    if *key==p.preview_key {return Ok(match p.sweep.kind {
                        ingetrazo_render::face_sweep::Kind::Overlay=>ingetrazo_render::pipeline::build_preview_mesh_surfaces(&p.sweep.mesh,scene),
                        ingetrazo_render::face_sweep::Kind::Prism=>{let rich=ordinary_surface_style(style);ingetrazo_render::pipeline::build_borrowed_mesh_world_surfaces(&p.sweep.mesh,scene,&rich)},
                    });}
                    if p.uses_variant && p.base.is_some_and(|pair|pair.1==*key) {let rich=ordinary_surface_style(style);return Ok(ingetrazo_render::pipeline::build_loose_world_surfaces_suppressed(scene,&rich,&p.sweep.suppressed));}
                }
                catalog.collect_surfaces(key, scene, camera, style)
            },
            |path|sampled_asset(scene,path),
        )?;
        if let Some(prepared)=frame.as_mut() {
            let prepared=Arc::get_mut(prepared).expect("New unpublished Metal frame is exclusive");
            prepared.lines=[base_lines.clone(),preview_lines.clone()];
            prepared.overlay=active_preview.filter(|p|p.sweep.kind==ingetrazo_render::face_sweep::Kind::Overlay).map(|p|p.preview_key);
            if base_line_changed{self.stats.base_line_uploads+=u64::from(base_lines.as_ref().unwrap().buffer.is_some());self.stats.base_line_upload_bytes+=base_lines.as_ref().unwrap().bytes as u64;adapter.base_lines[base_line_slot]=Some((revisions,pose_revision,line_ids,base_lines.as_ref().unwrap().clone()));}
            if preview_line_changed{self.stats.preview_line_uploads+=u64::from(preview_lines.as_ref().unwrap().buffer.is_some());self.stats.preview_line_upload_bytes+=preview_lines.as_ref().unwrap().bytes as u64;adapter.preview_lines=Some((active_preview.unwrap().key,preview_lines.as_ref().unwrap().clone()));}
            prepared.ghosts=Some(ghosts.clone());
            prepared.ghost_edge=style.edge_color;
            if ghost_changed {self.stats.ghost_builds+=1;self.stats.ghost_uploads+=ghosts.buffers.iter().filter(|b|b.is_some()).count() as u64;self.stats.ghost_upload_bytes+=(ghosts.bytes-ghosts.wire_count*24) as u64;self.stats.wire_uploads+=u64::from(ghosts.wire.is_some());self.stats.wire_upload_bytes+=(ghosts.wire_count*24) as u64;}
            self.ghosts=Some(ghosts);
            self.publish_bases(base_pins,base_keys);
            if let Some(p)=preview_candidate{self.stats.preview_builds+=1;adapter.preview=Some(p);}
            if preview.is_none() && cold{adapter.preview=None;}
            if let Some(c) = candidate {
                adapter.catalog = Some(c);
                adapter.catalog_builds += 1;
            }
            if let Some(poses) = candidate_current.take() {
                adapter.current = poses;
            }
            if refresh { std::mem::swap(&mut adapter.current, &mut adapter.scratch); adapter.accepted_model_refreshes+=1; }
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

#[cfg(test)] mod route_classification_tests {
    use super::*;
    #[test] fn missing_stage_is_distinct_from_invalid_data_and_native_refusal(){
        for reason in ["Metal placement-only adapter needs consolidated loose geometry stage","Metal placement translucent material requires alpha stage","Metal Scene adapter needs selection/guide overlays","Metal Scene adapter needs geographic/terrain passes"]{assert!(unsupported_stage(reason));}
        for reason in ["Metal container color invalid","Metal scene catalog cold metadata exceeds8MiB","World native texture size query unavailable","Placement path allocation refused","Metal placement composed matrix invalid"]{assert!(!unsupported_stage(reason));}
    }
    #[test]
    fn loose_own_mesh_is_one_identity_model_without_duplicate_groups() {
        fn revisions()->Revisions {Revisions{geometry:1,paint:1,style:1,layers:1,topology:1}}
        fn mesh()->ingetrazo_core::mesh::Mesh {let mut m=ingetrazo_core::mesh::Mesh::new();let v=[glam::DVec3::ZERO,glam::DVec3::X,glam::DVec3::Y].map(|p|m.add_vertex(p));m.add_face(v.to_vec(),vec![]).unwrap();m}
        let mut scene=Scene::new();scene.mesh=mesh();
        let mut g=Group::with_mesh("placed",mesh());
        g.mesh_share=Some(g.id);g.component_definition=Some(g.id);
        g.native_document=serde_json::json!({"proto":0,"xform":DMat4::IDENTITY.to_cols_array()});
        g.transform=DMat4::from_translation(glam::DVec3::new(3.,0.,0.));scene.groups.push(g);
        let catalog=Catalog::new(&scene,revisions(),None).unwrap();
        assert_eq!(catalog.slots.len(),2);assert_eq!(catalog.prototypes.len(),2);
        let mut models=Vec::new();catalog.refresh_models(&scene,revisions(),&mut models).unwrap();
        assert_eq!(models[0].mvp,DMat4::IDENTITY.as_mat4().to_cols_array_2d());
        assert_eq!(models[1].mvp,scene.groups[0].transform.as_mat4().to_cols_array_2d());
        let loose=catalog.prototypes.iter().find(|p|p.signature.loose).unwrap();
        let data=catalog.collect(&loose.key,&scene,&OrbitCamera::new(),&StyleConfig::default()).unwrap();
        let expected=build_loose_world_surfaces(&scene,&StyleConfig::default());
        assert_eq!(data.vertices.len(),expected.opaque_mesh.vertices.len());
        assert_eq!(data.indices,expected.opaque_mesh.indices);
        assert!(!data.indices.is_empty());
        let mut wire=Scene::new();let a=wire.mesh.add_vertex(glam::DVec3::ZERO);let b=wire.mesh.add_vertex(glam::DVec3::X);wire.mesh.add_edge(a,b);
        assert_eq!(Catalog::new(&wire,revisions(),None).unwrap().packet.placement_count(),0);
    }

}

#[cfg(test)]mod ghost_catalog_tests {
 use super::*;
 #[test]fn hidden_only_edge_does_not_enter_ordinary_prototype(){
  let mut s=Scene::new();let a=s.mesh.add_vertex(glam::DVec3::ZERO);let b=s.mesh.add_vertex(glam::DVec3::X);let e=s.mesh.add_edge(a,b).unwrap();s.mesh.edges[e].hidden=true;
  s.show_hidden_geometry=true;
  let c=Catalog::new(&s,Revisions::default(),None).unwrap();assert_eq!(c.packet.placement_count(),0);
  assert_eq!(ingetrazo_render::hidden_ghosts::Packet::build(&s).unwrap().edges.len(),2);
  s.show_hidden_geometry=false;assert!(ingetrazo_render::hidden_ghosts::Packet::build(&s).unwrap().edges.is_empty());
  s.mesh.edges[e].hidden=false;assert_eq!(Catalog::new(&s,Revisions::default(),Some(&c)).unwrap().packet.placement_count(),0);
 }
}
