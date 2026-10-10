# Active component history contract review

Pinned original: 6be29fe437a7cd992d4e079f71720821b25216b9. `probe-python-component-active-history.py` constructs the actual MainWindow, uses its production QAction connections, and freezes eight action-dispatch controls plus a real Qt Ctrl+Z key event to the Viewport widget. Offscreen Qt cannot create a GL context; this is event/command evidence, not rendered or physical input proof. Full outputs and provenance are in `component-active-history-source.json`. The earlier detached-context face-loss record remains unchanged.

## Operative source behavior

- MainWindow builds Undo with QKeySequence.Undo and connects `_on_undo` (main_window.py843–847,3945–3955). It offers the active tool `on_undo` first, then calls the single global History. It does not leave the edit context. Redo calls global History directly.
- A Line first point consumes Undo to reset the chain without popping a model command (`tools/line.py121`). This works even when a prior document command is available.
- History records `_history_mesh` at execute-time and `_in_command_mesh` temporarily selects that target for Undo/Redo (`core/history.py189–246`). A prior loose AddFace command can therefore undo and redo while an unrelated classic, container, or placed component context remains active. The active mesh reference stays unchanged.
- Viewport sets `_edit_undo_mark` after auto-MakeUnique (`viewport.py11151`). A changed placed-leaf sharing session absorbs post-mark commands into one ReshareInstanceCommand on exit (`11251–11257`). Two authored faces become one finished Undo, restoring the prototype and shared instances.
- Classic and container sessions without a sharing handle do not fold their command history. Two authored faces remain two finished Undo steps. One Undo removes only the last face; Redo restores it. Container representation baking itself is outside History.

## Retained invalid source boundary

Undo of automatic MakeUnique while the ordinary copied group remains open restores the Group's old prototype/xform but leaves scene.mesh pointing at the previously active detached world mesh. A following AddFace succeeds there and is lost from the model on exit. The actual QAction replay confirms the same loss as the earlier operative-handler record. This is literal source failure evidence, not permission to reproduce silent data loss.

## Native differences still open

Native ComponentEditSession currently parks the outer stacks and creates a fresh local stack. Finish collapses all changed session geometry into one document snapshot. Thus (1) prior document Undo is unavailable during editing and (2) classic/container multi-command Finish history differs from source. The active ordinary-entry MakeUnique command is parked too. Local safe Undo/Redo and finished placed-leaf batching do not establish equivalence of these architectures.

A faithful implementation needs global chronological entries with edit-target identity, preserving active and parked meshes independently; context entry and no-edit exit must not clear global Redo. Finished classic/container commands must retain individual order, while actual placed-leaf share-back remains one entry. Existing full-session Cancel is a separate native integrity behavior, not an original Undo contract. Nested mark handling, structural commands deleting/replacing the currently edited placement, and cross-context target invalidation require additional original controls before implementation is called complete.

Safe UI boundary: target-preserving mesh commands may cross entry without exiting context, as source demonstrates. Commands changing the active placement's mesh identity require explicit atomic rebind validation or a visible refusal; silently losing an authored face is unacceptable. This report does not change production history policy or classify a refusal/rebind choice as exact source parity. Parent review is required for that concrete source-failure boundary; no broad component/P08 promotion follows from these controls.
