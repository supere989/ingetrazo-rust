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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Stable model matrix in prepare_placements_with_camera. Historical field
    /// name is retained for the combined-matrix compatibility entry.
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

/// Only adjacent equal prototype/winding instances coalesce; no depth-order sort.
#[cfg(test)]
pub(crate) fn compatible_run_end(bindings: &[u32], poses: &[Pose], first: usize) -> usize {
    let mut end = first + 1;
    while end < poses.len()
        && bindings[end] == bindings[first]
        && poses[end].reflected == poses[first].reflected
    {
        end += 1;
    }
    end
}
#[cfg(test)]
#[test]
fn instance_runs_preserve_prototype_winding_and_original_order() {
    let pose = |reflected| Pose {
        mvp: [[0.; 4]; 4],
        reflected,
    };
    let poses = [
        pose(false),
        pose(false),
        pose(true),
        pose(true),
        pose(true),
        pose(false),
    ];
    let bindings = [0, 0, 0, 1, 1, 0];
    let mut first = 0;
    let mut runs = Vec::new();
    while first < poses.len() {
        let end = compatible_run_end(&bindings, &poses, first);
        runs.push((first, end));
        first = end;
    }
    assert_eq!(runs, vec![(0, 2), (2, 3), (3, 5), (5, 6)]);
}

/// Frozen source first-seen (prototype, mirrored) buckets; original placement
/// order within each bucket. Only bindings/winding changes rebuild this plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    pub prototype: u32,
    pub reflected: bool,
    pub first: usize,
    pub count: usize,
}
#[derive(Debug)]
pub(crate) struct Order {
    pub indices: Vec<usize>,
    pub runs: Vec<Run>,
    pub retained_payload_bytes: usize,
}
impl Order {
    pub fn new(packet: &Packet, poses: &[Pose]) -> Result<Self, String> {
        validate_poses(packet, poses)?;
        #[derive(Clone, Copy)]
        struct Record {
            prototype: u32,
            reflected: bool,
            original: usize,
            first: usize,
        }
        let n = poses.len();
        let cold = n
            .checked_mul(
                std::mem::size_of::<Record>()
                    + std::mem::size_of::<usize>()
                    + std::mem::size_of::<Run>(),
            )
            .and_then(|v| {
                v.checked_add(std::mem::size_of::<Self>() + std::mem::size_of::<Vec<Record>>())
            })
            .filter(|v| *v <= MAX_BYTES)
            .ok_or("Metal prototype ordering exceeds8MiB cold admission")?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(n)
            .map_err(|_| "Metal prototype ordering allocation refused")?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(n)
            .map_err(|_| "Metal instance order allocation refused")?;
        records.extend(packet.bindings.iter().zip(poses).enumerate().map(
            |(i, (&prototype, p))| Record {
                prototype,
                reflected: p.reflected,
                original: i,
                first: i,
            },
        ));
        records.sort_unstable_by_key(|r| (r.prototype, r.reflected, r.original));
        let mut start = 0;
        let mut groups = 0;
        while start < n {
            let mut end = start + 1;
            while end < n
                && records[end].prototype == records[start].prototype
                && records[end].reflected == records[start].reflected
            {
                end += 1;
            }
            let first = records[start].original;
            for r in &mut records[start..end] {
                r.first = first;
            }
            groups += 1;
            start = end;
        }
        let mut runs: Vec<Run> = Vec::new();
        runs.try_reserve_exact(groups)
            .map_err(|_| "Metal instance runs allocation refused")?;
        let retained = indices
            .capacity()
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|a| {
                runs.capacity()
                    .checked_mul(std::mem::size_of::<Run>())
                    .and_then(|b| a.checked_add(b))
            })
            .and_then(|a| a.checked_add(std::mem::size_of::<Self>()))
            .ok_or("Metal order capacity overflow")?;
        let actual = records
            .capacity()
            .checked_mul(std::mem::size_of::<Record>())
            .and_then(|a| a.checked_add(retained + std::mem::size_of::<Vec<Record>>()))
            .ok_or("Metal cold order capacity overflow")?;
        if actual > cold || actual > MAX_BYTES {
            return Err("Metal actual order capacity exceeds cold admission".into());
        }
        records.sort_unstable_by_key(|r| (r.first, r.original));
        for r in records {
            indices.push(r.original);
            if let Some(last) = runs.last_mut() {
                if last.prototype == r.prototype && last.reflected == r.reflected {
                    last.count += 1;
                    continue;
                }
            }
            runs.push(Run {
                prototype: r.prototype,
                reflected: r.reflected,
                first: indices.len() - 1,
                count: 1,
            });
        }
        Ok(Self {
            indices,
            runs,
            retained_payload_bytes: retained,
        })
    }
}
#[cfg(test)]
#[test]
fn source_global_order_preserves_first_bucket_and_original_members() {
    let key = |identity| PrototypeKey {
        identity,
        geometry_revision: 1,
        paint_revision: 1,
        style_revision: 1,
        layer_revision: 1,
    };
    let packet = Packet::new(vec![key(1), key(2)], vec![0, 1, 0, 0, 1, 0]).unwrap();
    let pose = |reflected| Pose {
        mvp: [[0.; 4]; 4],
        reflected,
    };
    let poses = [
        pose(false),
        pose(false),
        pose(false),
        pose(true),
        pose(false),
        pose(true),
    ];
    let order = Order::new(&packet, &poses).unwrap();
    assert_eq!(order.indices, vec![0, 2, 1, 4, 3, 5]);
    assert_eq!(
        order.runs,
        vec![
            Run {
                prototype: 0,
                reflected: false,
                first: 0,
                count: 2
            },
            Run {
                prototype: 1,
                reflected: false,
                first: 2,
                count: 2
            },
            Run {
                prototype: 0,
                reflected: true,
                first: 4,
                count: 2
            }
        ]
    );
    assert!(order.retained_payload_bytes > 0);
}

