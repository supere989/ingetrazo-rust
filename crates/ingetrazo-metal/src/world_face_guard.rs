//! Original consolidated color-pass guard. Two distinct owners suffice because
//! at most one preview owner is excluded; no warm scene/page scan is necessary.
#[derive(Clone, Copy)]
pub(crate) struct FaceGuard<K>([Option<K>; 2]);
impl<K: Copy + Eq> FaceGuard<K> {
    pub(crate) fn new() -> Self {
        Self([None, None])
    }
    pub(crate) fn add(&mut self, key: K) {
        if self.0.contains(&Some(key)) {
            return;
        }
        if let Some(slot) = self.0.iter_mut().find(|s| s.is_none()) {
            *slot = Some(key);
        }
    }
    pub(crate) fn open(&self, overlay: Option<K>) -> bool {
        self.0.iter().flatten().any(|key| Some(*key) != overlay)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consolidated_gate_excludes_preview_but_not_other_owners() {
        let mut guard = FaceGuard::<u64>::new();
        assert!(!guard.open(None)); // all opaque fronts belong to separate fvcol
        guard.add(7);
        assert!(guard.open(None));
        assert!(!guard.open(Some(7)));
        guard.add(7);
        assert!(!guard.open(Some(7)));
        guard.add(8);
        assert!(guard.open(Some(7)));
        assert!(guard.open(Some(8)));
        guard.add(9);
        assert!(guard.open(Some(9)));
    }
    #[test]
    fn actual_source_bucket_counts_choose_identical_gate() {
        let source: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/audits/metal-face-sweep/consolidated-guard/original.json"
        ))
        .unwrap();
        for row in source["cases"].as_array().unwrap() {
            let mut guard = FaceGuard::new();
            if row["consolidated_vertices"].as_u64().unwrap() > 0 {
                guard.add(1u64);
            }
            let front = row["front_only_vertices"].as_u64().unwrap();
            assert_eq!(
                if guard.open(None) { front } else { 0 },
                row["front_only_submitted"].as_u64().unwrap()
            );
            assert_eq!(row["consolidated_vertices"].as_u64().unwrap() + front, 36);
        }
    }
}
