# Nested Scene console consumer

Base: published `352805d2`. Pinned original: `6be29fe437a7cd992d4e079f71720821b25216b9`.

The opt-in actual application console now hydrates recursive Group children with genuine persistent Group/Mesh/list objects, sharing one VM Mesh per native prototype token across the entire tree. Publication recursively prepares placements, shared Mesh receipts and component definitions before application writes. `Group.adopt`, replaceable children lists and component flags use the same graph. Each placement keeps its own children and local float matrix; a shared Mesh definition does not require identical private child placements.

Original `core/history.py:SnapshotImport` owns loose mesh state and added TOP-LEVEL memberships. Existing child membership, placement and shared-Mesh edits survive that command's Undo. The application fixture observes each state immediately after the operative command/history action, without a later inspection command publishing delayed changes.

## Focused acceptance

- 12 exact typed original/current app states: recursive imported shared components; private child transforms; nested detach/reinsert; held Mesh aliases and shared vertex edits; top-level append/Undo/Redo/remove/reinsert; genuine Group.adopt and child-list replacement.
- Three actual publication refusals (ancestry cycle, nonfinite nested transform, foreign child) preserve complete visible Scene and both application history stacks.
- One focused application test forces logical publication admission pressure with existing Redo. Nested candidate changes are refused, scope retires, the retained Redo remains usable, and a fresh nested edit succeeds.
- Both real final IGZ archives are reopened in pinned original. Recursive local/effective Qt placements, instance/component flags, geometry and true Mesh aliases compare exactly; three altered reopen observations reject.
- Eight altered state/refusal observations reject through the same validator.

Build/run (inside ingetrazo-publish):

```
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -p ingetrazo-gui --features engineering-python --lib nested_group_publication_admission_preserves_redo_and_retries -j2
CARGO_TARGET_DIR=/tmp/ingetrazo-ai-registry-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p ingetrazo-gui --features engineering-python --example engineering_console_nested_fixture -j2
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-ai-console-nested-scene.py INPUT.igz > original.json
/tmp/ingetrazo-ai-registry-target/debug/examples/engineering_console_nested_fixture INPUT.igz native.igz > native.json
/tmp/ingetrazo-parity-python/bin/python scripts/check-ai-console-nested-scene.py original.json native.json
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/check-ai-console-nested-reopen.py INPUT.igz.source-final native.igz
```

## Boundaries and remaining work

Default production bridge admission is unchanged. Genuine nested ownership is now operative, but arbitrary custom child sequences, cyclic placement graphs, nonaffine/nonfinite/lossy native-double matrices remain explicit admission boundaries. Native refusal is safe application behavior, not a claim that original arbitrary scripting refuses the same inputs. Full stdlib/resources, native callback semantics and total VM/allocator memory admission remain OPEN.

Hydration is cold; it no longer clones full native Mesh/child trees into every ancestor baseline. Opaque native JSON metadata can still contain repeated tree payloads. Every publication still walks current placements, packs each distinct live Mesh once and clones receipts/definition snapshots for the existing core schema; this is not an O(changed) command or total-heap bound. App history still retains full Scene snapshots. Nested component-definition child snapshots can repeat subtrees and require further immutable ownership optimization. Effective matrices in the state fixture use float Qt-equivalent composition and the real reopen uses Qt itself; this is not GPU rendering or a claim about double-precision World traversal. Nesting admission is 128 levels.

Predecessor refused nonempty children at both hydration and publication. This batch removes that real console consumer refusal; broader Scene/global APIs remain required.
