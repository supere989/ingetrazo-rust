# Push/Pull Gate 2 bounded batch

Source pin and binary hashes: `provenance.json`. Final geometry: 22/26 strict controls pass. Four remaining failures are retained, not excluded: wedge inward original snapshot cannot enter the original manifold kernel; three Ctrl slanted/step cases are accepted by original weak closure but refused atomically by native strict solid admission. This evidence does not prove those live original objects invalid independently of serialization.

Ten focused module tests pass, including stale hover → layer lock → click refusal. Four real app-handler controls pass: closed group refusal; nested transformed shared context preview/cancel; Ctrl release; unit VCB/invalid VCB/local Undo/Redo/Finish/shared leaf publication/root Undo/Redo. No OS pointer or rendered gesture claim.

The geometric checker validates volumes/spatial differences for closed bodies and a weaker face-count/vertex footprint for open bodies; Full general oriented holes/metadata equality remains unproved. Closed checks additionally assert exact face counts. Both painted controls match oriented face/hole/attrs/interior inventories (coordinates at five decimals); reversed orientation, corrupted metadata and removed-face negatives reject. The final single-acquisition/layer followup is compiled by module tests; retained app binary is the preceding frozen source 1b11424c. Paint ownership is covered separately by a focused unit test including typed wall layers, cap opaque attrs, wall IFC and texture mapping. Qt float rounding is limited to authored/moved extrusion points; unrelated f64 coordinates are preserved. Whole collapse, tiny-distance no-op semantics and broad arbitrary normals remain open.

Replay inside the devbox from this worktree:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -p ingetrazo-tools --lib pushpull::tests -j2
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_gate2_fixture docs/audits/pushpull-gate2/source.json docs/audits/pushpull-gate2/native-final.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-pushpull-gate2.py docs/audits/pushpull-gate2/native-final.json docs/audits/pushpull-gate2/final-comparison.json
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_gate2_app_fixture docs/audits/pushpull-gate2/source.json docs/audits/pushpull-gate2/app-final.json
```

The strict geometry checker intentionally exits 1 until all four retained failures are resolved. Historical baseline/cap/rim/Qt artifacts retain earlier failures without tolerance changes. Source reconstruction recipes do not cover general GUI clamp behavior. The layer prerequisite is root-owned; see provenance.
