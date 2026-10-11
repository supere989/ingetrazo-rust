# Per-boundary radial multiplicity

Source correction 7036a420. Pinned original mesh.py855-859 appends each face once per boundary segment. Native VM graph had deduplicated per Edge, although prepared native FaceOwner already retained segment multiplicity. Literal old execution differs in eight edited/redone states; it is retained and rejected by the same strict comparator.

Twenty-one full actual original/native edited/Undo/Redo states now agree, covering empty/singleton/two-point/alternating/closing-repeat faces, a shared boundary hole and two equal holes. Inventories compare positions, endpoints, actual incidence, ordered radial membership, Face loop/hole refs, native core radial lists and real history counts/types. Eight negatives include literal old execution and mutated multiplicity, self endpoint, repeated loop, history and boolean-as-ID.

The correction applies ONLY to add_face. Original relink_face and add_hole deliberately deduplicate and are separate future helper obligations. No callback, membership deletion/replacement, packed field or production scripting promotion follows. The initial observer Rust type-error failure is retained, corrected before executing the literal old behavior.

Replay named vm_graph_radial with reduced-debug registry target, then:

```
python scripts/check-ai-graph-radial.py docs/audits/ai-graph-radial/original.json NATIVE docs/audits/ai-graph-radial/old-native.json
```

All fixture controllers assert logical charge and actual graph payload count zero at teardown. Complete allocator/VM memory bounds remain open.
