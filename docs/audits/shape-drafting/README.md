# P06 shape drafting consumer

Runtime `e7c9c749` → consumer `714ad45f` → strict checker `11619fef` → repair `313c287c`. Base `c7f046d8`; original pin `6be29fe437a7cd992d4e079f71720821b25216b9`. `receipt.json` authenticates the canonical 56-member archive and both owner binaries; `records.zip/provenance.json` freezes changed runtime/script hashes. All original source-member hashes were rechecked against the unchanged pinned checkout.

The actual original MainWindow activation, shape handlers and Viewport history match native App handlers over 66 recipes / 488 phases. Circle, Polygon, TwoPoint Arc, ThreePoint Arc, Center Arc, Pie and Rotated Rectangle have seven real native egui hover/press/Cancel input controls. The 50 checker adversaries reject absent geometry/metadata/history/curves, wrong types and inert egui ingress. Existing22 Line/Rectangle/Freehand drafting records also match after the shared source-plane repair. Narrow source admission (1), Circle (2) and Arc (2) tests pass. Final affected named build took10.07s; source admission test build3.45s; Circle test build4.87s, Arc reused it0.06s.

The recipes cover arbitrary/oblique and locked planes; pre-acquisition screen ring; radius, segments and angle VCB; cancellation, repeat, hot Arc segments and Undo/Redo; tangent, semicircle, curve continuation and corner trim; nested contexts; axis/reference lock colors. The new drafting ingress follows original exact-f32 all-point coplanarity, carries its plane through arrangement, and selects its Qt projection/segment metadata policy only when incoming mesh and segment rows are exact-f32. Native-double authoring retains the strict native path and its stored coordinates. The source virgin-plane AddFace and prior orphan cleanup semantics are explicit rather than fitted to resulting counts.

Retained failures include the initial Ray field build error, first source-plane refusal/mismatches, and the root checker’s373/376 +59/61 intermediate outcomes. The original first corner-repeat recipe did not deliver the source press+double sequence; it is retained unchanged, and the addendum contains the correctly delivered witness. First53 records lack a preview_color field; color comparisons are explicit in the addendum and remaining captures.

Boundary: geometry uses the accepted five-decimal keys, oriented rings, flags/metadata and curve grouping. It does not claim raw object UUID equality or desktop/whole-frame renderer parity. Original recipes supply explicit world/pixel observations. Native egui records exercise real viewport ingress but are not source screenshot comparisons. Native context entry selects Select, so the nested consumer explicitly reactivates the tested shape inside the admitted context; no component-edit behavior was changed.

## Replay without source recapture

Extract `records.zip` into an owned directory `$PROOF`. Use your integrated named binary `$SHAPE` (or the owner binary path in receipt):

```sh
"$SHAPE" "$PROOF/source.json" "$PROOF/fresh.json"
python3 scripts/check-shape-drafting.py "$PROOF/source.json" "$PROOF/fresh.json" "$PROOF/fresh-check.json"
"$SHAPE" "$PROOF/source-addendum.json" "$PROOF/fresh-addendum.json"
python3 scripts/check-shape-drafting.py "$PROOF/source-addendum.json" "$PROOF/fresh-addendum.json" "$PROOF/fresh-addendum-check.json"
"$SHAPE" "$PROOF/source-remaining.json" "$PROOF/fresh-remaining.json"
python3 scripts/check-shape-drafting.py "$PROOF/source-remaining.json" "$PROOF/fresh-remaining.json" "$PROOF/fresh-remaining-check.json"
"$DRAFTING" docs/audits/drafting-expanded-corpus.json "$PROOF/fresh-drafting22.json"
python3 scripts/check-retained-drafting.py docs/audits/drafting-accepted-corpus.json "$PROOF/fresh-drafting22.json" "$PROOF/fresh-drafting22-check.json"
```

The owner named build was `cargo build --locked -p ingetrazo-gui -p ingetrazo-core --example shape_drafting_workflows --example drafting_acceptance_fixture`, with target `/tmp/ingetrazo-native-font-merge-target`, jobs2/debug0/incremental0. No broad workspace suite, rendering capture or cloud run was performed. Existing passed Line/Rectangle/Freehand source captures were reused.
