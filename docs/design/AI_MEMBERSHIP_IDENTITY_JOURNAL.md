# Persistent Mesh membership identity journal

Status: pure-permutation prerequisite implemented and focused evidence retained; deletion/replacement graph ownership remains design. Production is disabled.

Frozen source `core/mesh.py` keeps `vertices`, `edges`, `faces` as persistent lists, while `_registry` maps coordinate keys to actual Vertex objects and Edge/Face topology references those objects directly. `capture_state` records list membership separately from referenced object fields; `restore_state` reuses the objects and replaces their field wrappers. A list reorder therefore does not rekey the registry or alter edge endpoints. A deletion or replacement may leave an object reachable through the registry, edges, face loops or script globals even though it is absent from a membership list. These states cannot be modeled by silently treating an array index as permanent object identity.

## Dependency-ordered publication

A pure permutation prerequisite can map prior genuine wrapper identities to new membership indices. It must prepare checked bijections, remap packed native edge endpoints and face/edge incidences, preserve native UID payloads, and prepare spatial/registry publication before moving records. No coordinate matching. Original source weld keys must map to the same objects' new packed indices. Undo restores the inverse layout before prior append history can run. Mixed append plus permutation requires a unified ticket, not successive mutating operations.

Full deletion/replacement requires identity-owned object storage separate from public list membership. Reachable detached objects remain alive and charged until all actual registry/topology/history/scope references release them. The native packed render/document adapter is a projection of that graph, with an explicit per-revision identity-to-packed-index map. Script-facing persistent lists must never be replaced with reconstructed value arrays or JSON.

## Required controls

Actual pinned source reorder/reverse/slice replacement controls with held Vertex/Edge/Face refs; registry lookup after reorder; exact Undo/Redo wrapper identities; duplicate-key ownership; stale transaction refusal; preparation refusal preserving graph/history/Redo; detached objects reachable from face loops/registry/scope; reinsert identity; branch truncation and last-handle release accounting. Pure permutations alone do not establish deletion/replacement or complete Edge/Vertex-set API parity.

Controller snapshots remain a temporary before/after reference template. Production architecture must use command-owned object deltas/shared immutable payloads, preserving complete source API and callback rollback semantics without O(root references × history depth) duplication.


## Real Edge prerequisite and next publication boundary

The standalone constructor/incidence proof in `docs/audits/ai-edge-objects` provides actual identity-bearing Edge objects, not JSON endpoint/radial surrogates. It does not yet fabricate a Mesh.edges list or claim populated generated-Vertex incidence. Next publication must prepare all Edge objects and incidence member/radial references BEFORE applying the packed native ticket, reserve member capacities with command-owned leases, then move admitted state into persistent memberships. Undo/Redo must retain the same Edge/Vertex/Face objects while source restoration replaces the incidence/radial wrappers.

Actual Scene.bounds has TWO different authorities (source scene.py649–704): loose bounds traverse visible Edge endpoints/Face positions; Group bounds use current public mesh.vertices membership. A registry-only detached Vertex can be reached for loose drawing yet must not pad Group membership bounds. Packed topology indices must therefore be separate from public memberships and must not implicitly determine every bounds query.

Deletion/replacement requires a genuine identity graph with persistent public Vertex/Edge/Face lists, registry-held actual Vertex refs, explicit edge endpoints and Face loops, and an identity-to-packed projection. Script external refs and history own detached nodes until released. Numeric registry IDs cannot be interpreted as public-list offsets after membership edits. Shared native index allocation and reversible ownership must remain staged/fallible; broad root snapshots remain temporary O(root×history), not optimized production architecture.


### Publication guard before the full incidence graph

`vm_incidence_refusal` is an explicit safety prerequisite, not Python behavior equivalence. Managed root vertices currently have no populated native Edge wrapper incidence, so admission refuses a nonempty/ill-typed baseline and publication refuses any inserted incidence. Root restoration and field history replace the set wrapper with a newly prepared empty set; external aliases to the discarded set retain their actual contents. Newly staged vertex incidence is validated before native publication too. This prevents silently committing fields that have no native/history counterpart while the true graph is being wired. Full source accepts these edits and remains required.

Next integration sequence:

1. Prepare actual Edge objects from FaceTransaction endpoint/radial identities, including reused base-edge wrappers; reserve persistent Mesh.edges member capacity and per-endpoint incidence/radial changes before any native apply.
2. Extend root history templates with real Edge refs and source member-only snapshots of Vertex.edges and Edge fields, with poison/unlock-before-drop. Replace the temporary empty-incidence refusal only when this command ownership is complete.
3. Decouple source registry vertex identities from public Vertex-list ordinals, using stable identity-owned graph nodes and separate packed projection. Public membership removal/replacement must preserve nodes reachable from endpoints, face loops, registry, script aliases and parked command deltas.
4. Prepare packed changed geometry/index and reversible identity mappings before publication; no coordinate reattachment or full Scene clone. Unknown native completion and VM/native callback boundaries remain separate required gates.

Constructor/incidence objects are VM heap allocations outside OwnerBudget's logical native accounting. Snapshot preparation is still O(root×history); persistent mutation tracking and delta history remain required before production.
