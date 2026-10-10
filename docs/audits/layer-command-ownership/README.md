# Layer command ownership: operative original baseline

`original.json` comes from `scripts/probe-python-layer-history.py`, pinned Python
6be29fe437a7cd992d4e079f71720821b25216b9, real LayersPanel callbacks and History
commands under Qt offscreen. This proves controller state, not rendered UI.

Four cases establish distinct ownership:
- Panel add/visibility/lock survive unrelated model Undo/Redo without adding history.
- AssignLayer-created registry removal uses the original target **name**, not stable
  object identity. Renaming it leaves the renamed layer on Undo; Redo creates the
  original target name again. Entity labels follow the command.
- Purge retains removed Layer objects. A visibility change after Undo survives
  another Redo/Undo, with original index restoration.
- SnapshotImport retains appended Layer objects. Deleting the imported layer in
  the panel leaves it absent on Undo, but Redo resurrects the renamed object.

Native source currently snapshots the whole registry for model Undo and its panel
minus button pops the last layer without reassigning tagged entities. Correction
and independent native comparison are **pending**. A blanket exclusion of layers
would lose Assign/Purge/Import ownership and is not an acceptable correction.

Default labels differ: original `Layer 0`, current native `Layer0`; any normalized
checker must name this representation boundary rather than silently hiding it.
No style/shadow/image-plane or full layer workflow completion is claimed.
