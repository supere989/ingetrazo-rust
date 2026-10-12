# Native single-picture manipulation acceptance

The actual App/egui consumer matches all 21 frozen original cases and 167 states for picture picking, dragging and resizing. All 21 native saves match the actual original reader, and the strict checker rejects 18 mutations. The cases cover selection and press origin, higher and equal z, hidden and locked items, Ctrl toggling, Shift resizing, independent aspect, minimum sizes before snapping, guides, sibling edges, drawing endpoints and midpoints, and Undo/Redo. Source Escape clears selection while keeping the drag grab; explicit native transaction cancellation is tested separately.

The two declared unknown metadata records survive the actual saved IGZ. Eight unchanged frames retain image decode, composite, path I/O and upload counters. Separate native ownership controls cover two completed gestures in one event packet, two Undo steps, explicit Cancel and a lost pointer.

The affected authoring replay also passes all 44 states, all 15 stacking categories, three original-reader pairs and 16 rejecting controls. It exercises the actual Add, Outline and Replace widgets through App update frames and an owned picker process. The older pointer replay passes its 13 cases, 89 states and 15 negative controls.

## Runtime and binary provenance

Gesture and old-pointer results were compiled at febcf5b72b5fe9f34e0ddaa53388912355775cb3. The final shared three-example build took 6.148 seconds; the preceding build took 6.531 seconds. The authoring consumer was then compiled at d28b5edc25b220f69f64e6d5bbf200437781193c in 0.670 seconds. The production files are unchanged between those revisions; only the authoring example changed. Every binary has its own retained SHA-256, build receipt and compiled revision in native-manifest.json.

The source archives are authenticated separately: picture-gestures/source-records.zip contains the literal 21-case oracle and retained guide-fixture correction; picture-authoring/source-records.zip contains the accepted authoring oracle. They are referenced by hash and are not copied into this native archive. Session settings, caches, binaries and worktrees are also excluded.

## Retained failures and the consuming fix

The first two authoring attempts failed because Outline did not change the border value. Their build logs, runtime errors and saved state are retained. A bounded diagnostic at 968225bb24da222b6acca3ec1b269f35f2c89bb6 showed that sending PointerMoved and Press together moved the floating window by the pointer delta and lost picture selection. The final consumer sends a hover frame before pressing and asserts that the control rectangle stays fixed. Its trace confirms stable Outline and Replace controls and the selected picture. The release ownership guard is not claimed as the causal fix for these failures.

## Limits

This is finite single-picture model, history, input and save acceptance. It does not cover multi-item manipulation or prove that coalesced physical OS input behaves like the separated consumer frames. Label and elevation font-metric snapping remain open; the drawing cases cover a real parallel-view edge endpoint and midpoint. Physical PNG/JPEG/PDF parity belongs to the separate picture export proof and remains open.

Both Apple metadata checks passed at d28b5edc: ARM in 2.246 seconds and Intel in 2.187 seconds. These checks establish Rust metadata compatibility only, not Apple C ABI, linking, native execution or GPU acceptance. No new build, native execution, source acquisition or checker/reader rerun was performed while archiving this package.
