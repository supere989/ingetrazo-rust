# Locked-layer picking and panel pruning

Pinned Python source: `6be29fe437a7cd992d4e079f71720821b25216b9`.

56 states use the actual Python `MainWindow`, `Viewport.pick_face/pick_edge/pick_group`, and `LayersPanel` checkbox signals in an unshown Qt widget. Four states (open, lock, hide, restore) cover 14 scenes: missing/empty/default/named/unknown tags, overlapping front/rear faces, loose edges, group and child ownership, transformed nesting, line groups and internal face tags. `source-inputs.zip` retains all source IGZs; native loads each open scene and executes its real layer commands before querying the same recorded rays. Exact picking, pruned selection and visible-face flags match. Source vertex observations are retained as context, not claimed as native vertex acceptance: source loose-vertex acquisition deliberately ignores layer selectability. Group picking follows placement layers; internal face tags do not gate the closed object in the source.

The native application dispatcher additionally checks Select acquisition/pruning, locked Push/Pull refusing to mutate or create history, unlock/commit, Undo/Redo, and preview Cancel. This is executable command/picker evidence, not pointer-event or screenshot acceptance. Hidden-geometry mode, box selection, annotation picking, and full layer/saved-view workflow acceptance remain separate work.

Replay: unpack the archive into a temporary directory and copy `original.json` there; run `layer_picking_fixture INPUT_DIR OUTPUT_JSON`, copy output as `native.json`, then run `python3 scripts/check-layer-picking.py EVIDENCE_DIR`. The checker compares 56 states and rejects six altered outputs.
