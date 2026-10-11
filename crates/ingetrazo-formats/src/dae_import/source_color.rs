//! Original missing-image tint, without a Python runtime or file IO.
//! CPython 3.13 uses SipHash13 over compact native-endian Unicode code units.
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;

fn keys_for_seed(mut seed: u32) -> (u64, u64) {
    let mut bytes = [0_u8; 16];
    if seed != 0 {
        for byte in &mut bytes {
            seed = seed.wrapping_mul(214013).wrapping_add(2531011);
            *byte = (seed >> 16) as u8;
        }
    }
    (u64::from_le_bytes(bytes[..8].try_into().unwrap()),
     u64::from_le_bytes(bytes[8..].try_into().unwrap()))
}
fn process_keys() -> (u64, u64) {
    static KEYS: OnceLock<(u64, u64)> = OnceLock::new();
    *KEYS.get_or_init(|| {
        if let Some(seed) = std::env::var("PYTHONHASHSEED").ok().and_then(|s| s.parse::<u32>().ok()) {
            return keys_for_seed(seed);
        }
        // The original also chooses one random key per process when unseeded.
        // This randomness affects only missing-image display colors.
        let state = std::collections::hash_map::RandomState::new();
        let mut first = state.build_hasher(); first.write_u8(0);
        let mut second = state.build_hasher(); second.write_u8(1);
        (first.finish(), second.finish())
    })
}
fn text_hash(text: &str, keys: (u64, u64)) -> i64 {
    if text.is_empty() { return 0; }
    let width = match text.chars().map(u32::from).max().unwrap() {
        0..=255 => 1, 256..=65535 => 2, _ => 4,
    };
    let mut hash = siphasher::sip::SipHasher13::new_with_keys(keys.0, keys.1);
    for ch in text.chars() {
        match width {
            1 => hash.write(&[ch as u8]),
            2 => hash.write(&(ch as u16).to_ne_bytes()),
            _ => hash.write(&(ch as u32).to_ne_bytes()),
        }
    }
    let value = hash.finish() as i64;
    if value == -1 { -2 } else { value }
}
pub(super) fn missing_image_color(reference: &str) -> Option<[f64; 3]> {
    if reference.is_empty() { return None; }
    let hash = text_hash(reference, process_keys());
    Some([0, 4, 8].map(|shift| 0.78 + ((hash >> shift) & 15) as f64 / 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_python_ascii_and_unicode_hash_vectors() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!("../../../../docs/audits/dae-material-resolution/hash-source.json")).unwrap();
        for cohort in corpus.as_array().unwrap() {
            let seed = cohort["seed"].as_u64().unwrap() as u32;
            for row in cohort["rows"].as_array().unwrap() {
                let text = row["text"].as_str().unwrap();
                assert_eq!(text_hash(text, keys_for_seed(seed)), row["hash"].as_i64().unwrap(), "seed={seed} text={text}");
            }
        }
    }
}
