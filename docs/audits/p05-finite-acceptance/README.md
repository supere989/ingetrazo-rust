# Production finite mesh and triangulation acceptance

The unchanged P05 criterion is satisfied by this production extension plus the [20-case topology corpus](../p05-boundary-incidence/README.md). [Independent disposition](../p05-criterion-review/README.md) records the combined scope. This is finite geometry acceptance, not universal or rendered parity.

Production `02bf8dfb` retains empty/one/two-point outer and short-hole records through actual Mesh and IGZ APIs. Nine constructed/save/load topologies match the pinned original; all nine native files reopen in the actual original plain reader with the same topology. Twenty-seven actual App Undo/Redo states match the unchanged retained history oracle. The welded-close reader encounter order is compared with actual source loading, separately from source construction. Invalid vertex IDs refuse before publication. Empty sibling faces refuse Push/Pull before orientation/history changes; they remain stored. Short grouped faces retain source profile normal/centroid behavior; the focused native CPU publication guard passes.

All sixteen finite Face/direct triangulation cases pass independent invariants. Degenerate and short-hole output stays literal. Nondegenerate triangles must use authored planar corners, preserve winding and triangle counts, cover exact rational outer-minus-hole area, have exact boundary/internal multiplicities, avoid crossings/overlap and put centroids in the solid domain. Collinear bridge segments are subdivided only in the independent coverage counters, never in the retained output. Normals must retain the literal Qt value or be unit native f64 in the same exact direction. Eight corruptions and the authenticated initial runtime failure reject.

**Strict output equality remains false.** The oblique source normal is Qt float32; native retains its f64 unit normal. Proper-hole and equal-height-hole cases use different valid diagonals/order from native earcutr. Complete ordered outputs and differences remain in `invariants.json`; no sorting, tolerance widening, fallback triangulator, or pixel equivalence claim hides them. The short-only-hole fast path now follows the source fan, preserving the existing efficient native earcutr and f64 paths.

`records.zip` retains the fresh integrated native output, 27 history states, nine native saves and actual original reopens, source inputs, the initial rejected output/build record, and final independent report. `manifest.json` authenticates every byte. The [combined integration](../text-finite-integration/README.md) binds the tested binary/source hashes and build/replay commands. The source capture is reused; the strict standalone checker remains unchanged.

Replay after extracting this archive to DIR:

```sh
mesh_finite_kernel_fixture DIR
python scripts/reopen-p05-finite-kernel.py DIR
python scripts/check-p05-finite-invariants.py DIR --initial DIR/initial-native.json --initial-sha256 519bc52b68587c56ed3c4f5683d7945ad9de24bd0e862c808b716b2eb9237aa9 --report DIR/invariants.json
```

The Python command requires the pinned original's dependencies. The combined CI archive loader authenticates both the source and mandatory rejected input before replay.
