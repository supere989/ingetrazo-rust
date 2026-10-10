# Operative image Move and selection-removal control

Pinned original 6be29fe437a7cd992d4e079f71720821b25216b9 actual MoveTool click/hover/cancel/Control-key/History versus production Rust app dispatch/undo/redo. Eight required families match exactly: selected/implicit image, locked implicit refusal, explicitly selected locked/hidden image, cancellation, copy exclusion, fractional multi-hover. Relative Undo preserves a later nonhistory shift. 559 workspace tests passed, two ignored. Picking supplied viewport ray and world gesture coordinates; not actual OS/GPU rendered gesture proof.

Fractional baseline failure retains grab-relative versus original incremental QVector3D rounding mismatch (one float ULP propagating through commit/undo). The correction applies actual float32 increments and float32 inverse-preview cancellation. Literal prior0aafeb63 source has no image Move targets and is rejected. The old software path stays historical, no fabricated unsupported outcomes.

Six generic Ctrl+Shift controls instantiate actual original Face/Edge/Group with selected/unselected variants, then supply the owned target to SelectTool._pick to isolate modifier policy. Native SelectTool uses actual raycast geometry. All preserve source remove-only behavior. This proves the modifier law, not original screen-pick admission/camera semantics.

Rotate, Scale, mixed command families, snapping/reference borders, actual physical-pointer routing and native image drawing remain open. No texture/resource/GPU proof claimed here.

Replay:
```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-image-move.py original.json
XDG_CONFIG_HOME=/tmp/isolated-image-config cargo run -p ingetrazo-gui --example image_move_tool > native.json
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-selection-remove.py modifier-original.json
cargo run -p ingetrazo-gui --example selection_remove > modifier-native.json
python3 scripts/check-image-move-tools.py original.json native.json modifier-original.json modifier-native.json
```
