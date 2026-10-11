//! Immutable operator admission: warm snapshots only check retained Arc identity.
use ingetrazo_render::{cad2d::Batch, cad_callback::GlyphOperator};
use std::{collections::HashMap, sync::Arc};
#[derive(Default)]
pub(crate) struct Cache {
    owners: HashMap<(usize, usize), (Arc<[Batch]>, Arc<[GlyphOperator]>)>,
    pub scans: u64,
}
impl Cache {
    pub fn admit(
        &mut self,
        batches: &Arc<[Batch]>,
        glyphs: Option<&Arc<[GlyphOperator]>>,
    ) -> Result<(), String> {
        let Some(glyphs) = glyphs else { return Ok(()) };
        if glyphs.len() != batches.len() {
            return Err("Metal glyph batch inventory mismatch".into());
        }
        let key = (
            Arc::as_ptr(batches) as *const Batch as usize,
            Arc::as_ptr(glyphs) as *const GlyphOperator as usize,
        );
        if self.owners.contains_key(&key) {
            return Ok(());
        }
        for glyph in glyphs.iter() {
            glyph.validate()?;
        }
        self.owners
            .try_reserve(1)
            .map_err(|_| "Metal glyph admission cache allocation failed")?;
        if self.owners.len() >= 64 {
            self.owners.clear();
        }
        self.owners.insert(key, (batches.clone(), glyphs.clone()));
        self.scans += 1;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_warm_inventory_skips_scan_but_new_untrusted_arc_refuses() {
        let batch: Arc<[Batch]> = vec![Batch {
            first: 0,
            count: 0,
            color: ingetrazo_render::cad2d::EncodedRgba([0.; 4]),
        }]
        .into();
        let good: Arc<[GlyphOperator]> = vec![GlyphOperator {
            composition: 1,
            straight_rgba: [1, 2, 3, 255],
        }]
        .into();
        let mut cache = Cache::default();
        cache.admit(&batch, Some(&good)).unwrap();
        for _ in 0..2000 {
            cache.admit(&batch, Some(&good)).unwrap();
        }
        assert_eq!(cache.scans, 1);
        let bad: Arc<[GlyphOperator]> = vec![GlyphOperator {
            composition: 3,
            straight_rgba: [0; 4],
        }]
        .into();
        assert!(cache.admit(&batch, Some(&bad)).is_err());
        assert_eq!(cache.scans, 1);
        let empty: Arc<[GlyphOperator]> = vec![].into();
        assert!(cache.admit(&batch, Some(&empty)).is_err());
        cache.admit(&batch, Some(&good)).unwrap();
        assert_eq!(cache.scans, 1);
    }
}
