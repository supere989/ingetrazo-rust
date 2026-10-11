# Authored perspective guide acquisition

The missing P07 guide producer/camera contract passes in runtime `ae96e2028c17cf5a307ca5037f6b1a867230325f`, with test-only initializer correction `6bdbe7ea`. Five actual camera modes (ordinary perspective, close perspective, two-point perspective, steep fallback and parallel) cover 35 cases at original nine/fourteen logical-pixel boundaries. Actual Protractor transactions create fresh diagonal guides; the original production camera, clipping, snap-scene, unprojection and integer-event context handlers are observed without product edits. Fifty-five actual App states and five actual egui acquisition sessions continue into Line numeric dispatch, Cancel, Undo and Redo.

The strict checker accepts exact guide directions, all camera matrix coefficients and clipped endpoints, the existing 1e-6 snap-position bound and five-decimal mesh/history records. Seven corrupted outputs are rejected. The old cast-after-double camera differs in X/Y/W coefficients in all 35 cases. The first comparison's 30 failures remain retained: it exposed original ray/callback arithmetic differences and a consumer label-classification bug, both corrected without changing the oracle or tolerance.

Source-authorable guide construction now normalizes once with the original float arithmetic. Native-double guide inputs retain double normalization and coordinates. Inference uses the existing accepted Qt camera implementation, moved from GUI navigation into core and re-exported under its prior GUI API. No second camera implementation remains. A source ray is unprojected lazily, at most once per snap query, and reused by line projections; ordinary render/pick cameras and native-double geometry retain their paths. Original-compatible line projection and clamp arithmetic applies only to source-authorable inputs.

The affected retained 48-guide/286-numeric/56-App-state corpus and eight negatives pass once with these binaries. The three moved camera admission/unprojection tests and existing native-double projection guard pass (four tests). Initial build took 18.551s; the correction and two-consumer build took 11.906s. New and retained semantic execution/checking took 0.229s total. The first attempted unit compilation lacked a new field in an old test initializer; the test-only correction took 3.86s to compile. No timing is a production performance claim.

`source-records.zip` contains the one completed new original capture, authenticated by `source-manifest.json`. `records.zip` retains initial failures, final outputs, logs, exact binary identities and Qt reference source consulted during diagnosis; `manifest.json` binds every member plus the final Git source files. Every member was reopened and byte-compared. The original product remains pinned at `6be29fe437a7cd992d4e079f71720821b25216b9`.

```sh
EXAMPLES/perspective_guide_workflows SOURCE/source.json OUTPUT/native.json
python3 scripts/check-perspective-guides.py SOURCE/source.json OUTPUT/native.json OUTPUT/check.json
```

[Independent read-only review](independent-review.json) accepts the bounded perspective-guide contract; combined integration remains pending. P07 remains Implemented until the separate keyboard-buffer work and full unchanged criterion review are accepted. No complete Protractor/Tape, whole-frame, OS-pointer or platform acceptance is inferred.
