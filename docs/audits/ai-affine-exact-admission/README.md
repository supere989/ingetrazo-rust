# Preserve native double placement during Qt hydration

Actual PySide6 QMatrix4x4 translation input 0.1 becomes 0.10000000149011612, while 0.125 remains exact. A native DMat4 owner can legitimately contain the former f64 value, so silently casting it on import would change the visible document even on an unrelated script. Native hydration now requires every value to be finite and exactly roundtrip through f32; otherwise it refuses before any script/publication. Newly authored Qt constructor inputs still use actual Qt-style f32 conversion. Arbitrary native-double placement compatibility remains an explicit admission boundary, not silent normalization.

Focused actual app test PASS: native 0.1 and finite f32-range overflow both refuse an unrelated face-creation script, preserving complete Scene and empty Undo/Redo. One test only; no broad replay. Prior 11-state affine source proof remains retained at 7580d1e5 and exercises exactly representable Qt-origin matrices; it is not mislabeled as a new executable replay of this follow-up.

Command: CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j2 -p ingetrazo-gui --features engineering-python --lib native_double_placement_refuses_before_unrelated_publication -- --nocapture
