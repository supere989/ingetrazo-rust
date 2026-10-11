# Integrated annotation acquisition and Move

One cached CLI/two-consumer build at `01211f52` took **14.02 seconds**; the affected replay took **1.98 seconds**. All **49 acquisition states and 77 selected-Move states** match retained source exactly, with 49/77 rejected mutations and mandatory prior failure controls. No source corpus was recaptured for integration. The single new label precision regression also passes; its log reports 334 unrelated tests filtered out.

Independent review corrected two defects before this build: dimensions must be classified against their original picked geometry before exclusive annotation targeting, and label position must narrow each authored operand before adding, as source and rendered text do. The new seven-state anchored-edge control verifies raw ordered vertices through grab/preview/Cancel/commit/Undo/Redo. The earlier 42 acquisition states are unchanged. Label arithmetic preserves authored float64 data.

The new route uses actual cached source screen rectangles/segments and the production App/Move dispatcher. This is headless source QWidget picking and native app behavior, not physical desktop mouse acceptance. The original 77-state precision, riding, nested/parked and invalid-geometry boundaries remain accepted. P15 still needs its remaining style/value/transform and rendered/platform clauses.

Shared history code did not change, so the other thirteen-consumer workflow checks were not repeated locally. The published stable integration CI receives the new acquisition consumer alongside the prior set. Native Mac rendering checks have their own retained runner verdicts. `records.zip` authenticates all bounded source inputs, outputs and execution logs; `acceptance.json` pins binaries and source archives.
