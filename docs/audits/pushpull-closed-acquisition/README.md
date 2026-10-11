# Closed Push/Pull acquisition and oblique sweep

Runtime d1d126f5 fixes source entry orientation before normal acquisition, outside Undo history. The source only flips the picked edge-connected component; native keeps existing vertex/edge rows and face identities, preserves two-sided paint via source turn_face_over, and invalidates model caches when acquisition actually changes winding/partition state. The narrow Tool hook is consumed at dispatch, rebasing only matching Face identities in the existing history snapshot. Cancel retains acquisition orientation; extrusion Undo returns that orientation.

Three newly captured closed source cases match exact normals and oriented face/hole/attribute/interior inventories at acquisition, Cancel, commit, Undo, Redo: reversed cube cap, reversed oblique gable roof, and independently painted front/back cap. Twelve checker corruptions reject. One new parked-component/identity/paint test and four existing affected identity tests pass. The existing four open reversed cases and thirteen inward cases were replayed because the shared acquisition/rebuild code changed; source captures were reused unchanged. Their existing explicit unrelated-sheet preservation safeguard remains.

The oblique roof exposed an initial eleven-face native result versus nine source faces: paint-compatible seam fusion kept empty old gables separate from newly painted coplanar strips. Reusing the source affected-plane material/paint rebuild closes the exact nine-face result. initial-native.json retains this failure.

The source comment's annulus-neighbour flush recipe and the retained-slab inward recipe both commit their entire request, not a smaller top-out. Their source results are six faces; current native results are nineteen and eleven. These are actionable endpoint gaps for the following delta, not passes or evidence of accepted non-Ctrl top-out. All five fresh source captures are retained. No claim covers OS gestures, rendered base suppression, the full source corpus, or arbitrary oblique/ray precision.

Replay (inside ingetrazo-publish, this checkout):

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-gui --example pushpull_closed_acquisition
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_closed_acquisition docs/audits/pushpull-closed-acquisition/source.json /tmp/closed-replay.json
python3 scripts/check-pushpull-closed-acquisition.py docs/audits/pushpull-closed-acquisition/source.json /tmp/closed-replay.json /tmp/closed-comparison.json
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -j2 -p ingetrazo-tools --test pushpull_history_identity
```

Source capture command is `QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-pushpull-closed-acquisition.py OUTPUT.json`. Source pin and executable hashes are in provenance.json. No broad suite was run.
