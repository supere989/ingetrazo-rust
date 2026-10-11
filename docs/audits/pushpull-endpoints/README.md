# Push/Pull endpoint batch

Runtime: `4d3a7436` → `874c7921` → `3e85e292`, based on `dc1ee0cf`; UUID feature prerequisite `9f8be1b2` is the cherry of root-owned `df9ad4ad` and is not part of this handoff.
Original pin: `6be29fe437a7cd992d4e079f71720821b25216b9`.

## Results and source law

The pinned original tool's `_MIN_EXTRUDE=2e-4`, `_clamp_extrusion`, `_prism_cap` endpoint, flat exception in `_commit`, and new-vertex paint propagation in `_mutate` are the authority. Zero/subminimum pointer commits retain acquisition; subminimum VCB is rejected without model/history mutation. Clean prism caps clamp at the opposing cap, permit the whole flat surface endpoint, and remember the clamped value for repeated operation. A raised bump pushed flush removes its collapsed walls and gives its cap paint ownership to the restored plane. Existing hole-loop orientation remains authored; newly cut neighbor faces receive paint only when they carry new vertices.

`comparison.json` records twelve required original/native cases with exact oriented boundary rings, holes, face counts, attrs and interior flags. Closed cases additionally use the existing native original manifold spatial difference oracle (unchanged threshold 1e-6; observed differences zero). Flat cases are compared directly and are not passed through a volume oracle. Six corrupted result controls reject empty collapse, duplicate flat cap, lost hole, lost paint, unclamped endpoint and reversed boundary. The wider parallel sheet is excluded by the original strict containment rule (loop_inside_face, candidate outer ring); annular support checks include the selected hole-loop vertices. These two source controls compare exact clamped preview distances without committing unrelated sheet geometry. Original results are captured from live tool objects before JSON serialization; clamped distance is recorded before commit resets transient state.

`app.json` records thirteen application-consumer controls plus two source acquisition-limit controls. They invoke actual pointer handlers, VCB, preview/cancel, Undo/Redo and nested Finish on `IngeTrazoApp`; they are not OS-rendered gesture evidence. The nested case uses a literal integer affine parent placement, linked leaf instances, clamped full collapse, repeat from the flat face, two local Undo steps and one folded document Undo step. An untouched f64 orphan vertex and invalid VCB refusal are checked. Eleven focused Push/Pull unit tests retain prior invalid Ctrl and locked-layer controls and add protected opposing subdivision refusal.

## Remaining boundaries

This implements clean-prism parallel-cap limits, not the entire original lateral/nonparallel blocker classifier. Full collapse onto a subdivided opposing surface currently refuses atomically, preserving its geometry and opaque metadata; original flat healing/partition ownership remains required. The previous three original-invalid Ctrl/wedge cases and internal-wall winding discrepancy remain unchanged and open. Arbitrary noninteger transformed placement rounding, general stepped/slanted blockers, actual rendered gestures and broad Gate 2 completion are not proved here.

## Retained development failures

Initial build refused `Uuid::new_v5` because the core crate did not declare the feature; root supplied the existing dependency fix. The first app fixture used the outer-ring centroid for a holed cap, which lies inside the void and legitimately missed acquisition. The corrected fixture uses an actual triangulation interior point. Earlier source comparisons exposed a seven-face flush result versus the original six, old authored hole winding changes, and missing paint on newly cut neighbors; the final strict comparison closes those cases. These are reported as development failures, not accepted discrepancies. No tolerance was widened.

## Replay (inside ingetrazo-publish)

```sh
cd /tmp/ingetrazo-pushpull-endpoints
export CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
/tmp/ingetrazo-parity-python/bin/python scripts/reference-pushpull-endpoints.py docs/audits/pushpull-endpoints/source.json
cargo test -j2 -p ingetrazo-tools --lib pushpull::tests
cargo build -j2 -p ingetrazo-tools --example pushpull_gate2_fixture -p ingetrazo-gui --example pushpull_endpoints_app
$CARGO_TARGET_DIR/debug/examples/pushpull_gate2_fixture docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-endpoints/native.json
$CARGO_TARGET_DIR/debug/examples/pushpull_endpoints_app docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-endpoints/app.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-pushpull-endpoints.py docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-endpoints/native.json docs/audits/pushpull-endpoints/comparison.json docs/audits/pushpull-endpoints/app.json
```

Binary, source and output hashes are retained in `provenance.json`. Final outputs and exclusive target remain available for root replay until released. No broad suite, tracker change or push was performed.
