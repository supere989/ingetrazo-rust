# Component CI history presentation repair

Exact public source ed469b91 reproduces the component editor fixture failure at
line 68. `baseline.log` retains the literal failed assertion; `candidate.log`
retains the successful complete original component acceptance replay.

The only unequal Scene fields are selection, selection_order, version and
view_version. Entry clears selection; source ReshareInstanceCommand restores
only prototype mesh and selected placement, and increments scene.version.
It does not restore pre-entry selection or invalidation counters.

Fresh pinned original translated, rotated and tilted/scaled nested controller
cases now select the parent before entry and record commit/Undo/Redo counters
and selection. Native cases record the same invariants. Numeric counter totals
are implementation-specific; monotonic increments and empty selection are
checked explicitly. Two new negatives restore selection or old version and
are rejected in addition to the existing four affine/precision negatives.

The editor fixture still compares the entire remaining Scene after Undo/Redo,
including mesh shares, component definitions, private children, placement IDs,
geometry, annotations and resources. No production implementation or tolerance
changed. The complete original loader then reopens all saved edited/undone/
unique/copied fixtures and checks real Python mesh identity.

This is controller/history and actual native-document reopen proof, not rendered
UI gesture acceptance or a full P12/P15 claim.
