# Identity-owned graph projection prerequisite

Pinned authority: core/mesh.py at 6be29fe437a7cd992d4e079f71720821b25216b9.
Production scripting remains disabled. This design does not approve API exclusions.

## Source ownership that numeric packing cannot erase

`Mesh._registry` owns actual Vertex references, independently of `Mesh.vertices` order. Raw deletion from the public list does not delete registry entries; `vertex()` can return a detached object without appending it. `find_edge()` searches the smaller endpoint incidence and calls each member's `other()` dynamically. It does not require membership in Mesh.edges. `remove_edge()` instead detaches both endpoint incidence sets and removes public membership without cascading faces. `prune_orphan_vertices()` removes registry entries by object identity. Native source registry currently maps keys to packed numeric IDs, so arbitrary deletions cannot be implemented by renumbering those IDs alone.

The source snapshots capture registry, public lists, positions, incidence, radial order and shallow attrs. Edge endpoint fields are deliberately omitted. Source Undo restores captured lists/fields while endpoint changes survive. Packed geometry must be rebuilt from currently live endpoints even when restoring a snapshot; copying an old packed native buffer would violate this policy.

## Proposed owner split

- Stable Vertex/Edge/Face records keyed by genuine VM object identity. Persistent record storage holds leased object references, original weld-key ownership and native stable UIDs. Detached references remain valid; records release only after registry/history/VM ownership releases.
- Public ordered membership vectors are independent views over those records. A projection ticket maps current public membership plus referenced face loops/edge endpoints into packed core vertices/edges/faces. Reachable detached vertices require explicit record-to-packed mapping; public membership cannot be inferred from packed rows.
- Source lookup registry maps SourceKey to identity records and preserves raw-move stale keys. Its changes are command deltas, not rebuilt from current public order. Source `find_edge` consumes actual incidence and dynamic `other`, not a numeric edge-key shortcut.
- Command journal owns before/after membership deltas, record fields and original source registry changes. History snapshots are a temporary prerequisite; sparse reverse operations must replace O(root × history) storage before production.

## Prepared publication and failure boundary

Before touching native state: validate reachable identities/types, reserve all checked member/record/index/loop/extents, prepare spatial index and source-registry delta, allocate capacity-aware leases, and finish all Python callbacks with no native/public-list locks held. Ticket holds those charges until apply/discard. Native writes are allocation-free swaps/updates guarded by owner revision. Existing stale transactions refuse before lookup or mutation. No unjournaled wrapper changes can be ignored when publishing native arrays.

History restoration prepares projection using restored public membership and CURRENT unsnapshotted endpoint fields. Refusal leaves wrappers, packed state, registry, history and Redo unchanged. Arbitrary attrs equality happens after source command execution; its side effects/raising behavior remain an unresolved parity boundary that must be reconciled with atomic host safety, not hidden behind safe refusal.

## Required focused controls

1. Public-only edge remove/reorder/replace vs remove_edge incidence detach; find_edge identity results.
2. Detached Vertex in registry reused by add_edge/add_face; prune removes by identity even after move.
3. Held old Edge/Face/Vertex remains valid across deletion, replacement, Undo/Redo and chronological branch truncation.
4. Current endpoint mutation survives failure/Undo while native packed endpoints update correctly.
5. Combined deletion/replacement, loop refs, append and attrs in one command; no silent partial projection.
6. Actual allocation denial in prepared identity maps/native topology/spatial index leaves exact graph/history/ledger unchanged; discard releases lease.
7. Many small commands use persistent lookups and delta residency, not per-command root cloning. Report visited rows and real allocator growth independently of logical inventory.
8. GC/finalization and native callback reentry remain subject to poison/lock-release rules, with genuine reference lifetime controls.

Next implemented dependency is genuine Mesh.find_edge. Its source fixture exercises dynamic incidence selection and callback ordering. Public arbitrary membership publication is still required and currently refused; no production enablement follows from the lookup slice.
