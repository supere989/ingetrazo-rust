# Scoped Fillet preservation and history

Runtime `0d84512b`; actual consumer `70ccb628`. This closes scoped publication preserving all old vertices, untouched Face/Edge identities/flags, free wires at selected endpoints, unrelated open faces, authored interior and first-round paint precedence. Only selected connected shells undergo cold solid admission. Refused radius clears preview without model/history mutation. Successful commit prepares once. Per-app radius/segment settings survive tool switches; multi-window class-global parity remains unproved.

Five required model recipes pass oriented boundary, hole, metadata, identity and raw count comparison: endpoint-free-wire, open-neighbor, paint-first-rim, concave, turned-squashed. The oblique-cap literal failure remains: source27 vertices/38 edges/14 faces, native26/36/14. The existing source Qt-f32/native f64 coordinate boundary is1e-6; no tolerance increase. New module-global source curve numbers are compared by equivalence partitions; old authored IDs remain literal.

Two local application recipes have16 exact handler phases: activation,6s, click/move sizing, invalid5m refusal, Cancel,0.2m commit, Undo, Redo. Nested translated plain component has8 matching active phases plus Finish-Undo. Three nested Finish/Redo states preserve geometry, attributes and history but differ literally: source drops two unused original rounded endpoints (20 vs22 vertices) and re-splits curve partitions. Checker allows only those two exact unused endpoint positions and explicitly records curve failure. This is not full nested parity.

The initially protected nested recipe is retained in records.zip: original transformed_mesh walking discards orphan vertices, authored interior and hidden-edge flags, whereas native preserves them. It is excluded from passing claims, not silently normalized. No context runtime was changed in this Fillet batch.

Seven checker mutations reject lost wire, wrong interior, wrong paint, lost identity, reversed face, missing face, merged curve ownership. Recipe byte hashes and literal case/phase inventory reject replacement/empty corpora. Eight focused Fillet tests pass. Initial source loader typo and native interior-validator failure logs remain in the archive. No broad suite or GPU replay.

Replay retained source proof:
```
python3 scripts/check-fillet-scoped.py docs/audits/fillet-scoped
```
Replay a freshly compiled consumer:
```
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p ingetrazo-gui --example fillet_scoped -j2
python3 -c "import zipfile; z=zipfile.ZipFile('docs/audits/fillet-scoped/records.zip'); open('/tmp/fillet-source.json','wb').write(z.read('source.json'))"
/tmp/ingetrazo-metal-world-target/debug/examples/fillet_scoped /tmp/fillet-source.json /tmp/fillet-native.json
python3 scripts/check-fillet-scoped.py docs/audits/fillet-scoped /tmp/fillet-comparison.json /tmp/fillet-native.json
```

Original scripts require pinned submodule PYTHONPATH and Qt offscreen; records are live-object output, not IGZ serialization. The named native example exercises real dispatch/history/context methods, not rendered pointer events. Source accepts segment counts above256; native bounded refusal remains outside this acceptance. Broad P09/rendered gestures/other invalid-topology behavior remain open.
