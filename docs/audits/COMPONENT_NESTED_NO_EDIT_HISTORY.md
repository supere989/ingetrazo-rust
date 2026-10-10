# Nested no-edit component history

The source bake in `core/scene.py::_bake_container_xform` is outside command History. Actual `Viewport.begin_group_edit/end_one_group_edit` preserves zero entries after an untouched leaf or container. A prior parent AddFaceCommand or MoveGroupCommand remains one entry.

The frozen six-case source protocol is `scripts/probe-python-component-nested-noedit.py` and `component-nested-noedit-source.json`. `component-nested-noedit-baseline-failure.log` retains production3c2deaa incorrectly adding history for nested no-edit container.

The Rust fix advances the outer session's authored-change baseline only for representation changes made by a no-edit inner session. A child baseline is replaced only if it still exactly matches the inner entry's snapshot. Prototype/definition entries likewise advance only when their previous values match. Prior parent child moves, geometry commands, newly authored prototypes and definitions are not hidden. Original full session snapshots remain intact for native Cancel and authored Undo.

Source six controls PASS; GUI integration13PASS; GUIlib96PASS/1clipboardignored. This is controller/Scene history evidence with exact logical placement sharing checks and native Undo/Redo, not rendered or pointer proof.
