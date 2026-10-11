# Text creation prompt: operative App and source ownership

Runtime `79dd958e` implements the original two-click Text tool through the actual native dispatcher and existing modal. First click caches edge length, face area or coordinates; second click retains the Qt-float anchor/offset until multiline OK/Cancel. Enter remains newline. Whitespace-empty/Cancel resets acquisition without model/history mutation. Accepted creation adds one label command, preserves live selection, and Undo removes only its label selection. Global label membership crosses nested contexts without copying local geometry selection into parked meshes.

The source reference is unchanged `6be29fe`. `pick_edge` reads loose `scene.edges` only (viewport.py 9235,9486), including soft/occluded reference edges; grouped edges belong to separate group picking. `pick_face_any` transforms triangles but returns the prototype Face (8103,9140,10733), so a scale2 closed Group retains prototype area. Face area is the outer Newell area; model units/precision and imperial division follow core/units.py. No per-frame traversal or extra Scene clone is introduced by opening/canceling. First-click reference pickup is a cold loose-edge scan plus the existing face-placement query; acceptance still uses the existing full Scene history snapshot.

Strict retained acceptance: 77 original/native Text states, six root-created command→nested Undo/Redo/Cancel/Finish states with raw active vertices and selection, seven actual pinned Python save reopens, seven native raw-payload reopens, and one actual egui Modal focus/Escape/newline/OK/history test. Checker rejects 14 state mutations, six ancestor mutations, three reopen mutations and both real failed candidates. The affected old edit/style consumer remains 44 states/four actual reopens/54 rejecting controls; its original inputs were extracted unchanged from the prior canonical archive, not recaptured.

Appending a label initially discarded an existing label's unknown raw fields. The accepted narrow IGZ correction gives each TextLabel immutable Arc raw/baseline ownership, shared through clones/history, excluded from semantic equality/serde. Fresh null owners are shared once; fresh disk fields stay original. Per-owner overlay preserves metadata through membership changes without index matching. `failed-unknown-label-root-edge.igz` is a mandatory checker-rejected failure. `failed-group-edge-suggestion.json` retains the erroneous grouped reference pick and is also mandatory rejected. `failed-incomplete-context-finish.igz` records a fixture that exited one of two contexts; the corrected fixture exits both, matching original end_group_edit. Setup attempts are recorded separately.

`manifest.json` pins every archive entry, runtime/helper/original source bytes and both named binaries plus the focused test binary before target release. `records.zip` was verified entry by entry. Initial 66 source states were captured once; the closed-group eleven and ancestor six were acquired separately. The final helper reproduces all cases, but the initial source corpus was not recaptured.

Replay (extract records.zip to DIR; retained binaries are under /tmp/ingetrazo-ai-registry-target/debug/examples):

```sh
annotation_text_create_fixture DIR
/tmp/ingetrazo-parity-python/bin/python scripts/reopen-annotation-text-create.py DIR
python3 scripts/check-annotation-text-create.py DIR
```

For the unchanged edit/style replay, extract its prior canonical input/original/failure files to DIR/prior-edit, then run annotation_edit_style_fixture and its existing reader/checker there. Focused real modal test: `cargo test -j2 -p ingetrazo-gui --lib annotation_editor::tests::text_creation_real_modal_accept_cancel_and_live_selection_history -- --exact`, debug0/incremental0 in the reused target.

This proves source handlers, App command/history/save and actual egui modal ingress. It is not new GPU/glyph/occlusion or physical pointer-device evidence. The full P15 row and remaining Select hover/other already-recorded boundaries are unchanged. No broader transform or renderer scope is claimed. Worktree, final raw proof directory and reusable dependency target remain retained for root replay; terminal duplicate cleanup waits for release.
