//! Separate cached ghost ownership; never enters ordinary prototypes or casters.
use ingetrazo_render::scene_revision::Revisions;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Key {
    pub revisions: Revisions,
    pub poses: Option<u64>,
    pub objects: bool,
    pub geometry: bool,
    pub wire: bool,
}
impl Key {
    pub(crate) fn reusable(self, old: Option<Self>) -> bool {
        self.poses.is_some() && old == Some(self)
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Uniform {
    pub edge: [f32; 4],
    pub height: u32,
    pub mode: u32,
    pub padding: [u32; 2],
}
#[cfg(target_os = "macos")]
pub(crate) struct Ghosts {
    pub key: Key,
    pub buffers: [Option<crate::Buffer>; 3],
    pub counts: [usize; 3],
    pub bytes: usize,
    pub wire: Option<crate::Buffer>,
    pub wire_count: usize,
}
#[cfg(target_os = "macos")]
impl Ghosts {
    pub fn prepare(
        metal: &crate::NativeMetal,
        scene: &ingetrazo_core::scene::Scene,
        key: Key,
    ) -> Result<Self, String> {
        let packet = ingetrazo_render::hidden_ghosts::Packet::build(scene)?;
        let rows = [&packet.faces, &packet.textured, &packet.edges];
        let mut buffers = [None, None, None];
        let mut counts = [0; 3];
        let mut bytes = 0usize;
        // All candidates are private until the whole world frame has been accepted.
        for (i, rows) in rows.into_iter().enumerate() {
            counts[i] = rows.len();
            let data = bytemuck::cast_slice(rows);
            bytes = bytes
                .checked_add(data.len())
                .ok_or("Metal ghost byte overflow")?;
            if !data.is_empty() {
                let mut b = metal.buffer(data.len(), true)?;
                b.write(data)?;
                buffers[i] = Some(b);
            }
        }
        let mut wire_rows = Vec::<ingetrazo_render::hidden_ghosts::Vertex>::new();
        if key.wire {
            for e in &scene.mesh.edges {
                if e.hidden || e.soft || !scene.is_layer_visible(e.layer.as_deref()) {
                    continue;
                }
                wire_rows
                    .try_reserve(2)
                    .map_err(|_| "Metal wire packet admission")?;
                for id in [e.v0, e.v1] {
                    let p = scene.mesh.vertices[id].position.as_vec3();
                    if !p.is_finite() {
                        return Err("Metal wire position overflow".into());
                    }
                    wire_rows.push(ingetrazo_render::hidden_ghosts::Vertex {
                        position: p.to_array(),
                        color: [1.; 3],
                    });
                }
            }
        }
        let wire_count = wire_rows.len();
        let data = bytemuck::cast_slice(&wire_rows);
        bytes = bytes
            .checked_add(data.len())
            .ok_or("Metal wire bytes overflow")?;
        let wire = if data.is_empty() {
            None
        } else {
            let mut b = metal.buffer(data.len(), true)?;
            b.write(data)?;
            Some(b)
        };
        Ok(Self {
            key,
            wire,
            wire_count,
            buffers,
            counts,
            bytes,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_layout_and_revision_admission() {
        assert_eq!(
            std::mem::size_of::<ingetrazo_render::hidden_ghosts::Vertex>(),
            24
        );
        assert_eq!(std::mem::size_of::<Uniform>(), 32);
        let key = Key {
            revisions: Revisions::default(),
            poses: Some(7),
            objects: true,
            geometry: false,
            wire: false,
        };
        assert!(key.reusable(Some(key)));
        for changed in [
            Key {
                objects: false,
                ..key
            },
            Key {
                geometry: true,
                ..key
            },
            Key {
                poses: Some(8),
                ..key
            },
            Key { poses: None, ..key },
            Key {
                revisions: Revisions {
                    paint: 1,
                    ..key.revisions
                },
                ..key
            },
        ] {
            assert!(!changed.reusable(Some(key)));
        }
    }
}
