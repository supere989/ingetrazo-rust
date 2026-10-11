# Cursor-pinned camera navigation

Runtime: `56d899e2` then required modal correction `7c4bd85e`.
Base: `a70c5488`, with frozen P13 `fb648177` / `8e2cf740` prerequisites.
Pinned original: `6be29fe437a7cd992d4e079f71720821b25216b9`; actual Qt 6.12.0.

Seven literal original camera operations and six original viewport handler sessions
match exactly: all 18 target/angle/distance/pivot/acquisition states, without a
numeric tolerance. Six altered coordinate/angle/distance/count/pivot/inventory
controls fail the actual checker. The compiled original viewport dispatcher
negative has 14/18 mismatched camera states and fails that same checker. Its
camera helper/module remains the candidate; only the dispatcher is replaced by
the literal base source. This is an isolated compiled defect negative, not a
claim that the executable is an untouched historical full revision.

The production `render_viewport` is exercised on `IngeTrazoApp` with a declared
800×600 viewport and raw egui events. `native_gpu_viewport_enabled=true` skips
CPU compatibility painting; no GPU backend is attached in this protocol. Model revision, Scene.version, and both
history stacks remain unchanged. This proves actual viewport/app event routing;
it does not claim native GPU pixels or complete operating-system window/input.
The original uses unmodified actual Qt camera and viewport handlers on an unshown
widget, therefore its existing no-retained-frame ray-pick fallback. It is not a
painted-depth capture, and does not establish GPU depth-hit equivalence.

Implementation uses Qt float vector/quaternion boundaries, unwrapped double
angles, height-scaled incremental QPoint drag deltas, and `0.9 ** steps` focus
zoom with close-up retreat. One pivot is acquired per drag; wheel focus and
bounds/visible-ray index are revision-cached. The authoritative model revision
prevents snapshot-address/version cache reuse. No Scene clone or per-frame mesh
copy is introduced. MouseWheel has no position: previous raw ingress position is
advanced in event order, rather than reading frame-final hover. Stored source
`nav/invert_wheel` and `nav/invert_orbit_y` booleans are honored when present in
preserved preferences; this batch adds no preference UI.

Qt quaternion normalization dynamically rescales its x/y/z/w accumulation.
Replacing it with a max-first sum caused two measured ULP discrepancies; the
literal accumulation corrected them without tolerance changes. Algorithm basis:
Qt `qquaternion.cpp` and `qmath.h` (GPL-compatible Qt source), cross-checked against
the actual pinned application / Qt binary, rather than treating a current Qt
source branch as the pinned installed version.

Two core refusal/magnitude controls and four GUI ingress controls pass. Covered:
nonfinite/overflow refusal without camera mutation, close-up retreat, wheel
magnitude, unowned press, pixel-only wheel no-op, release outside, Escape,
wheel-before-move event ordering, and disabled/modal capture retirement. The
unconditional viewport hook runs its retirement guard while Text3D/material UI
is disabled. Ordinary Alt+primary tool input is preserved: literal source
mousePressEvent orbits on MMB, or primary only under explicit source nav_mode;
it has no generic Alt+primary orbit shortcut.

## Replay

`records.zip` contains every raw logical record/log; `manifest.json` records every
uncompressed digest, binary digest and exact negative dispatcher source. Verify
those digests, extract into an owned temporary directory, then run:

```sh
python3 scripts/check-cursor-navigation.py OUTPUT/original.json OUTPUT/restored-native.json
# Expected failure:
python3 scripts/check-cursor-navigation.py OUTPUT/original.json OUTPUT/old-dispatch-native.json
```

Fresh compiled consumer interface:

```sh
cursor_navigation_workflow OUTPUT/original.json FRESH_NATIVE_JSON
python3 scripts/check-cursor-navigation.py OUTPUT/original.json FRESH_NATIVE_JSON
```

Build once: `cargo build --locked -p ingetrazo-gui --example cursor_navigation_workflow -j2`,
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, existing owner target. Focused
checks: `cargo test -p ingetrazo-core --lib camera_cursor::tests -j2` and
`cargo test -p ingetrazo-gui --lib cursor_navigation::tests -j2`.

## Remaining scope

Actual retained-frame depth acquisition, platform-native wheel translation and
cursor image are unproved here. Existing saved-view/projection evidence remains
separate. No literal retained zoom-extents source session was located; native
fit_to tests or composer paper fit do not close that P16 acceptance requirement.
No P16 classification/tracker change is made. Temporary raw copies and frozen
binaries remain owned until parent archive review/release; reusable target and
source worktree are preserved.
