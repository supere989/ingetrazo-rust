//! Cold preview packet descriptors. Native buffers are owned by World/Prepared.
use crate::world_placement::{Packet, PrototypeKey};
use glam::DVec3;
use ingetrazo_core::scene::Scene;
use ingetrazo_render::face_sweep::{Input, Key, Packet as SweepPacket};
use std::sync::Arc;

pub(crate) struct Preview {
    pub key: Key,
    pub sweep: SweepPacket,
    pub packet: Arc<Packet>,
    pub base_packet: Arc<Packet>,
    pub preview_key: PrototypeKey,
    pub base: Option<(PrototypeKey, PrototypeKey)>,
    pub base_ids: Vec<usize>,
    pub uses_variant: bool,
    pub generation: u64,
    pub namespace: [u128; 2],
}
impl Preview {
    pub fn reusable(&self, input: Input, publication: u64, eye: DVec3, base: &Arc<Packet>) -> bool {
        Arc::ptr_eq(&self.base_packet, base) && self.key == input.key(publication, eye)
    }
    pub fn prepare(
        scene: &Scene,
        input: Input,
        publication: u64,
        eye: DVec3,
        base: &Arc<Packet>,
        loose: Option<PrototypeKey>,
        old: Option<&Self>,
    ) -> Result<Self, String> {
        let sweep = SweepPacket::build(scene, input, eye)?;
        let generation = old.map_or(Ok(1), |old| {
            old.generation
                .checked_add(1)
                .ok_or("Metal preview generation exhausted")
        })?;
        // Runtime cache namespaces, never authored/persisted document IDs.
        let namespace = old.map(|old| old.namespace).unwrap_or_else(|| {
            [
                ingetrazo_core::groups::Group::new("").id.as_u128(),
                ingetrazo_core::groups::Group::new("").id.as_u128(),
            ]
        });
        let mut keys = Vec::new();
        keys.try_reserve_exact(
            base.prototypes
                .len()
                .checked_add(1)
                .ok_or("Metal preview descriptor count overflow")?,
        )
        .map_err(|_| "Metal preview descriptors refused")?;
        keys.extend_from_slice(&base.prototypes);
        let uses_variant = !sweep.suppressed.is_empty();
        let previous = old.filter(|old| {
            old.key.input.face_identity == input.face_identity
                && old.key.input.mesh_revision == input.mesh_revision
                && old.sweep.kind == sweep.kind
        });
        let base_variant = base
            .prototypes
            .iter()
            .position(|key| Some(*key) == loose)
            .and_then(|index| {
                let original = keys[index];
                let prior = previous
                    .and_then(|old| old.base)
                    .filter(|pair| pair.0 == original);
                if !uses_variant {
                    return prior;
                }
                let variant = prior
                    .filter(|_| previous.is_some_and(|old| old.base_ids == sweep.suppressed))
                    .map(|pair| pair.1)
                    .unwrap_or(PrototypeKey {
                        identity: namespace[0],
                        geometry_revision: generation,
                        ..original
                    });
                keys[index] = variant;
                Some((original, variant))
            });
        let base_ids = if uses_variant {
            sweep.suppressed.clone()
        } else {
            previous.map_or_else(Vec::new, |old| old.base_ids.clone())
        };
        // Camera-side suppression changes metadata, not cap/wall coordinates.
        let preview_key = old.filter(|old| old.key.input==input && old.key.publication==publication && Arc::ptr_eq(&old.base_packet,base)).map(|old|old.preview_key).unwrap_or(PrototypeKey {
            identity: namespace[1],
            geometry_revision: generation,
            paint_revision: publication,
            style_revision: 0,
            layer_revision: 0,
        });
        let index =
            u32::try_from(keys.len()).map_err(|_| "Metal preview prototype index overflow")?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(
                base.bindings
                    .len()
                    .checked_add(1)
                    .ok_or("Metal preview binding count overflow")?,
            )
            .map_err(|_| "Metal preview bindings refused")?;
        bindings.extend_from_slice(&base.bindings);
        if !sweep.mesh.faces.is_empty() {
            keys.push(preview_key);
            bindings.push(index);
        }
        Ok(Self {
            key: input.key(publication, eye),
            sweep,
            packet: Packet::new(keys, bindings)?,
            base_packet: base.clone(),
            preview_key,
            base: base_variant,
            base_ids,
            uses_variant,
            generation,
            namespace,
        })
    }
}

