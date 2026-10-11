# Empty-selection annotation Delete

Runtime d405c113 extends the existing annotation Delete consumer; no new named binary. **26 new typed source/App states and 26 rejection controls pass**, together with the retained **30 Delete states/30 controls**. One affected named build took 4.93s. All processes are terminal; no broad suite, GPU run or old source recapture.

Source tools/select.py639–654 gives extension picks priority, then actual selection, then hover. Hover is cleared before erase_entities; it does not become selection. The native actual egui Delete key retains extension-light precedence and calls explicit annotation target admission. Existing selected-target Delete remains unchanged. Empty-selection command snapshots stay empty through Undo/Redo. Source pointer acquisition uses actual Select.on_hover; native acquisition uses existing annotation cache and actual Tool dispatch, not a target injection.

Five cases are hovered Label/Dimension at root and nested contexts, plus selected Dimension outranking hovered Label. Root cases record before/Delete/Undo/Redo; nested cases additionally Finish/Undo/Redo. Lists and runtime identity, exact authored fields, selection, history, active mesh and group membership are compared recursively with exact JSON types. Offscreen key/controller evidence is not physical device/GPU evidence. Wider nonannotation hover erase and source entity expansion remain outside this annotation workflow.

`manifest.json` pins all records.zip entries, runtime/helper/checker sources, unchanged original source blobs and the frozen binary. Every archive entry was byte-authenticated against raw output before handoff. Existing inputs/original30 were extracted from canonical annotation-delete and annotation-hover archives; only the missing26 source controls were captured. Earlier material source failures remain in their existing canonical archives.

Replay: authenticate/extract records.zip into a writable directory; run `annotation_delete_fixture <directory>` and `python3 scripts/check-annotation-delete.py <directory>`. Required old inputs: delete-input.igz/original.json. Required new inputs: hover-input.igz/hover-delete-original.json. Old four native IGZ outputs remain generated for the existing reopen helper. Root must add the two new authenticated input extractions to its existing consumer runner; no twentieth binary/build.

Raw /tmp/ingetrazo-hover-delete-proof remains pending root release. Target/worktree/source retained.
