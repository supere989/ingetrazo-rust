# Finite valid Push/Pull closure

Source pin `6be29fe437a7cd992d4e079f71720821b25216b9`; production base `6e3b8c4a`; runtime `7dcd52a7` + corrective `5e0c62d3`. Records and binary hashes are in provenance.json. Original source was captured once; it was not changed.

The literal roster is 26 workflows / 65 operations: roof thicken→flush, all six wedge permutations, fifteen length1/2/3 side-wall permutations and four repeated-wall sequences. All original regression functions pass their closed/outward/no-coplanar-seam assertions. All65 primitive and65 persistent App results match exact coordinates, oriented cyclic rings, holes, face counts, attrs and interior. Each App operation acquires/cancels/reacquires, moves, commits positive VCB magnitude, Undo and Redo; exact vertex UIDs/edge and face runtime tokens are asserted. Models persist across operations, rather than being reset from source each step. Roof's first operation duplicates the previously retained gate2 input and is not newly missing coverage.

Initial native results admitted65 but22 oriented inventories differed. Retained native-initial.json shows the seam stations and Qt direction discrepancies. Source run_stitch removes degree-two collinear stations with identical adjacent face sets; source Face.normal normalizes its Qt-stored Newell vector in float32. The correction uses that authored direction while preserving f64 document positions and geometric admission. Cold cleanup discovers connected sweep entities once, reevaluates neighbours in original vertex order, and publishes one batched remap. A focused test preserves loose-wire junctions, opaque metadata, face identity and a disconnected subdivided face inside sweep bounds. Existing nine solid outputs remain byte-semantic equal to prior canonical evidence.

The checker rejects12 corruptions, including a2^-22 coordinate alteration. No tolerance, renderer, source normalization or invalid topology guard changed. Source-invalid Ctrl cases, original unrelated-sheet deletion safeguard, and the unproved accepted non-Ctrl top-out witness remain explicitly outside this finite valid roster. This is not a full P03 row promotion.

Replay from this runtime:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-gui -p ingetrazo-tools --example pushpull_finite_workflows --example pushpull_gate2_fixture
# Extract source.json from records.zip into a private directory.
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_finite_workflows source.json app-fresh.json
python3 scripts/check-pushpull-finite-corpus.py docs/audits/pushpull-finite-corpus report.json app-fresh.json
```

All handles terminal. Temporary final inputs/outputs remain available for root independent replay; reusable target and source are retained.