#[cfg(target_os = "macos")]
pub(crate) struct Lines {
    pub buffer: Option<crate::Buffer>,
    pub count: usize,
    pub bytes: usize,
}
#[cfg(target_os = "macos")]
impl Lines {
    pub fn prepare(metal: &crate::NativeMetal, positions: &[[f32; 3]]) -> Result<Self, String> {
        if positions.len() % 2 != 0 || positions.iter().flatten().any(|x| !x.is_finite()) {
            return Err("Metal preview line endpoints invalid".into());
        }
        let bytes = positions
            .len()
            .checked_mul(12)
            .ok_or("Metal line extent overflow")?;
        if bytes > 64 * 1024 * 1024 {
            return Err("Metal line packet admission exceeds64MiB".into());
        }
        let buffer = if bytes == 0 {
            None
        } else {
            let mut b = metal.buffer(bytes, true)?;
            b.write(bytemuck::cast_slice(positions))?;
            Some(b)
        };
        Ok(Self {
            buffer,
            count: positions.len(),
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Scene, Input, Arc<Packet>, PrototypeKey) {
        let mut scene = Scene::new();
        let ids = [DVec3::ZERO, DVec3::X, DVec3::Y].map(|p| scene.mesh.add_vertex(p));
        let face = scene.mesh.add_face(ids.to_vec(), vec![]).unwrap();
        let input = Input {
            face,
            face_identity: scene.mesh.faces[face].history_id,
            mesh_revision: scene.version,
            anchor: DVec3::new(0.3, 0.3, 0.),
            normal: DVec3::Z,
            offset: DVec3::Z,
            attached: true,
            prism_cap: false,
            keep_base: false,
        };
        let key = PrototypeKey {
            identity: 41,
            geometry_revision: 1,
            paint_revision: 1,
            style_revision: 1,
            layer_revision: 1,
        };
        (scene, input, Packet::new(vec![key], vec![0]).unwrap(), key)
    }
    #[test]
    fn offset_reuses_base_variant_and_cancel_retains_original_packet() {
        let (scene, input, base, key) = setup();
        let eye = DVec3::new(0., 0., -5.);
        let a = Preview::prepare(&scene, input, 1, eye, &base, Some(key), None).unwrap();
        assert!(a.uses_variant);
        assert!(a.reusable(input, 1, eye, &base));
        assert!(Arc::ptr_eq(&a.base_packet, &base));
        let moved = Input {
            offset: DVec3::Z * 1.5,
            ..input
        };
        let b = Preview::prepare(&scene, moved, 1, eye, &base, Some(key), Some(&a)).unwrap();
        assert_eq!(a.base, b.base);
        assert_ne!(a.preview_key, b.preview_key);
        assert_eq!(b.packet.placement_count(), 2);
        let c = Preview::prepare(&scene, moved, 1, -eye, &base, Some(key), Some(&b)).unwrap();
        assert!(!c.uses_variant);
        assert_eq!(c.base, b.base);
        assert_eq!(c.preview_key,b.preview_key);
        assert_eq!(c.packet.prototypes[0], key);
        assert!(Preview::prepare(
            &scene,
            Input {
                mesh_revision: scene.version + 1,
                ..input
            },
            1,
            eye,
            &base,
            Some(key),
            Some(&a)
        )
        .is_err());
        assert!(a.reusable(input, 1, eye, &base));
        assert_eq!(base.prototypes, vec![key]);
    }
}
