# Composer Select-item pointer transactions

13 operative original recipes / 89 ordered phases match fresh native full-App egui output exactly, without coordinate tolerance. Thirteen initial corruption controls, extended to fifteen for release-boundary history, reject wrong movement, press origin/selection, stacking winner, hidden ownership, Escape rollback, history/type/snapshot errors, foreground mutation, lost-release Undo, outside-paper motion and coalesced packet history.

Source recipes cover unselected and selected drag, a first move beyond the old item bounds, click-only, locked/hidden frames and overlays, reverse array versus z order, equal-z insertion order, Escape during/before movement and an out-and-back no-op. Actual QtTest viewport presses are spontaneous, reach original FrameItem.mousePressEvent and acquire the real mouse grabber. Tool mode is Select, view interactive and view-edit absent. The previous fabricated event capture was inert and rejected before item dispatch; both diagnostics are retained. Source model frame rendering alone is suppressed by the fixture subclass; no pixel claim.

The source ordinary item drag owns the initial press, selects then, and uses that origin for every move. Locked/hidden items cannot be picked, but still supply source axis snap targets. The literal page/margin/guide/item target and 2mm snap law is cached on press; no target rebuilding per motion. Painting's existing visible order is reused for picking. Hidden targets use the new all-item ordering wrapper. Existing f64 model positions and metadata are preserved; no projection, tessellation or whole-Scene copy is added.

Source Select Escape drops selection without rolling back the active item's coordinates; its real release commits and reselects. This pointer route now preserves that behavior. Explicit cancel_composer_edit and placement/property rollback remain unchanged. A real owned release finishes immediately: two complete gestures in one egui packet make two Undo entries, including the first intermediate position. Unrelated property-editor releases do not finish their draft.

Native supplementary guards prove blank press preserves selection until release, an item remains pickable outside paper but inside canvas, an actual Foreground layer at the pressed point prevents grabbing the underlying item, and consumed-release capture loss finishes the last admitted pose with working Undo. Foreground proof is layer identity/pointer ownership, not a child button callback or OS popup test.

Only the first changed move takes the active-sheet snapshot; press/click/idle take none, continued drag reuses it, and out-and-back leaves no history entry. Existing composer::* tests ran once after the final runtime correction: 34 pass, zero failures (0.54s); no broader GUI/workspace suite. The final named replay runs in about half a second. Exact logs and binary/source hashes are retained, including the local borrowed-input hang and mechanical compile failure.

## Replay

Authenticate/extract source-records.zip, then:

```sh
composer_pointer_workflows SOURCE/source.json OUTPUT/native.json
python3 scripts/check-composer-pointer.py SOURCE/source.json OUTPUT/native.json OUTPUT/check.json
```

Build only `cargo build -p ingetrazo-gui --example composer_pointer_workflows` (jobs2/debug0/incremental0). Final provenance identifies source commit, full binary hash and accepted-final members. Manifests authenticate complete ZIPs and every member.

P04 remains bounded: this is ordinary single-item body acquisition/drag/history, not full rendered composer acceptance. Resize handles, multi-item/box/Ctrl/Alt selection, view-edit navigation and model-drawing snap remain separate existing gaps. No tracker, root or CI files were changed.
