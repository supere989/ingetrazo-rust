# Terrain visibility and command ownership

Source4d666db, actual original pinned6be29fe437a7cd992d4e079f71720821b25216b9. Existing source ScenesPanel/Viewport/path command oracle now matches all bounded presentation states including ordinary terrain visibility. No rendered UI/native terrain drawing claim.

Loaded-terrain visibility is a runtime presentation edit, not model History. Public set_terrain_visibility performs no-op checks, preserves legitimate Redo, changes only visibility in existing identity-held objects and captured command objects, and adds no Undo entry. Loaded-terrain checkbox and saved-view recall route that method. Runtime publication uses publish_runtime_terrain and seals the pending command before replacing its display cache.

Original StraightenModelCommand owns clearing terrain and retains its previous object. Six actual original states demonstrate why bool-only ownership is insufficient: command clears terrain, later runtime rebuild creates hidden z9 cache, Undo restores old visible z1, Redo repeats None, and a second hidden z5 rebuild cannot become the command's Redo result. Native app snapshot clear exercises that terrain ownership footprint; it is NOT implementation/acceptance of the complete StraightenModel geometry/geodetic command. Literal bool-only source variant compiles/runs and wrongly resurrects z9/z5; strict checker rejects it. Stale-visibility baseline retained in saved-view-command-ownership; four payload negatives reject changes.

Runtime terrain ID/command metadata are skipped in IGZ. Source reference identity is modeled explicitly; same-coordinate fresh rebuild remains a distinct cache object. Ownership is sealed only at cold command/nonhistory boundaries. Unowned Undo/Redo transfers the live terrain allocation, excluding it from inverse Scene cloning. Captured owned-after geometry uses Arc to share the held immutable snapshot across history clones; held visibility is separate and changes without cloning geometry. An actual owned resource capture/instantiation still copies CPU vertex/UV/index arrays at cold History boundaries due to the current value-based TerrainObject model. No zero-copy whole-history/performance claim; shared terrain geometry resource redesign remains a memory optimization gate. No per-frame geometry copy/pass added.

Full workspace and prior annotation/global context protocols pass. P15 broader command families, native DEM/tile fetch/terrain rendering, full StraightenModel, image planes/style/shadows/richer units/cutlist and Metal remain open.

Replay inside publish worktree:

```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-presentation-history.py /tmp/original-terrain.json
CARGO_TARGET_DIR=/tmp/isolated-terrain-target cargo run --locked -p ingetrazo-gui --example presentation_history_fixture -- /tmp/native-terrain
python3 scripts/check-terrain-command-ownership.py /tmp/original-terrain.json /tmp/native-terrain/native.json docs/audits/saved-view-command-ownership/native.json docs/audits/terrain-command-ownership/bool-only-negative.json
```
