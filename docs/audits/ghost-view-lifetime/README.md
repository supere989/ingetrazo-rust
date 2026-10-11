# Global hidden-view switch lifetime

Root followup `9a2b107e` copies the two document-global ghost switches through ordinary model Undo/Redo, component entry/publication, Cancel and Finish-all. Existing native whole-Scene snapshots previously revived older switches, while entry initialized them to false. Presentation-only context exits preserve the flags without adding a geometry history command.

`reference.py` invokes the pinned original `SnapshotCompound`, `History` and real nested `Scene` edit operations. All seven states in `original.json` retain the live switches at the expected context depth. The original pin remains `6be29fe437a7cd992d4e079f71720821b25216b9`, with no source modifications. Native regression `document_presentation::tests::ghost_flags_stay_live_across_model_history_and_nested_contexts` also checks the additional native Cancel path, exact geometry Undo/Redo and unchanged history depth; it passed in the combined batch at source `1ab860bb`. Original scope covers history and entry/Finish lifetime, not an original Cancel gesture or rendered UI.

Replay original inside `ingetrazo-publish` from the integration checkout:

```sh
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python docs/audits/ghost-view-lifetime/reference.py /tmp/ghost-view-lifetime-original.json
```
