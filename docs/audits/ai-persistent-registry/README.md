# Persistent private source registry — bounded standalone proof

Source `a1dad5aa`, parent `851070f8`. Frozen Qt source remains `6be29fe437a7cd992d4e079f71720821b25216b9`; unchanged original seven weld outputs reused from ai-weld-boundary. New owner_weld executes a separate private-owner transaction per point, so later scripts read the persistent base registry rather than only transaction overlays.

Four focused controls cover stale views before lookup after append/Undo/Redo/replacement, failed index preparation preserving an earlier transaction, duplicate vertex-key last-wins and duplicate edge IDs, replacement retaining stale-view charges until final drop. Three shared-budget regressions pass. The actual Rust allocator records zero allocations for 1,000 begin/drop operations at each base size 1/1,000/50,000. Varying endpoint operations report three lookups per operation and no additional cold rows; these operations do allocate keys/staging and do not claim zero allocations.

Registry preparation reserves keys/container capacity before semantic writes, then moves prepared entries into the private shared state. Every view carries document/state/version tokens. Registry leases account conservative logical typed slots, retain high-water capacity through Undo, and survive stale held views. They are not physical allocator quotas: allocator overhead, capacity slack, BigInt allocation failures, VM/compiler/native allocations remain excluded. Failed capacity preparation may grow an empty container capacity without changing published lookup contents. Publication still prepares the full core spatial index O(base); constructor/replacement cold scans remain. Other application writers and full Python adapters are not routed through this owner. Production scripting stays disabled.

Focused replay inside ingetrazo-publish, cwd tools/ai-interpreter-proof:

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo test -j2 --no-default-features --bin owner persistent_registry
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo test -j2 --no-default-features --bin owner shared_budget
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin owner_weld
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target cargo run -j2 --no-default-features --bin registry_begin
python3 scripts/check-ai-persistent-registry.py
```

No VM rebuild or broad suite. Retained checker uses the same validation function for all six malformed/mutated negatives.
