# Global history in WORLD group editing

Followup to COMPONENT_ACTIVE_HISTORY.md. Full component and Move parity remain open.

## Source evidence

Original source is pinned at 6be29fe437a7cd992d4e079f71720821b25216b9 with Python 3.13.13. The active-history probe uses actual MainWindow QAction connections and one Qt Ctrl+Z event: 13 offscreen semantic controls, not rendered hardware gesture proof. Six additional nested entry-bake/Undo controls retain exact world geometry, placement, raw axes and sharing observations. Operative source is views/main_window.py history dispatch, core/history.py execution and Undo/Redo, Viewport group entry/exit, and MoveGroupCommand inverse placement composition. Executable probes retain those actual source recipes.

## Implemented contract

Scene carries a serde-skipped history_context path of stable placement UUIDs. Commands retain explicit root or active-context mesh targets. Ordinary/classic and container Finish lifts each command into its parent context without collapsing order. Changed placed/shared leaf Finish retains the original single shareback command fold. Prior loose commands can Undo/Redo against their parked mesh while the active context remains unchanged.

Source entry bakes occur outside History. Finish applies outer-to-inner bake records to older snapshots and parked ancestors, preserving world geometry, private ownMesh identity, independent child placements and raw axes metadata. Nested parent Move Undo composes its inverse translation onto the baked representation, matching the six-control original probe. Cancel retains legitimate ancestor history steps while restoring the cancelled context checkpoint.

Candidate and historical snapshot validation precedes context exit or stack mutation. Invalid geometry, stale context IDs and reference-changing active history targets refuse visibly and atomically. Composer-local history and Line first-point Undo retain priority.

## Explicit integrity boundaries

Original active autoMakeUnique Undo can detach the edited mesh and lose subsequently authored faces. Its failure remains frozen in component-active-history-source.json. Native refuses that active reference-changing operation; finishing or cancelling first permits normal ordered history. This is an intentional integrity safeguard, not literal source parity.

Original unchanged sharing-leaf Finish can retain a SnapshotMutation Redo bound to a detached temporary WORLD mesh. Actual MainWindow Redo changes that dead mesh while neither visible placement changes. component-finish-redo-source-failure.json retains this separate failure. Native validates and rebinds pending snapshots to the parent/prototype so Redo and subsequent Undo affect both linked placements. This is another bounded correction. Native whole-session Cancel remains a source-absence boundary.

## Verification and limits

The checker compares 18 frozen source ordering observations and two explicit native integrity boundaries. It rejects collapsed ordinary Finish and missing-case controls. Replay against pre-change 6afb3c1 fails; component-global-history-baseline.json and its log retain raw output and executable SHA. component-global-history-native.json retains the fresh corrected executable SHA and outputs.

Twelve dedicated regressions cover stable IDs, ordering, nested bakes, prior loose commands, Cancel checkpoints, stale targets, Line priority, unchanged sharing Redo, live-preview atomic refusal and invalid historical/current geometry. Existing array/nested tests now assert source outside-history bake semantics. The full private workspace run passes; exact totals are reported with the review commit.

History still uses whole-Scene snapshots. Entry clones cancellation checkpoints and Finish materializes tagged snapshots. This stage makes no performance or GPU upload claim. Broader structural edits, imported legacy ambiguity, cross-resource history and actual rendered context/history gestures remain open.
