# Unified existing-field and append command ticket

Status: source-only prerequisite, not enabled. Production scripting remains disabled.

Frozen source raw QVector setters retain original registry weld keys. A transaction can therefore edit an existing position while adding a face that resolves the original coordinate to that same moved object. The resulting native graph must publish the new position, old loop edits, new vertices/edges/faces and radial append incidence together, preserving object identities.

## Preparation and publication

1. Validate exact persistent VM Vertex/Face membership prefix and staged objects. General membership edits require a separate identity journal.
2. Prepare raw existing positions/loops and appended topology under one exclusive owner ticket. Source-key registry preparation changes observed positions without rekeying old objects; appended keys retain their source lookup identity.
3. Reserve final spatial-index preparation, native history fields and face/radial storage before semantic writes. An append spatial delta prepared against the old positions cannot be installed after moving its base prefix; the mixed path must use a final prepared index.
4. Publish all components allocation-free, advancing one owner state token and monotonic revision. Candidate discard never changes positions, registry lookup, stack membership or history leases.
5. Undo/Redo must preflight the complete target before moving any field/topology payload. Apply old/new existing fields together with append truncation/restoration. Append and field-only commands must remain chronological under the same state-token contract.

## Current source-only increment

`Registry::prepare_append_positions` prepares prefix observation changes and new source-key entries as one `Change`, retaining the existing key map. Its unpublished position vector has an explicit preparation lease. Focused controls cover discard preserving the old View, successful publication making it stale, original-key ownership after movement, and quota refusal preserving state/charges. Both focused controls compiled and passed after the target loan returned; retained output is in `docs/audits/ai-mixed-registry/`. This validates registry preparation only, not unified topology publication.

## Required following work

The native append ticket still needs final-index and reversible existing-field payload integration; VM mixed refusal remains intact. Root-only snapshot history remains temporary O(root references × depth). Shared immutable/delta history, full membership edits, arbitrary attribute callback atomicity, native reentry and physical VM/compiler bounds remain required. No mixed parity or performance acceptance is claimed here.
