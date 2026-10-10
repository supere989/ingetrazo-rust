# Fourth reviewed swarm milestone

Implementation base `c2fc7b4`; each evidence artifact records its exact checkout.

- 232 Rust workspace tests pass; CLI builds.
- Native document fields/assets remain preserved for four original engineering documents, with actual Python reopening.
- Corrected glTF/GLB Y-up export convention; external JSON, data-URI and GLB imports pass original geometry/UV and authored color/texture-byte checks. Earlier wrong-axis acceptance failure remains retained.
- Mixed clipboard cursor preview/cancel/commit/undo and original native geometry/UV checks pass. Desktop clipboard, overlap merge and cross-context workflows remain open.
- Saved-view selected update/rename/remove and actual Python recall pass for camera/style/section/nested visibility after fixing persisted identity references. Historical failure retained.
- Original dimension schema and32 cursor placement cases pass. Live anchors and deep outliner edits pass6 original reopen stages. Twelve nested transformed contexts compare world-axis/aligned measurements, line points and placement with original runtime, including reflection/nonuniform scale.
- True face intersections and planar rebuild/heal replace menu stubs. Independent original intersection/mutation and eight healing cases pass. Physical Pieces passes four original-runtime cases for touching boards, fragments, inherited names and patch attribution, including transformed world geometry and atomic undo/no-op. Orient Faces is implemented and unit-tested; broader P11 robustness remains open.
- Surviving vertex identities are retained across topology rebuilds and solid/tool edits. Parts sizes/materials and merged cutlists are implemented; full controls/corpus acceptance remains open.
- Recursive visible COLLADA export geometry uses shared hole-aware world triangulation. COLLADA import/material export and independent DAE acceptance remain open.
- Software-rendered textured startup remains a bounded virtual-display check, not complete rendered gesture/platform acceptance.

Published third milestone CI passed: [run38012356527](https://github.com/supere989/ingetrazo-rust/actions/runs/38012356527). This milestone cloud status is verified separately after publication.

No full feature row is Verified. All release gates in the tracker remain open.

Cloud run38013336872 failed the virtual-display smoke at the fixed five-second capture: retained [black screenshot](ci-milestone-04-render-baseline.png), empty startup log and still-live app process. Rust tests/build passed; later semantic CI steps were skipped. A bounded45-second rendered-frame readiness wait replaces fixed delay, retaining process liveness and the same nonempty-image criterion. Local readiness smoke passes; cloud correction is unverified until rerun.
