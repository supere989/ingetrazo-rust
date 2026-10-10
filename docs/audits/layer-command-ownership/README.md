# Layer command ownership

Source baseline `cc170ea7` executes actual pinned Python LayersPanel callbacks
and AssignLayer/PurgeUnused/SnapshotImport commands under Qt offscreen.
Native source `891ac5ce` uses production app methods and model-history hooks.
Four cases/13 states compare exactly after explicitly translating the legacy
native default name `Layer0` into source `Layer 0`. All other names, ordering,
flags, entity labels and history depths are exact. Four corruptions are rejected.

Ownership is distinct:
- Panel add/visibility/lock remain outside History and retain Redo.
- Assign-created layers are removed by the original target **name** on Undo.
  Renaming leaves the renamed layer alive; Redo creates the original name and
  reapplies the command-owned entity labels.
- Purge restores held Layer objects at their original indices, preserving later
  visibility edits through repeated cycles.
- SnapshotImport owns appended objects. UI-deleting an imported renamed layer
  does not invalidate its held object; Redo appends that object again.

Runtime identity/ownership metadata is not serialized and does not alter layer
semantic equality. Panel mutations seal the preceding model operation before
changing registry membership; Undo applies only the recorded command ownership.
Ordinary nested history carries this metadata through Finish. Mesh-shareback
continues to preserve the current global registry, matching source absorption.

The tray now routes registry/flag edits through these app methods, offers naming,
and deletes its selected layer rather than the last one. Current-context deletion
reassigns nested meshes/placements, modeled annotations and registry caches.
Purge descends nested geometry, respects saved-view hidden layers and checks
parked outer contexts. Locks/hidden layers prune Face/Edge/Group selection.

`baseline-native.json` is a literal pre-correction app replay of the first two
cases: panel addition disappears on Undo; the Purge cycle was already a
positive control. The old output is rejected on its recorded panel case, not
merely because the other two cases were absent. Three focused regression tests cover ordinary registry state,
nested entity reassignment and lifted ordinary context history. Workspace:
540 passed/2 ignored; original shareback and global-entity replays pass.

Boundaries remain open: no rendered tray gesture acceptance, actual file-import
UI proof (native import case uses the snapshot import boundary), complete global
layer deletion across parked contexts, all annotation selection/assignment
families, command-family-specific geometry/attribute ownership, shared-definition
rename/delete corners or full immutable-history performance. Image planes are
not modeled yet. No full layer/P15/style/shadow/image-plane claim. Tile fields
were untouched.
