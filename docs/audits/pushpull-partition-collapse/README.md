# Subdivided opposing-cap collapse

Runtime `df6d7f4d`, based on `d32d11e9`; original pin `6be29fe437a7cd992d4e079f71720821b25216b9`.

Five valid original/native cases match exact oriented face rings, holes, attrs, interior flags, vertex survivor identities and edge flags/unique incidence. Recipes cover diagonal protected partitions, ring/island paint precedence, empty attrs fallback, annular partitions and disconnected/moving-endpoint loose wires. Requested -99 clamps to -3 across a combined coplanar opposing partition. Original tool objects are observed before serialization, with actual exact Undo/Redo replay for all six recipes. Nine payload mutations are rejected. Eleven focused PushPull unit tests pass. The twelve previous endpoint cases and their actual app consumer are replayed against the changed binary and pass unchanged.

The implementation follows source `_weld_into`, face-cycle dedupe and flat coplanar healing: more holes win a duplicate cycle, otherwise first face wins; dropped attrs are inherited only when the survivor is empty. Covered planar mothers disappear; partially covered mothers retain exact partition boundaries as holes, including the source emptied mother face. Existing edges between surviving source vertices preserve flags, while recreated edges reset them. Exact-position and original UID indexes avoid edge-by-vertex searches. Geometry outside the collapsed connected component is retained. The old protected-subdivision atomic-refusal unit test is replaced with topology and protected-payload success assertions.

The sixth annular T-junction recipe produces repeated-vertex/degenerate supporting loops in the live original. Native refuses atomically, preserving the complete input and history; this is an explicit integrity divergence, not a parity pass. `development-invalid-collapse.json` retains the earlier candidate that accepted such a primitive result while app admission refused it; the final runtime closes that primitive/app inconsistency. Source cached edge face references can contain duplicate references to the same mother; raw and unique counts are both retained. Native keeps unique cached incidence rather than copying duplicate cache references. No tolerance changes were made.

The native fixture invokes actual application pointer dispatch, VCB, preview/cancel, local Undo/Redo and nested linked-instance Finish with an integer affine parent. It verifies one folded document Undo, exact shared publication and root Undo/Redo. This is application-handler evidence, not OS-rendered gesture evidence. General lateral/nonparallel limits, arbitrary concave/nested-hole flat healing, noninteger context precision and the previously retained source-invalid Ctrl/internal-wall discrepancy remain outside this bounded closure.

## Replay inside ingetrazo-publish

```sh
cd /tmp/ingetrazo-pushpull-partition-collapse
export CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-pushpull-partition-collapse.py docs/audits/pushpull-partition-collapse/source.json
cargo test -j2 -p ingetrazo-tools --lib pushpull
cargo build -j2 -p ingetrazo-gui --example pushpull_partition_collapse --example pushpull_endpoints_app
cargo build -j2 -p ingetrazo-tools --example pushpull_gate2_fixture
$CARGO_TARGET_DIR/debug/examples/pushpull_partition_collapse docs/audits/pushpull-partition-collapse/source.json docs/audits/pushpull-partition-collapse/native.json
python3 scripts/check-pushpull-partition-collapse.py docs/audits/pushpull-partition-collapse/source.json docs/audits/pushpull-partition-collapse/native.json docs/audits/pushpull-partition-collapse/comparison.json
$CARGO_TARGET_DIR/debug/examples/pushpull_gate2_fixture docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-partition-collapse/endpoint-native.json
$CARGO_TARGET_DIR/debug/examples/pushpull_endpoints_app docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-partition-collapse/endpoint-app.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-pushpull-endpoints.py docs/audits/pushpull-endpoints/source.json docs/audits/pushpull-partition-collapse/endpoint-native.json docs/audits/pushpull-partition-collapse/endpoint-comparison.json docs/audits/pushpull-partition-collapse/endpoint-app.json
```

Source/binary/output hashes are in provenance.json. No broad suite, tracker edit or push. Final inputs/outputs and target remain for root replay until released.
