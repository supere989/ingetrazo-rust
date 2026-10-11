# Original Push/Pull inward-limit algorithm

Runtime `e7ac63de`, based on root `0d762417`. Reviewed runtime identity schema was copied verbatim as prerequisite `b72fa139`; root should skip it when full `cbc7fd24` is present. Separate current-batch identity seam `210740b6` copies tokens only for surviving source edges and preserves retained face clones; recreated edges receive fresh constructor identities.

## Source law and results

`tools/pushpull.py:_compute_inward_limit` (740–842) is ported as a single cold algorithm: attached rim classification, strict outer-loop punch exclusion, projected representative/vertex overlap for parallel blockers, immediate nonparallel outward escape, and inward ray samples against nonparallel noninterior boundaries with the original 1 mm hair-short margin. Hole rims participate in attachment/immediate-exit checks; original lateral sampling deliberately uses the outer ring. Projected containment deliberately ignores candidate holes by the source default. Sub-segment backing boundaries are included. The imported arrangement/triangulation/orient helper policies are mapped in `pushpull/inward_limit.rs`. Qt arithmetic is limited to this decision boundary; stored document coordinates remain f64. There is no per-pointer triangulation or limit scan: acquisition caches the result, and prelimited commits/trials reuse it.

Thirteen original decision cases were captured from live original tools once: cap, triangular zero-exit walls and end, wedge roof/walls, gable sampled exit, step mouth, unattached sheet, partial/disjoint parallel faces and Ctrl cap. Every limit, clamped preview distance and final status string matches exactly. Eleven final oriented face/holes/attrs/interior inventories match exactly, including the gable limit 3.353660982894973 and valid Ctrl endpoint top-out. Two additional earlier original controls (wider punchable sheet and annular support) are borrowed with their frozen source geometry/decisions; native acquisition and preview match. No numeric tolerance is introduced.

The source clamp also applies in Ctrl keep-base mode. The valid Ctrl full-cap commit initially fails its closed-solid admission, then the source `_mutate_or_top_out` performs six bisection trials and commits -2.953125, preserving an interior slab at z=0.046875. Native interactive commits now follow that bounded search for the specific Ctrl closed-solid admission refusal. The direct primitive still refuses the full invalid candidate atomically; that refusal and the successful app result are both observed in native.json. Trials snapshot only the active mesh and small selection/version state, restore each probe, and publish only the final model/history step. Generic non-Ctrl guard-triggered top-out and other source refusal categories remain outside this tested search closure.

Real app handler evidence covers pointer acquisition/move, preview/cancel, VCB, exact Undo/Redo, and nested transformed linked-context Finish with one folded document command. Two actual original on_value calls prove positive VCB follows the negative drag direction while a negative VCB reverses it; native results and messages match exactly. Pending tool statuses are emitted only by real clamps/top-out and drained by the app; reset/cancel clear tool state. The app's existing status owner retains the emitted string, without claiming the source's exact 4/5-second flash timing. This is actual app dispatch evidence, not OS-rendered gestures.

Eleven checker mutations reject missing inventory, changed lateral bound, lost zero exit, unclamped Ctrl, lost face, reversed cap, wrong interior, missing history, missing status, opaque attrs corruption and wrong VCB direction. Eleven focused PushPull unit checks and the separate token regression pass. All twelve earlier endpoint source comparisons and their app consumer pass against the changed runtime, including painted/hole/nested history controls. The prior source-invalid Ctrl cases remain covered by unchanged primitive atomic refusal checks.

## Required remaining differences

`comparison.json` intentionally sets full_pushpull_parity=false. In the unattached-sheet case, five far-cap/wall loops have opposite winding between source and native; flags are not normalized away. In the disjoint-parallel case, original global flat healing deletes every face while native preserves the collapsed cap and disconnected sheet. Both exact raw results are retained, with the native remaining payload pinned separately so a new difference cannot silently become an accepted mismatch. These are unresolved literal parity gaps, not completed behavior or automatic intentional corrections. Arbitrary precision/degenerate cases, reversed-input acquisition, generic top-out categories and the full original corpus/physical GUI remain unproved.

## Replay inside ingetrazo-publish

```sh
cd /tmp/ingetrazo-pushpull-partition-collapse
export CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -j2 -p ingetrazo-tools --lib pushpull
cargo test -j2 -p ingetrazo-tools --test pushpull_history_identity
cargo build -j2 -p ingetrazo-gui --example pushpull_inward_limits
$CARGO_TARGET_DIR/debug/examples/pushpull_inward_limits docs/audits/pushpull-inward-limits/source.json docs/audits/pushpull-inward-limits/native.json
python3 scripts/check-pushpull-inward-limits.py docs/audits/pushpull-inward-limits/source.json docs/audits/pushpull-inward-limits/native.json docs/audits/pushpull-inward-limits/source-vcb.json docs/audits/pushpull-inward-limits/remaining-native.json docs/audits/pushpull-inward-limits/comparison.json
```

Source regeneration uses the two reference scripts with the pinned original; the reused_limit_controls key is copied from the previously frozen endpoint source.json, not recaptured. Hashes identify the current scripts, runtime, binary and captured inputs/outputs. No original Python installation was modified, no broad suite/tracker edit/push occurred. Target and final outputs remain for root replay until released.

Final identity followup `a1b44be8` preserves the original remove/add ownership of partially punched mother faces: replacement faces receive fresh runtime tokens; protected survivor faces retain theirs. Both focused identity controls pass. The final inward app binary was rebuilt after this token-only seam.
