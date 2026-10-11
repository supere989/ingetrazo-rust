//! Backend-neutral immutable CAD callback data. This producer owns no GPU
//! allocations and is not rendering acceptance: a native painter must admit,
//! prepare and retain a frozen snapshot until actual command completion.
use crate::cad2d::{Batch, Geometry, Pose};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// Exact glyph source operator, ordered one per draw batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphOperator {
    pub composition: u32,
    pub straight_rgba: [u8; 4],
}
impl GlyphOperator {
    pub fn validate(self) -> Result<(), String> {
        match self.composition & 255 {
            0 if self.composition == 0 => Ok(()),
            1 if self.composition == 1 => Ok(()),
            2 => Ok(()),
            _ => Err("Unsupported CAD glyph operator".into()),
        }
    }
}
#[derive(Clone)]
pub struct Snapshot {
    pub geometry: Arc<Geometry>,
    pub batches: Arc<[Batch]>,
    pub pose: Pose,
    pub revision: u64,
    pub glyphs: Option<Arc<[GlyphOperator]>>,
}
#[derive(Default)]
struct Counters {
    handles: AtomicU64,
    updates: AtomicU64,
    unchanged: AtomicU64,
    snapshots: AtomicU64,
}
#[derive(Clone, Default)]
pub struct Factory {
    counters: Arc<Counters>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metrics {
    pub handles: u64,
    pub updates: u64,
    pub unchanged: u64,
    pub snapshots: u64,
}
impl Factory {
    /// Retained callbacks keep this owner alive, so a cached factory identity
    /// cannot be recycled while any tile can still reference its handles.
    pub fn cache_identity(&self) -> usize {
        Arc::as_ptr(&self.counters) as usize
    }

