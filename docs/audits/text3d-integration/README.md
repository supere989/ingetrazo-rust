# Native 3D Text and rapid Scale integration

Native 3D Text replaces both status-only insert actions with a parameter dialog and actual extruded geometry. Each nonspace glyph is a child group; insertion follows the cursor, aligns to a picked face, commits once and cancels without a document edit. Regeneration keeps the container identity/placement, original-compatible parameters and fingerprint. Edited letters refuse regeneration. Geometry is built once per accepted insert/edit before changing context, tool or history.

Runtime `abd6f65f`, final application consumer `a3cab394`. One joint named build supplies the font, application and Scale consumers; the application probe had one incorrect use of the Scale-specific pending-preview flag for Move. The retained failed assertion was corrected to the actual Move placement descriptor plus unequal live/baseline matrices, and only that example was rebuilt. No product behavior was changed to satisfy the control.

Acceptance:

- One workspace suite: **933 passed, zero failed, two existing clipboard skips**, 32 nonempty test binaries; examples compile and doctests pass.
- Ten fresh original/native font cases match every ordered f32 vertex, face/hole loop, edge, soft flag and fingerprint exactly. Original captures are reused. The retained compiled flattening counterfactual and 30 mutations reject.
- Actual application insertion/hover/Cancel/click, one group per glyph, original-compatible IGZ, 20 m edit, Undo/Redo owner identity, unchanged/invalid refusal, open-container owner editing, real egui dialog Escape, flat text, manual vertex/paint/erase/individual-pose refusal, common-pose allowance, and pending Move→edit→Cancel/Undo pass.
- Python's unchanged reader reopens three workflow documents plus a fourth decimal-half/Unicode/paint fingerprint boundary. All remain pristine editable text; parameters, letter membership, dimensions and placement are checked.
- Fresh merged Scale consumer: nine recipes, 52 states exact. The modal input boundary and retained native modifier timeline coexist in this build.
- The earlier published `9c74158b` checkpoint is green on Linux and all six Apple jobs. Published `0310291e` also passes its semantic workflow: Linux acceptance plus ARM/Intel cross compilation and actual native framework linkage, five jobs green. See [CI receipt](published-ci.json). Main is unchanged.

Read-only verification: `python3 scripts/check-text3d-integration.py`.

Review corrected dialog/navigation cursor ownership, active model-preview contamination of edit history, Qt-f32 fingerprint boundaries and empty metadata recognition before this batch. Direct nested target edits outside the active parent, including a text-owner exit below one ancestor, explicitly refuse before mutation; select the container in its parent for the supported path. A text edit retires the pending transform tool after admission.

Remaining: real native Mac/Windows outline acceptance; synthetic/mixed-family fallback, variable fonts and complex scripts; enumerated font chooser (the dialog currently accepts a family name); arbitrary nested owner/context selection; physical OS UI and full rendered placement. This is bounded functional/geometry acceptance, not whole-feature or platform parity. Portable reference-image/section depth and strict Metal pixel residuals remain separately tracked. All 37-area classifications and five release gates remain unchanged.

Archive records include complete build/test output, the initial probe failure, fresh native results, four IGZ documents, original-reader checks and exact binary provenance. Temporary copies are released only after byte verification; reusable target and source trees remain.
