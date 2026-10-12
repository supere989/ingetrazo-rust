# Integrated spline import and portable curve math

Reviewed runtime `c0c7a9f39d2dd0087a212d098ff331a080fc27e0` combines the frozen DXF spline implementation and the portable curve-math correction with the published composer runtime. The original Python checkout remains unchanged at `6be29fe437a7cd992d4e079f71720821b25216b9`.

One affected `context_document_lifecycle` build took **8.577 seconds**. Its three DXF modes, original-reader reopens and strict checks took **7.538 seconds**:

| Workflow | Cases | Exact application states | Original-reader reopens | Rejected mutations |
| --- | ---: | ---: | ---: | ---: |
| Legacy polylines | 8 | 56 | 8 | 16 |
| Circles, arcs, ellipses and bulges | 12 | 84 | 12 | 36 |
| Splines | 15 | 105 | 15 | 45 |
| Total | 35 | 245 | 35 | 97 |

All three modes also pass their document/history/precision guards; the spline invalid-only control passes. Native binary SHA-256: `4795f07877398b9c34bb7f3ae77ea1b9cb9f1e13b0ca9fb1b6396332cc3522af`. Runtime crates tree: `fe575ff04e8f53b4638c3e57ca9455b98717a9d7`.

The integration reused the same checkout and target, so ordinary Cargo invalidation rebuilt affected code without discarding dependencies. No source corpus was recaptured, no unrelated composer check was repeated, and no broad local suite ran. CI adds one spline mode to the existing authenticated context consumer; its shared example build is unchanged.

Published `08726369` exposed an actual native ARM and Intel macOS curve-test failure: a platform trigonometric result changed a tiny bulge endpoint residue. The correction pins the already-locked `libm 0.2.16` and forces software math for the six construction helpers. Existing point fixtures and arithmetic order remain unchanged. The owner passed five focused tests, including all twelve curve and fifteen spline paths and a literal intermediate trace. This combined application proof now covers the corrected runtime. **Corrected native macOS execution is still pending.** The older Mac rendering gates remain open and were skipped after the dependency-test failure in `08726369`.

P22 remains Implemented. INSERT/MINSERT blocks, broader face fusion and exchange acceptance remain open. The larger banded fit-spline branch is source-derived but outside this finite corpus. No overall parity, platform release, GPU-performance or total-memory claim is made.

`receipt.json` retains exact commands, durations, binary/source bindings and outcomes. `records.zip` and `manifest.json` retain 98 new output/log/settings records (13,712,544 raw bytes) and authenticate them against the completed run. The 122 unchanged input records (9,768,848 bytes) remain in their existing source archives and are hash-referenced instead of copied into this archive. The replay script is included.

[Owner spline evidence](../dxf-splines/README.md), [independent review](../dxf-splines/independent-review.json), [portable math evidence](../dxf-curves/portable-math-manifest.json), [previous composer integration](../dxf-composer-integration/README.md).
