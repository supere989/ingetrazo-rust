# Source pure-image preview across group Finish

Pinned original6be29fe437a7cd992d4e079f71720821b25216b9 actual Viewport.begin/end_group_edit, MoveTool click/hover/cancel and History versus production Rust app component editing/tool dispatch/Undo/Redo. Ten mandatory contexts: plain, linked component, folded mesh command, nested, nested folded × live Finish / explicit Cancel before Finish. Subsequent hover/commit/Undo/Redo/Cancel are compared exactly, including float32 incremental rounding. No image/history command exists at Finish when only preview is active. Literal prior2f11ed50 fails all five Finish-live contexts, while cancellation controls remain positive.

The opt-in preserves only pure image Move global previews across context publication, with source frame and tool target ownership. Local mesh/group/section or mixed targets retain existing safeguards pending required full remapping implementation. This is staging, not an exclusion from full parity.

Cold metadata collection now visits held image objects owned by live tools without per-frame allocation. Source GC control clears all Scene selection/membership/history roots, leaving only actual MoveTool._images; object lives until Tool.Cancel and then is collected. Native metadata matches both controls. Literal previous collector loses tool-owned metadata early. This proves explicit active tool roots; full membership-reindex and arbitrary detached target mutation still require separate gesture corpus.

Controller inputs are supplied directly; OS pointer/rendered gestures, native image raster/outlined selection, Rotate/Scale and resource performance are open. Existing refusal nine-case and picking29 controls remain required.

Replay:
```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-image-move-context.py original.json
XDG_CONFIG_HOME=/tmp/isolated-context-config cargo run -p ingetrazo-gui --example image_move_context > native.json
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-image-tool-roots.py roots-original.json
XDG_CONFIG_HOME=/tmp/isolated-roots-config cargo run -p ingetrazo-gui --example image_tool_roots > roots-native.json
python3 scripts/check-image-move-context.py original.json native.json roots-original.json roots-native.json literal-old.json roots-literal-old.json
```
