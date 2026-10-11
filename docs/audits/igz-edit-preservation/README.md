# Native IGZ edit/save/reopen preservation

Runtime `78946ce1`, based on `a9cd0eea`. Actual frozen Python reader/writer is used, without a Rust-derived schema oracle.

Nine cases: original arco/banca/luminaria/pileta examples; nested/reflected placements; PNG/JPEG/alpha textures; georeferenced PBR recipes; fresh document; imported numeric layers. Each native edit adds an attributed face, a layer/curve-tagged edge, front/back/base file textures, and a reference image, and renames a root group when present. Saves are reopened with actual Python and compared exactly for complete loose/group meshes, ordered group tree and shared-Mesh relationships, authored UIDs, matrices, materials, layers, image planes and georeference. Every pre-existing unedited top-level field outside those modeled lists is separately checked against the raw original. The fixture also reopens natively and asserts runtime group IDs remain unchanged.

Portable texture packing walks the serialized payload, matching the source traversal and hash-prefix filename law. Original archive bytes are borrowed; files read once per path; back paint, untinted bases, prototype/nested content and planes share the same packer. Packing avoids the former full Scene clone. Missing files retain their paths as in Python. A conflicting retained member fails before opening/replacing the destination. The three focused tests cover recursive deduplication, missing-file preservation and real conflicting-member refusal with unchanged live Scene/file.

`expected-after.json.gz` holds complete normalized post-edit Python observations (1.85 MB), rather than duplicate 160 MB pretty-printed before/after JSON. Asset paths normalize to exact SHA256 contents. Eight inventory alterations are rejected by the same validator. Removing the actual untinted-base ZIP member is additionally rejected by both the archive checker and actual Python-reopen comparison. Initial name-publication failure is retained; its source fixture initially omitted resetting the process-global auto-name counter, which was corrected to explicit fresh cold-load controls.

Original missing UIDs are independently random and are represented as `<generated>` for cross-runtime comparison only. Authored IDs are compared literally; native ID continuity is asserted separately. Names inferred from legacy unnamed inputs follow a cold document load counter; cross-document interactive global auto-name scheduling is not claimed. Legacy names/UIDs are published when the group list is regenerated; raw-preserving no-op/loose-only saves may still retain an originally absent UID. Original 16-character UID strings have deterministic native keys while their disk strings remain preserved.

Replay inside the devbox, from this worktree:

```sh
QT_QPA_PLATFORM=offscreen PYTHONPATH=/work/ingetrazo /tmp/ingetrazo-parity-python/bin/python scripts/reference-igz-edit-preservation.py prepare /tmp/igz-edit-replay
/tmp/ingetrazo-ai-registry-target/debug/examples/igz_edit_preservation /tmp/igz-edit-replay/manifest.json /tmp/igz-edit-replay/native
QT_QPA_PLATFORM=offscreen PYTHONPATH=/work/ingetrazo /tmp/ingetrazo-parity-python/bin/python scripts/reference-igz-edit-preservation.py inspect /tmp/igz-edit-replay/manifest.json /tmp/igz-edit-replay/native /tmp/igz-edit-replay/observed.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-igz-edit-preservation.py /tmp/igz-edit-replay/expected.json /tmp/igz-edit-replay/observed.json /tmp/igz-edit-replay/manifest.json /tmp/igz-edit-replay/native
```

The retained gzip can replace fresh expected.json in the checker. Inputs/hashes, source and original hashes, binary SHA and exact commands are in provenance.json. Tiny representative edited archives are retained; the large unchanged canonical inputs already exist in the pinned original and are not duplicated.

Full P01 remains open. This is format-consumer edit preservation, not an interactive Save/context/Undo workflow claim. Terrain edit schema refusal, all richer units/annotations/presentation edits, malformed/unmodeled records, saved-view reference translation for non-UUID source identities and arbitrary Unicode filename grammar still need separate operative acceptance. Existing embedded/unreferenced assets are retained. JSON/index/writer work and new-file bytes remain cold O(document/assets); this is not total-memory admission or recoverable allocator-OOM proof. Direct low-level forced format1 remains an explicit path-reference format. No broad workspace suite or rendering comparison was run.
