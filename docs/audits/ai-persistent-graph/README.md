# Persistent VM Edge graph prerequisite

Source `c74bf789`; pinned original `6be29fe437a7cd992d4e079f71720821b25216b9`.
Standalone RustPython proof only; production execution stays disabled.

## Accepted focused observations

- Thirteen complete actual original/native states match recursively: shared generated edges, incidence and ordered radial membership, shallow field alias identity, repeated Undo/Redo, exception restoration, detached held references and the original omitted endpoint restoration policy.
- Field comparisons use source snapshot dictionary order: all soft fields, all hidden fields, all layer fields, all curve fields. Eight actual callback events match. Geometry comparison short circuits later opaque equality exactly in the selected control.
- False/true and same-membership-mutating equality controls match with one callback. Initial native three-callback failure is retained and rejected by the same validator.
- Real logical-budget denial after working graph edits preserves graph state, nonempty Redo and previous owner charges.
- Created-edge order/replacement/endpoint edits are defensively refused and restored pending packed-identity projection. Two cold engineering callbacks actually call Mesh mutation/construction under held transaction ownership; caught refusal poisons the command, preserves history/charges and recovers only after stage discard. They do not claim arbitrary Python callback equivalence.
- Generated triangle plus manually held edge now produces source's three Vertex/four Edge objects. Actual cycle collection leaves no payloads. The prior three/one boundary remains in earlier evidence.

## Required open defects

Original `core/ai.py` invokes snapshot equality AFTER `history.execute`. A raising `__eq__` leaves the command committed/Redo cleared in source; native currently restores and refuses. A mutating equality can change a live field after captured history in source; native detects and restores. Both exact mismatches are retained in checker.json, and no full callback-parity claim is made.

Packed native projection of public Edge fields/endpoints/incidence is incomplete. Existing Edge deletion/replacement/reorder and arbitrary iterable incidence/radial slots remain required. New-edge endpoint refusal is a temporary prerequisite, not original API parity. Full helper APIs, source aliases, object mutations and callback history must still be implemented.

Graph snapshots are O(root × history), not the final optimized sparse journal. Charges cover logical owned references and selected temporary native capacities, not RustPython's complete set table/heap, allocator peak, or arbitrary builtin cost. Identity comparisons sort O(n log n), not a claimed O(n) source set lookup. No rendering, production enablement, total-memory or deadline promotion.

## Replay

Inside ingetrazo-publish, in the isolated worktree:

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build --manifest-path tools/ai-interpreter-proof/Cargo.toml --bin vm_persistent_graph --bin vm_graph_eq_outcomes --bin vm_graph_callbacks --bin vm_graph_field_order --bin vm_graph_safety -j2
```

Run each named binary and pass outputs to:

```
python scripts/check-ai-persistent-graph.py docs/audits/ai-persistent-graph/original-13.json NATIVE13 docs/audits/ai-persistent-graph/original-equality-6.json NATIVEEQ docs/audits/ai-persistent-graph/original-callbacks.log NATIVECB docs/audits/ai-persistent-graph/old-callbacks-3.log
python scripts/check-ai-graph-safety.py docs/audits/ai-persistent-graph/original-field-order.json NATIVEORDER NATIVESAFETY
```

Checkers enforce inventories/types, run nine recursive graph mutations plus five admission/order mutations through the same validators, and reject literal old callback counts. They retain the two original/native semantic boundaries rather than widening criteria.

The initial compile API failure is retained. The initial field-output checker failed because the fixture printed only an assertion message; it was corrected to export actual callback events and real stack counts, then rerun. No source tolerance was changed.
