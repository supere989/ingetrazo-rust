# Editable command history prerequisite

Pinned original: `6be29fe437a7cd992d4e079f71720821b25216b9`.

Fresh genuine RustPython execution matches six complete original states: native positions/face rings, held Vertex/Face identity, replaced position/loop/attribute wrappers, shared nested-list mutation visibility, and actual Undo/Redo stack counts. Focused controls cover refused Redo under logical quota, failed execution preserving Redo, no-op preserving Redo, chronological append/field Undo/Redo, branch truncation, exhausted-quota empty Undo/Redo, and final zero logical charge. The checker rejects fourteen mutated inventories and the literal older native report without history.

Commands:

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo run --manifest-path tools/ai-interpreter-proof/Cargo.toml --bin vm_editable_history -j2
python scripts/check-ai-editable-history.py docs/audits/ai-editable-history/original.json docs/audits/ai-editable-history/native.json docs/audits/ai-existing-fields/native.json
```

This remains a standalone prerequisite. Production execution is disabled. Immutable before/after templates retain O(root VM references × history depth); native spatial-index preparation remains O(root). This is temporary, not the production architecture. Mixed existing-field edits plus append and membership changes remain required but currently refuse atomically. Arbitrary attribute equality can execute callbacks; the geometry validation guard does not establish attribute/history callback atomicity. General native reentry/finalizers, complete attribute codec projection, multi-owner history, VM/native memory and wall-time bounds remain open. Logical charges exclude compiler/VM allocator internals and capacity slack before reconciliation; no physical peak-memory cap is claimed.
