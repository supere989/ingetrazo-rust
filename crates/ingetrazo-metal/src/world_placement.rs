//! Immutable local prototype identities/bindings and separately admitted poses.
//! Source children are independent bindings. Identity is authored/prototype
//! identity, never a geometric equality hash. No Scene ownership or traversal.
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PrototypeKey {
    pub identity: u128,
    pub geometry_revision: u64,
    pub paint_revision: u64,
    pub style_revision: u64,
    pub layer_revision: u64,
}
#[derive(Debug)]
pub struct Packet {
    pub(crate) prototypes: Vec<PrototypeKey>,
    pub(crate) bindings: Vec<u32>,
    retained_payload_bytes: usize,
}
const MAX_BYTES: usize = 8 * 1024 * 1024;
impl Packet {
    /// Moves admitted descriptors. No prototype vertices or Scene copy here.
    pub fn new(prototypes: Vec<PrototypeKey>, bindings: Vec<u32>) -> Result<Arc<Self>, String> {
        let retained_payload_bytes = prototypes
            .capacity()
            .checked_mul(std::mem::size_of::<PrototypeKey>())
            .and_then(|n| {
                bindings
                    .capacity()
                    .checked_mul(4)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or("Metal placement descriptors exceed8MiB capacity admission")?;
        if bindings.iter().any(|i| *i as usize >= prototypes.len()) {
            return Err("Metal placement prototype binding invalid".into());
        }
        let temporary = prototypes
            .len()
            .checked_mul(std::mem::size_of::<PrototypeKey>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<PrototypeKey>>()))
            .ok_or("Metal placement sorting capacity overflow")?;
        if retained_payload_bytes
            .checked_add(temporary)
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err("Metal placement cold descriptors exceed8MiB payload admission".into());
        }
        let mut identities = Vec::new();
        identities
            .try_reserve_exact(prototypes.len())
            .map_err(|_| "Metal placement identity allocation refused")?;
        let actual = identities
            .capacity()
            .checked_mul(std::mem::size_of::<PrototypeKey>())
            .and_then(|n| n.checked_add(std::mem::size_of_val(&identities)))
            .and_then(|n| n.checked_add(retained_payload_bytes))
            .ok_or("Metal placement sorting capacity overflow")?;
        if actual > MAX_BYTES {
            return Err("Metal placement actual cold capacities exceed8MiB".into());
        }
        identities.extend_from_slice(&prototypes);
        identities.sort_unstable();
        if identities.windows(2).any(|p| p[0] == p[1]) {
            return Err("Metal placement duplicate prototype key".into());
        }
        Ok(Arc::new(Self {
            prototypes,
            bindings,
            retained_payload_bytes,
        }))
    }
    pub fn prototype_count(&self) -> usize {
        self.prototypes.len()
    }
    pub fn placement_count(&self) -> usize {
        self.bindings.len()
    }
    /// Descriptor Vec capacities only; excludes allocator and cold uniqueness sort.
    pub fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub mvp: [[f32; 4]; 4],
    /// Original instancing switches winding for reflected placements; it does
    /// not rebuild or upload a reversed copy of the static local prototype.
    pub reflected: bool,
}
pub(crate) const MAX_POSE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) fn validate_poses(packet: &Packet, poses: &[Pose]) -> Result<(), String> {
    if poses
        .len()
        .checked_mul(std::mem::size_of::<Pose>())
        .is_none_or(|n| n > MAX_POSE_BYTES)
        || poses.len() != packet.bindings.len()
        || poses
            .iter()
            .any(|p| p.mvp.iter().flatten().any(|x| !x.is_finite()))
    {
        return Err("Metal placement pose count/matrix invalid".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(id: u128) -> PrototypeKey {
        PrototypeKey {
            identity: id,
            geometry_revision: 1,
            paint_revision: 2,
            style_revision: 3,
            layer_revision: 4,
        }
    }
    #[test]
    fn shared_prototype_and_separate_children_are_bindings_not_geometry_copies() {
        let p = Packet::new(vec![key(1), key(2)], vec![0, 0, 1]).unwrap();
        assert_eq!(p.prototype_count(), 2);
        assert_eq!(p.placement_count(), 3);
        assert!(
            p.retained_payload_bytes()
                >= std::mem::size_of::<Packet>() + 2 * std::mem::size_of::<PrototypeKey>() + 12
        );
    }
    #[test]
    fn malformed_bindings_duplicates_and_oversized_capacity_refuse() {
        assert!(Packet::new(vec![key(1)], vec![1]).is_err());
        assert!(Packet::new(vec![key(1), key(1)], vec![]).is_err());
        let huge = Vec::with_capacity(MAX_BYTES / 4 + 1);
        assert!(Packet::new(vec![], huge).is_err());
        let mut changed = key(1);
        changed.paint_revision += 1;
        assert!(Packet::new(vec![key(1), changed], vec![0, 1]).is_ok());
    }
    #[test]
    fn unchanged_descriptor_does_not_skip_pose_admission() {
        let p = Packet::new(vec![key(1)], vec![0]).unwrap();
        let mut pose = Pose {
            mvp: [[0.; 4]; 4],
            reflected: false,
        };
        assert!(validate_poses(&p, &[pose]).is_ok());
        pose.mvp[0][0] = f32::NAN;
        assert!(validate_poses(&p, &[pose]).is_err());
        assert!(validate_poses(&p, &[]).is_err());
    }
}
