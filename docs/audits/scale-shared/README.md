# Plain shared Scale targets

Pinned source ScaleGroupCommand mutates a classic xform=None group's shared mesh directly. Geometry/UVW reach all sibling placements; only selected group axes carry the scale. Selecting two groups sharing the same mesh previews once from seeds, then executes two Scale commands at commit.

Native Targets captures all matching scene/definition/registry owners once and updates their frozen vertex rows in place. No whole Scene clone, mesh clone or model traversal on pointer motion. Owner paths guard group runtime identities; selected axes remain per-placement. Cancel/restore restores every owner; commit/retype applies source command multiplicity to geometry/UVW. Equal independent mesh does not acquire sharing.

Thirty exact source phases across baked, shared-one and shared-two: before, grab, preview, center, commit, retype, Undo, Redo, cancel-preview, cancel. Native uses actual Targets helper and snapshot replay; root's Scale controller/application acceptance is separate. Nine corruptions reject. Focused nested/definition registry ownership and axes/token preservation test passes. No broad suite or OS gesture claim.

Runtime targets.rs includes root's uncommitted base target engine as prerequisite; root should merge share-specific additions by symbols and retain newer only_placements/apply_cold_mesh/has_loose_vertices helpers. Locally copied root ScaleMatrix and Mesh::place_vertices are uncommitted build prerequisites, not duplicated intake changes.

Replay: cargo build -j2 -p ingetrazo-tools --example scale_shared; example SOURCE OUT; python3 scripts/check-scale-shared.py SOURCE OUT COMPARISON. Source PYTHONPATH=/work/ingetrazo parity Python scripts/reference-scale-shared.py OUT.
