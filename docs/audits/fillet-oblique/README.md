# Fillet oblique authoring correction

The retained 27/38/14 versus 26/36/14 discrepancy was an invalid source fixture, not a demonstrated native kernel defect. `reference-fillet-scoped.py` assigned positions then wrote `_verts`; pinned `Mesh.vertex()` uses `_registry`. The oblique source output had duplicate objects at [2.5,0,0], with four one-face boundary edges. The original regression constructs the oblique face loops directly and requires `core.orient.is_closed`. We retain the old output and reject it as a negative instead of reproducing broken identities in Rust.

The corrected source authoring uses actual `Mesh.place_vertex`, which rekeys `_registry`. The before state remains exactly equal to the archived input: positions, edge/face records, attributes and recorded old-object origins. Only hidden runtime registry state changes. Correct source and unchanged native kernel both yield 26 vertices, 36 edges and 14 faces. Independent directed boundary incidence is two/opposite on every edge. The original cap plane invariant also passes.

Audited all six earlier fixture constructors. Only oblique-cap and turned-squashed used the inert `_verts` assignment. Those two were recaptured once with `place_vertex`; the other four were not recaptured. Corrected turned-squashed remains 128/300/182 and matches its previous oriented geometric/metadata inventory. Corrected source/native model outputs match face loops/holes, flags, paint/interior, old Face/Edge identity and old vertex ownership. New source-global curve IDs compare equivalence partitions; existing Qt-f32/native-f64 coordinate boundary remains 1e-6.

Both actual application consumers pass activation, cursor sizing, typed commit, Undo and Redo (ten phases), including history/selection counts and preview faces/lines. This is direct original Viewport/History and native App dispatch, not XTEST or rendered pixel proof. Eight actual validation mutations reject old invalid-registry output, removed edge, reversed face, wrong paint/interior, lost old edge identity, changed hidden flag and lost old vertex identity. No kernel/app/base change, broad suite, GPU run or push.

`15a6f6a6` is the stable corrected source/consumer commit. `records.zip` is byte-verified and retains the previous invalid source output, corrected output and build/runtime logs. The previous historical `fillet-scoped` acceptance and failures remain immutable; this proof supersedes its oblique topology diagnosis. Nested Finish unused-vertex/curve differences and protected-context metadata loss remain separate open work, as do broad P09 rendered workflows.

Replay:
```
python3 scripts/check-fillet-oblique.py docs/audits/fillet-oblique
```
Fresh native consumer (reusing the owned target):
```
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p ingetrazo-gui --example fillet_oblique -j2
python3 -c "import zipfile;z=zipfile.ZipFile('docs/audits/fillet-oblique/records.zip');open('/tmp/fillet-oblique-input.json','wb').write(z.read('source-fixed.json'))"
/tmp/ingetrazo-metal-world-target/debug/examples/fillet_oblique /tmp/fillet-oblique-input.json /tmp/fillet-oblique-output.json
python3 scripts/check-fillet-oblique.py docs/audits/fillet-oblique /tmp/fillet-oblique-comparison.json /tmp/fillet-oblique-output.json
```
Original replay requires pinned source PYTHONPATH and Qt offscreen. No runtime repair commit exists because actual valid source behavior already matches native.
