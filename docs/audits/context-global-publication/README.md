# Document globals across edit contexts

Source f463bf4, original pinned6be29fe437a7cd992d4e079f71720821b25216b9. Actual original Viewport begin/end plus command History versus native IngeTrazoApp entry/Finish/Undo/Redo; no rendered UI/input claim. Four direct/nested ordinary/folded cases,20 states+4entries exactly match guide coordinates, leader-text coordinates/content, geographic point coordinates/name/description, geographic datum. Four strict payload mutation negatives and actual literal-old source entry failure retained. Optional null layer is the only representation normalization.

Original begin_group_edit changes geometry owner, not global guide/text/georef entities. Rust now carries document globals through entry and publishes them at Finish; false schema refusal removed. Ordinary annotation-only commands remain individual chronological undo entries. Mesh-shareback wrapper retains document globals by its original command ownership rule; no blanket removal from ordinary Scene history.

Implementation also carries modeled paths/terrain/sheets/savedviews/units. This corpus has not independently exercised their edit command families. No full P15 or native Metal claim. Source image_planes/style/shadows/cutlist/units richer options lack full Rust Scene equivalents; these remain original document/renderer scope, not silently authorized divergence. Existing raw native document preservation is separate from operative editing support.

Replay inside ingetrazo-publish, cwd worktree:

```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/probe-python-context-globals.py /tmp/fresh-global-original.json
CARGO_TARGET_DIR=/tmp/isolated-global-target cargo run --locked -p ingetrazo-gui --example context_globals_fixture -- /tmp/fresh-global-native
python3 scripts/check-context-globals.py /tmp/fresh-global-original.json /tmp/fresh-global-native/native.json docs/audits/context-global-publication/literal-old-entry-failure.log
```
