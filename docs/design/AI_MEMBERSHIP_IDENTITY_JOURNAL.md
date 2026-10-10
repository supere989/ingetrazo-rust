# Persistent Mesh membership identity journal

Status: pure-permutation prerequisite implemented and focused evidence retained; deletion/replacement graph ownership remains design. Production is disabled.

Frozen source `core/mesh.py` keeps `vertices`, `edges`, `faces` as persistent lists, while `_registry` maps coordinate keys to actual Vertex objects and Edge/Face topology references those objects directly. `capture_state` records list membership separately from referenced object fields; `restore_state` reuses the objects and replaces their field wrappers. A list reorder therefore does not rekey the registry or alter edge endpoints. A deletion or replacement may leave an object reachable through the registry, edges, face loops or script globals even though it is absent from a membership list. These states cannot be modeled by silently treating an array index as permanent object identity.

## Dependency-ordered publication

A pure permutation prerequisite can map prior genuine wrapper identities to new membership indices. It must prepare checked bijections, remap packed native edge endpoints and face/edge incidences, preserve native UID payloads, and prepare spatial/registry publication before moving records. No coordinate matching. Original source weld keys must map to the same objects' new packed indices. Undo restores the inverse layout before prior append history can run. Mixed append plus permutation requires a unified ticket, not successive mutating operations.

Full deletion/replacement requires identity-owned object storage separate from public list membership. Reachable detached objects remain alive and charged until all actual registry/topology/history/scope references release them. The native packed render/document adapter is a projection of that graph, with an explicit per-revision identity-to-packed-index map. Script-facing persistent lists must never be replaced with reconstructed value arrays or JSON.

## Required controls

Actual pinned source reorder/reverse/slice replacement controls with held Vertex/Edge/Face refs; registry lookup after reorder; exact Undo/Redo wrapper identities; duplicate-key ownership; stale transaction refusal; preparation refusal preserving graph/history/Redo; detached objects reachable from face loops/registry/scope; reinsert identity; branch truncation and last-handle release accounting. Pure permutations alone do not establish deletion/replacement or complete Edge/Vertex-set API parity.

Controller snapshots remain a temporary before/after reference template. Production architecture must use command-owned object deltas/shared immutable payloads, preserving complete source API and callback rollback semantics without O(root references × history depth) duplication.
