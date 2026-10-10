# Scoped Qt native-paper tracking

This is the crates.io harfbuzz-sys 0.8.0 package (HarfBuzz8.4.0), retaining its Rust and native license notices. INGETRAZO-PROVENANCE.json pins original retained file bytes. Unused tools/test/example fonts are omitted; no new font redistributions.

Qt6.12.0 uses HarfBuzz14.5.0. Its upstream `hb-aat-layout-trak-table.hh` is retained under docs/audits/coretext-native-sixth/qt-hb-trak-source.hh, including the original license. The backport adds TrackTableEntry size interpolation/clamping and TrackData default-track interpolation verbatim (only method renames avoid replacing legacy paths). It uses HarfBuzz8.4's existing internal grapheme iteration; adds tracking to advances only with modern saturating arithmetic. It does not edit shaped clusters, offsets, Unicode normalization, substitutions, kerning, or other general shaping logic.

`ingetrazo_hb_font_set_qt_tracking` explicitly opts a mutable font into this policy and refuses immutable fonts or failed user-data admission. Native paper shaping calls it before immutable publication. The unmarked branch retains original source behavior, including legacy offset additions and integer rounding. This is a narrowly scoped source compatibility backport, not a full HarfBuzz14.5 upgrade or full Unicode version parity claim.

Focused in-process tests exercise actual hb_shape with synthetic trak tables: single nonzero/default tracks, multi-track interpolation, size interpolation/clamping, unchanged offsets and grapheme mark positioning, malformed/empty table safety, and immutable admission refusal. These controls do not replace required native macOS provider/Canvas/PDF replay.