#[cfg(test)]
#[test]
fn source_order_allocates_runs_for_groups_and_rejects_invalid_poses() {
    let key = PrototypeKey {
        identity: 1,
        geometry_revision: 1,
        paint_revision: 1,
        style_revision: 1,
        layer_revision: 1,
    };
    let packet = Packet::new(vec![key], vec![0; 1000]).unwrap();
    let mut poses = vec![
        Pose {
            mvp: [[0.; 4]; 4],
            reflected: false
        };
        1000
    ];
    let order = Order::new(&packet, &poses).unwrap();
    assert_eq!(order.runs.len(), 1);
    assert_eq!(order.runs.capacity(), 1);
    assert_eq!(order.indices.len(), 1000);
    poses[500].mvp[0][0] = f32::NAN;
    assert!(Order::new(&packet, &poses).is_err());
    assert_eq!(order.runs[0].count, 1000);
}

/// Winding is draw state; compare only the matrix occupying the old packed slot.
pub(crate) fn packed_matrix_changed(
    old_order: Option<&Order>,
    old_poses: &[Pose],
    packed: usize,
    next: &Pose,
) -> bool {
    old_order
        .and_then(|o| o.indices.get(packed))
        .and_then(|&i| old_poses.get(i))
        .is_none_or(|old| old.mvp != next.mvp)
}
#[cfg(test)]
#[test]
fn packed_update_tracks_matrix_slots_through_reordering() {
    let order = Order {
        indices: vec![0, 2, 1],
        runs: vec![],
        retained_payload_bytes: 0,
    };
    let mut p = [Pose {
        mvp: [[0.; 4]; 4],
        reflected: false,
    }; 3];
    p[0].mvp[3][0] = 1.;
    p[1].mvp[3][0] = 2.;
    p[2].mvp[3][0] = 3.;
    let mut moved = p[2];
    moved.mvp[3][0] += 0.6;
    let changed: Vec<_> = [p[0], moved, p[1]]
        .iter()
        .enumerate()
        .filter(|(i, x)| packed_matrix_changed(Some(&order), &p, *i, x))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(changed, vec![1]);
    assert!(packed_matrix_changed(Some(&order), &p, 1, &p[1]));
    let mut mirror = p[2];
    mirror.reflected = true;
    assert!(!packed_matrix_changed(Some(&order), &p, 1, &mirror));
    assert!(packed_matrix_changed(None, &p, 1, &p[2]));
    assert!(packed_matrix_changed(Some(&order), &[], 1, &p[2]));
}
