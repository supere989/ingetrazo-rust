# P05 direct Mesh weld/bulk/non-manifold delta

Runtime `1353a564`, base `67546fe7`, pinned original `6be29fe437a7cd992d4e079f71720821b25216b9`. This is a bounded kernel behavior batch, **not complete P05**.

Twenty original/native cases compare full ordered vertex payloads, edge endpoints/flags/radial incidence and face loops/holes/attributes. Eleven cases are exactly equal: positive/negative cell crossings, strict distance rejection, ties and ordered neighboring acquisition, two/three-face splits with endpoint no-op, a hole split onto a preexisting painted subedge, same-cell diagonal acquisition, placement collision ownership and an exact-half-key diagonal. Eight original bulk recipes independently verify the original bulk builder against original sequential output. A 1,200-quad import-like grid retains exact native vertices/faces and every proper edge; its first source recipe also includes a holed face and collapsed sliver. This demonstrates a specific larger case, not general throughput or a scene-scale performance claim.

All nine bulk/grid full states remain **unequal**: the source sliver creates one self-edge with triple radial membership; ordinary native edge admission omits it. The checker reports this raw mismatch and requires all other payloads/order to remain exact; it does not rewrite the full source output or claim parity. Source sliver behavior and pathological triangulation/other kernel lifecycle remain open.

The native binary additionally asserts unchanged existing Face/Vertex identities across splits, reused subedge opaque metadata, fresh subedge constructor attributes, endpoint no-op, malformed/missing/nonfinite split atomicity, invalid edge endpoint refusal and genuine large-coordinate f64 preservation. The source splitter uses constructors for new edges rather than inheriting removed-edge paint/flags; welded existing subedges keep their metadata. Dense edge-row removal remaps incidence once, with no complete Mesh clone. Incident ring insertion is admitted before semantic changes.

Neighbor lookup preserves source unconditional same-cell acquisition (including diagonals farther than the radius), then ordered 26-cell probes with strict distance. Exactly Qt-f32-authorable positions use Python ties-even keys and Qt-f32 distance arithmetic without altering coordinates. Genuine native doubles retain the existing native key/distance extension. This is a stated native-double compatibility policy, not provenance inferred from arbitrary rounded payloads. Global PointKey/math APIs are unchanged. All Mesh spatial preparation/reindex/place/acquisition/split key construction uses the same local policy. find_edge scans the smaller endpoint incidence and validates both IDs.

Named builds used the same target/dependencies: initial 2.61s, supplemental consumer build, final reviewed key-policy build. No broad suite, GPU capture, source modification, CI or push. Only missing same-cell/collision and exact-half controls were captured after the initial 17; initial source inputs remain archived. Ten strict corruptions reject (empty/duplicate/truncated, weld, radial order, reused metadata, lost large edge, paint, boolean ID and half-tie).

Replay existing binary without compilation:

```sh
unzip docs/audits/p05-mesh-kernel/records.zip -d /tmp/p05-review
/tmp/ingetrazo-metal-world-target/debug/examples/mesh_kernel_acceptance /tmp/p05-review/input.json > /tmp/p05-review/fresh-native.json
python3 scripts/check-mesh-kernel.py /tmp/p05-review/source.json /tmp/p05-review/fresh-native.json
```

If a focused rebuild is needed:

```sh
CARGO_TARGET_DIR=/tmp/ingetrazo-metal-world-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -j2 -p ingetrazo-core --example mesh_kernel_acceptance
```

Raw inputs/outputs remain `/tmp/p05-kernel-proof` until parent release. Binary SHA and source/runtime hashes are in archived provenance; ZIP member hashes are in manifest.json. No target cleanup while reserved.