    pub fn handle(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<Arc<Callback>, String> {
        geometry.validate_batches(&batches)?;
        pose.validate()?;
        let handle = Arc::new(Callback {
            state: Mutex::new(Snapshot {
                geometry,
                batches,
                pose,
                revision: 1,
                glyphs: None,
            }),
            counters: self.counters.clone(),
        });
        self.counters.handles.fetch_add(1, Ordering::Relaxed);
        Ok(handle)
    }
    pub fn glyph_handle(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
        glyphs: Arc<[GlyphOperator]>,
    ) -> Result<Arc<Callback>, String> {
        validate_glyphs(&batches, &glyphs)?;
        let handle = self.handle(geometry, batches, pose)?;
        handle
            .state
            .lock()
            .map_err(|_| "CAD producer state poisoned")?
            .glyphs = Some(glyphs);
        Ok(handle)
    }
    pub fn metrics(&self) -> Metrics {
        Metrics {
            handles: self.counters.handles.load(Ordering::Relaxed),
            updates: self.counters.updates.load(Ordering::Relaxed),
            unchanged: self.counters.unchanged.load(Ordering::Relaxed),
            snapshots: self.counters.snapshots.load(Ordering::Relaxed),
        }
    }
}
fn validate_glyphs(batches: &[Batch], glyphs: &[GlyphOperator]) -> Result<(), String> {
    if batches.len() != glyphs.len() {
        return Err("CAD glyph batch inventory mismatch".into());
    }
    for glyph in glyphs {
        glyph.validate()?;
    }
    Ok(())
}
/// Stable callback Arc; updates never clone geometry, indices or palette bytes.
/// Prepared native paint must retain the snapshot, not this mutable producer.
pub struct Callback {
    state: Mutex<Snapshot>,
    counters: Arc<Counters>,
}
impl Callback {
    /// Cold ownership accounting: the callback record owns only shared Arc
    /// references. Geometry, batches and the shared counter owner are separate.
    /// Allocator metadata and native consuming GPU caches are not payload bytes.
    pub fn retained_cpu_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    pub fn update(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
    ) -> Result<(), String> {
        pose.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "CAD producer state poisoned")?;
        if Arc::ptr_eq(&state.geometry, &geometry)
            && Arc::ptr_eq(&state.batches, &batches)
            && state.pose == pose
            && state.glyphs.is_none()
        {
            self.counters.unchanged.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        if !Arc::ptr_eq(&state.geometry, &geometry) || !Arc::ptr_eq(&state.batches, &batches) {
            geometry.validate_batches(&batches)?;
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or("CAD producer revision exhausted")?;
        *state = Snapshot {
            geometry,
            batches,
            pose,
            revision,
            glyphs: None,
        };
        self.counters.updates.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn update_glyphs(
        &self,
        geometry: Arc<Geometry>,
        batches: Arc<[Batch]>,
        pose: Pose,
        glyphs: Arc<[GlyphOperator]>,
    ) -> Result<(), String> {
        validate_glyphs(&batches, &glyphs)?;
        geometry.validate_batches(&batches)?;
        pose.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| "CAD producer state poisoned")?;
        if Arc::ptr_eq(&state.geometry, &geometry)
            && Arc::ptr_eq(&state.batches, &batches)
            && state.pose == pose
            && state
                .glyphs
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(old, &glyphs))
        {
            return Ok(());
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or("CAD producer revision exhausted")?;
        *state = Snapshot {
            geometry,
            batches,
            pose,
            revision,
            glyphs: Some(glyphs),
        };
        self.counters.updates.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let snapshot = self
            .state
            .lock()
            .map_err(|_| "CAD producer state poisoned")?
            .clone();
        self.counters.snapshots.fetch_add(1, Ordering::Relaxed);
        Ok(snapshot)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad2d::{EncodedRgba, Vertex};
    #[test]
    fn frozen_owner_survives_edits_and_invalid_update_is_atomic() {
        let geometry = Geometry::new(
            vec![
                Vertex {
                    position: [0., 0.],
                    coverage: 1.,
                },
                Vertex {
                    position: [2., 0.],
                    coverage: 1.,
                },
                Vertex {
                    position: [0., 2.],
                    coverage: 1.,
                },
            ],
            vec![0, 1, 2],
        )
        .unwrap();
        let batches: Arc<[Batch]> = vec![Batch {
            first: 0,
            count: 3,
            color: EncodedRgba::from_bytes([37, 93, 157, 128], 1.).unwrap(),
        }]
        .into();
        let factory = Factory::default();
        let handle = factory
            .handle(geometry.clone(), batches.clone(), Pose::default())
            .unwrap();
        let frozen = handle.snapshot().unwrap();
        for _ in 0..20 {
            handle
                .update(geometry.clone(), batches.clone(), Pose::default())
                .unwrap();
        }
        assert_eq!(factory.metrics().updates, 0);
        assert_eq!(factory.metrics().unchanged, 20);
        let moved = Pose {
            translation: [5., 7.],
            ..Default::default()
        };
        handle
            .update(geometry.clone(), batches.clone(), moved)
            .unwrap();
        assert_eq!(frozen.pose, Pose::default());
        let current = handle.snapshot().unwrap();
        assert_eq!(current.pose, moved);
        assert_eq!(current.revision, 2);
        assert!(Arc::ptr_eq(&frozen.geometry, &current.geometry));
        assert!(Arc::ptr_eq(&frozen.batches, &current.batches));
        assert!(handle
            .update(
                geometry,
                batches,
                Pose {
                    scale: [f32::NAN, 1.],
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(handle.snapshot().unwrap().revision, 2);
        assert_eq!(factory.metrics().updates, 1);
        let before = handle.snapshot().unwrap();
        let bad_bounds: Arc<[Batch]> = vec![Batch {
            first: 3,
            count: 3,
            color: before.batches[0].color,
        }]
        .into();
        let bad_color: Arc<[Batch]> = vec![Batch {
            first: 0,
            count: 3,
            color: EncodedRgba([f32::NAN, 0., 0., 1.]),
        }]
        .into();
        for rejected in [bad_bounds, bad_color] {
            assert!(handle
                .update(before.geometry.clone(), rejected, moved)
                .is_err());
            let after = handle.snapshot().unwrap();
            assert_eq!(after.revision, before.revision);
            assert_eq!(after.pose, before.pose);
            assert!(Arc::ptr_eq(&after.geometry, &before.geometry));
            assert!(Arc::ptr_eq(&after.batches, &before.batches));
        }
        let replacement: Arc<[Batch]> = vec![Batch {
            first: 0,
            count: 3,
            color: EncodedRgba::from_bytes([211, 17, 43, 255], 1.).unwrap(),
        }]
        .into();
        handle
            .update(before.geometry.clone(), replacement.clone(), moved)
            .unwrap();
        let repainted = handle.snapshot().unwrap();
        assert_eq!(repainted.revision, 3);
        assert!(Arc::ptr_eq(&repainted.geometry, &before.geometry));
        assert!(Arc::ptr_eq(&repainted.batches, &replacement));
        assert_ne!(before.batches[0].color, repainted.batches[0].color);
        assert_eq!(frozen.batches[0].color, before.batches[0].color);
        assert_eq!(factory.metrics().updates, 2);
        drop(handle);
        assert_eq!(frozen.geometry.indices().len(), 3);
    }
}

#[cfg(test)]
mod glyph_admission_tests {
    use super::*;
    #[test]
    fn glyph_operator_inventory_rejects_unknown_and_missing_before_publication() {
        let batches = [Batch {
            first: 0,
            count: 0,
            color: crate::cad2d::EncodedRgba([0.; 4]),
        }];
        assert!(validate_glyphs(&batches, &[]).is_err());
        assert!(GlyphOperator {
            composition: 3,
            straight_rgba: [0; 4]
        }
        .validate()
        .is_err());
        assert!(GlyphOperator {
            composition: 257,
            straight_rgba: [0; 4]
        }
        .validate()
        .is_err());
        for composition in [0, 1, 2 | (17 << 8) | (31 << 16) | (255 << 24)] {
            assert!(validate_glyphs(
                &batches,
                &[GlyphOperator {
                    composition,
                    straight_rgba: [15, 20, 27, 255]
                }]
            )
            .is_ok());
        }
    }
}
