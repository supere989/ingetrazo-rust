# Existing face loop adopts an appended Vertex

Standalone prerequisite; production disabled. Pinned original `6be29fe437a7cd992d4e079f71720821b25216b9`.

Fresh original and genuine RustPython execution match six complete states for existing position edit plus face creation followed by assigning `held.loop[1] = added.loop[1]`. Exact VM `is` assertions prove the existing Face and added Face share the genuine appended Vertex, its held reference remains usable while removed from membership by Undo, and the same added Face/Vertex identities return on Redo. No identity inference from coordinates or ordinal booleans. Error/quota refusal preserves state/Redo; chronological Undo/Redo and final logical charge zero pass. Fourteen strict same-validator mutations reject.

The literal `2c620c00` VM controller compiles against this fixture and refuses the source-valid existing loop reference (exit101); its log is retained. Corrected source was restored. Native identity mapping covers the full current Vertex membership, while existing-field position projection covers only the admitted native prefix. Mixed preparation checks loop indices against the checked final vertex extent before publication; native existing-edge/radial records intentionally retain original direct-loop-edit semantics.

Commands:

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo run --manifest-path tools/ai-interpreter-proof/Cargo.toml --bin vm_mixed_references -j2
python scripts/check-ai-mixed-references.py docs/audits/ai-mixed-references/original.json docs/audits/ai-mixed-references/native.json
```

Full membership permutation/deletion/replacement, real Edge/Vertex incidence APIs, arbitrary attrs/callback atomicity, and physical VM/native budgets remain required. Controller snapshot history is still temporary O(root references × depth), not accepted production architecture. This evidence does not establish full Python or whole-document parity.
