# Source-degenerate face boundaries — standalone followup

Source fe06dac7, parent proper-ring stage1b8dc29e. Actual pinned original run_transactional/History corpus produces27 states (nine inputs, append/Undo/Redo) matching full vertices, sorted set-equivalent vertex adjacency, edges/radial entries, face outer/hole IDs and Undo/Redo stack counts. Inputs: empty/one/two/repeated/welded-close outer loops; empty/one/two-point single holes; empty outer with proper hole. No tolerance or normalized radial deduplication. Self-edge vertex membership is once, two-vertex boundary radial Face incidence is twice. Empty faces still own an Undo entry. Ordinary add_edge retains its source degenerate refusal; only face boundary linking permits self-edges.

Seven focused face controls pass, including self-edge mesh constructor/reload and free-edge rejection, prior six atomic ownership/admission/metadata controls with unsupported-face test now using nonfinite input.18 proper-ring source states remain exact after common self-edge handling change. Same validator rejects eight malformed/mutated controls. No VM or broad suite.

Commands inside ingetrazo-publish at tools/ai-interpreter-proof:
```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin face_degenerate
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo test -j2 --no-default-features --bin face_history face_owner
```
Worktree root:
```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-ai-degenerate-faces.py
python3 scripts/check-ai-degenerate-faces.py
```

Still OPEN: multiple-hole/nested-hole filtering and source exceptions, new Face attrs/UV/material mutations, all object handles/Python identity, full Group/_finish and five recipe helpers, complete Scene ownership, VM/native budgets and production integration. This is another prerequisite, not run_python completion. No graphics or invalid-renderable face acceptance claim follows from faithful document topology.
