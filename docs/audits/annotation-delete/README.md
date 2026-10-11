# Annotation Delete acceptance

Runtime 62a7e45a. Actual pinned Select/Viewport/History compared with native App Delete/Undo/Redo/Finish/Save: 30 exact ordered identity/content states, 30 rejecting state mutations. Four complete native documents reopen through the original Python reader exactly; four lossy reopen controls reject. Five focused tests pass, covering locked/stale refusal, one mixed command, duplicate label identity/serde, nested ancestor Undo then Cancel, and native f64 preservation.

Run `annotation_delete_fixture DIR`, then `python3 scripts/check-annotation-delete.py DIR` and pinned PySide Python `scripts/check-annotation-delete-reopen.py DIR`. Extract records.zip to DIR; all entries pinned in manifest.json. Source capture uses real Viewport and SelectTool erase dispatcher; no source override or manual annotation hydration.

Delete is one existing snapshot command. TextLabel runtime identity is skipped in serde and semantic equality. Global annotation commands survive ancestor history while editing; geometry-changing placed-leaf Finish follows existing prototype history law. Annotation-only Finish retains unchanged prototype vertices. A narrow zero-face pure-removal source-origin seam applies Qt forward/inverse rounding only for exact-f32 prototype/frame; native f64 data retains its prior policy.

Retained failures include lifted selection and unchanged-leaf reopen rounding. Existing candidate/full snapshot cloning remains a cold cost. Shared-edge healing in mixed annotation Delete is explicitly refused before mutation; full source geometry erase classifier and wider annotation transforms remain separate requirements. No total allocator/VM heap bound or rendered interaction claim.
