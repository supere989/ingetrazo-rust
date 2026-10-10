# Independent same-pass integer CAD blend

Frozen32197c4 produces all nine exact whole RGBA images on actual RTX5090. Literal old float-blend negatives remain: rounded4, ellipse10, polygon5 differing pixels. Five actual hardware lifecycle cases pass two resizes: baseline, callback-only, callback-error, real vertex/index allocation-pressure rollback/retry, uniform pose/color reuse. Managed nearest RGB textures and later UI painting remain intact.

Transport retains immutable image ID, executable/helper SHA, bounded command and owned-client capture. No desktop capture, extra image/pass, or CPU image upload. Source-ordered range eligibility rejects overlap/malformed metadata; unsupported input usage and f32 intermediate overflow refuse before preparation. Mode is cold-selected and stable through resize. Warm geometry uploads unchanged.

Recheck scripts/check-integer-cad-shape-pixels.py original9 hardware9/shapes/capture --old-float old-float9 --output RESULT. Lifecycle checker uses flattened case capture directories plus docs/audits/native-cad2d-inline/original-reference.json.

Opt-in NVIDIA/Linux bounded proof only. Thin cosmetic one-pixel coverage failure, full sheets, Metal, broader UI AA and performance/platform acceptance remain OPEN.
