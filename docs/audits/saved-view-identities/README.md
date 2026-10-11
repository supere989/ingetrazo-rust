# Saved-view placement references

Frozen original `6be29fe` supplies four operative capture/recapture/apply cases: authored 16-char/arbitrary/UUID-spelling IDs, missing legacy IDs, duplicate literal IDs, and freshly generated original 16-char IDs. Nested placements include reflection/translation and shared prototype identity. Native uses actual IngeTrazoApp capture/update/rename/apply/delete and ordinary Undo/Redo; update/rename retain view history identity and panel deletion stays independent of geometry history. Existing Python-authored views recall on native admission and preserve their unknown fields.

Runtime changes: canonical UUID32 is the native key; other literal spellings have deterministic independent keys. Saved-view references remain authored literal strings. Capture/apply uses original dictionary insertion order and last placement for duplicate UIDs, with linear expected lookup rather than groups × hidden-IDs. For unchanged geometry, save materializes only missing group IDs actually referenced by a saved view, without regenerating the raw group tree. Unrelated UID-less ancestors/siblings and all literal fields remain unchanged. Unknown unresolved references survive; no lossy valid-case refusal is added.

Focused result: four exact source/native observation records, eight actual Python recalls of native files, native reopen/recall and app history assertions. Six negative controls invoke the same validators: four mask mutations, literal old missing-ID serializer output, and a lossy authored-ID rewrite. Raw group transforms, prototype indices and vendor metadata survive; source-generated 16-char IDs remain literal. Legacy independently random generated IDs are not equated across independent runtimes; resolution after actual reopen is checked instead.

Replay (no build required while frozen binary is available):
```
/tmp/ingetrazo-ai-registry-target/debug/examples/saved_view_identity_fixture /tmp/ingetrazo-saved-view-identity-proof > /tmp/ingetrazo-saved-view-identity-proof/native.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-saved-view-identities.py /tmp/ingetrazo-saved-view-identity-proof
```
Regenerate independent original inputs with `scripts/reference-saved-view-identities.py OUTPUT`. The checker performs actual pinned Python load and SavedView.apply, not only serialized equality. Original resource symlink is required for compilation. Named build: `CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build --locked -j2 -p ingetrazo-gui --example saved_view_identity_fixture`.

Boundaries: this proves placement-reference visibility recall, not rendered full-view fidelity. Hidden-shown ghost controls, nonstring UID/reference grammar, and unrelated command-owned visibility changes across model Undo require further full P13 acceptance. The loader's runtime UUID namespace is finite; this is not a general guarantee against adversarial hash/UUID collisions. Source/current saved-view unknown fields are retained. Scene snapshots and cold save traversal costs are unchanged; no whole-path sparse-memory claim.

Initial setup failures were missing resource symlink and missing fixture app_version; corrected without relaxing runtime admission. The intermediate generated-ID checker wrongly assumed original save retained arbitrary vendor keys; corrected to compare the actual authored input (original writer drops them). These were fixture/setup findings, not silently ignored failures.

Final manifest and native outputs remain under `/tmp/ingetrazo-saved-view-identity-proof` for root integration replay; do not purge until released.

## No-op preservation regression and correction

Root integration found the first implementation's broad `saved_views nonempty` condition regenerated unrelated UID-less groups, failing the existing no-op nested-value and embedded-byte preservation tests. The correction overlays only referenced generated IDs into the retained raw tree. Existing geometry-edit serialization is unchanged. All 17 tests in the single `native_preservation` target pass, including both originally failing tests; four-case/eight actual Python recalls/six-negative compact replay passes. The unchanged legacy capture explicitly checks that its referenced parent has a UID while its unreferenced child still has no UID. No assertion was weakened and no full suite was run.
