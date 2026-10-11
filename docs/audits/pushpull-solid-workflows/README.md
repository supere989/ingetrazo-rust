# Push/Pull host-hole solid workflow

Runtime ab271ec2 closes both actual original stacked-block side pushes (test_pushpull_subtractive.py:421), including the second adjacent wall and its host-hole boundary contact. The source's triangular boundary is a valid manifold even when polygon rims share redundant incidences. Native admission checks the complete oriented triangle manifold rather than weakening raw closure; removed, reversed, duplicated and nonfinite corruptions all reject. Translating an existing split host edge must weld then stitch T-junctions before endpoint classification. Existing metadata and snapshot ownership remain intact.

The retained valid Ctrl step +0.5 now matches every oriented face/hole/attribute/interior field: original cap_rebuild plane_key canonicalizes the rebuilt plane, and interior partitions retain its arrangement winding. Untouched partitions are outside this policy.

Nine newly captured pinned regression operations produce identical physical solid shapes under the independent original manifold reader, with exact volumes and zero symmetric difference. Eight also match literal oriented fields. The ninth is a real direct-private-helper discrepancy: test_repush_top_extends_holed_wall_keeping_opening calls _push without operative prism classification, causing the source to reverse a host-hole ring. A separately captured _push_real witness uses actual classification and matches native exactly. Both observations remain; no ring orientation is normalized away in the exact checker.

Six actual IngeTrazoApp controls match exact source fields at commit and Redo, with acquired-state Cancel/Undo and vertex/edge/face runtime identity restoration. These include both stacked pushes, through-wall, operative holed repush, Ctrl step, and a nested translated shared component with local history, Finish, linked publication and document Undo/Redo. There is one command per commit and none for acquisition/Cancel. This is real App handler/VCB evidence, not OS gesture or rendered evidence. Thirteen checker corruptions reject, including a 2^-22 coordinate change. Twelve focused PushPull tests pass.

The two invalid original Ctrl inward cases remain atomic errors. On the current base, wedge0--0.4 returns an unchanged zero-limit no-op, not the older documented error; its invalid original raw output is not copied. The retained source global deletion safeguard, non-Ctrl top-out witness, source private-helper hole winding, broader slanted/permutation corpus and platform gestures remain OPEN. Full P03 is not claimed. initial-native.json and repaired-failed-candidate.json retain causal failures.

Replay inside ingetrazo-publish, from this checkout:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-gui -p ingetrazo-tools --example pushpull_solid_workflows --example pushpull_gate2_fixture
/tmp/ingetrazo-metal-world-target/debug/examples/pushpull_solid_workflows docs/audits/pushpull-solid-workflows/app-input.json /tmp/pushpull-solid-fresh-app.json
python3 scripts/check-pushpull-solid-workflows.py docs/audits/pushpull-solid-workflows /tmp/pushpull-solid-fresh-comparison.json /tmp/pushpull-solid-fresh-app.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-pushpull-solid-shape.py docs/audits/pushpull-solid-workflows /tmp/pushpull-solid-shape.json
```

The original scripts capture only the missing sequential and operative-classification witnesses. Original pin, source/test/binary/record hashes are in provenance.json. No broad suite, original recapture of existing corpora, GPU run, tracker edit or push occurred. Final temporary inputs/outputs remain for root replay until release.
