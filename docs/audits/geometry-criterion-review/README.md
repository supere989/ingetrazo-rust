# Finite Follow Me and reconstruction criterion review

Two independent read-only reviews assessed the unchanged P10 and P11 criteria against retained source comparisons, current runtime admission and application history controls. Root inspected the referenced comparisons and tests and accepts both finite criteria as **Verified**. This consolidates evidence already present; it does not add new implementation, source capture or local test execution. `retained-evidence.json` pins the earlier comparison revisions and bytes explicitly. The current `67afd7fa` published workspace test/example-coverage step also passes; the remaining semantic CI stages are still running.

## P10 — Follow Me sweep

Unchanged criterion: “Sweep straight/bent/closed paths with correct frames, joins and manifold topology.”

- [The 20-case source/native receipt](../python-followme-evidence.json) covers straight extrusion, bent miter, holed tube, closed torus, nine revolutions across three plane orientations/radius pairs, two wire revolutions, two attached closed-solid sweeps and three open-sheet attachments.
- [The independent checker](../../../scripts/check-python-followme.py) compares native manifold shapes for all 17 closed bodies and complete oriented face rings for the three open-sheet cases. Straight, bent, tube and torus symmetric differences are zero; the maximum retained difference across the rotational cases is about 2.91e-7, within the unchanged 1e-5 boundary. A straight-for-bent negative rejects.
- [The runtime](../../../crates/ingetrazo-tools/src/followme.rs) validates raw closed mesh topology before publication; existing frame, miter, path closure, lathe-pole and attached-profile tests protect those paths. This is not solely acceptance of a normalized external mesh.
- Actual manual release, Cancel, Undo and Redo are covered by [application tool history](../../../crates/ingetrazo-gui/src/tool_history.rs). The active workspace suite passed on the repair revision.

The tracker had left this finite row Partial for arbitrary path robustness and rendered gestures. Those remain broader requirements, but are not missing clauses of this row's stated criterion.

## P11 — Intersect, heal, rebuild and pieces

Unchanged criterion: “Actual face intersections, cap rebuilding and separated piece outputs; regression corpus and undo.”

- [Intersection evidence](../python-intersect-evidence.json) contains four literal source segment cases and a closed-shell mutation: ten faces, exact cut stations and zero source/native symmetric-difference volume. The independent [checker](../../../scripts/check-python-intersect.py) rejects a wrong-output negative.
- [Eight healing cases](../python-heal-evidence.json) compare complete source rings, holes, winding and paint. [Four Pieces cases](../python-pieces-evidence.json) compare names, world-space faces/edges and paint, with actual app single Undo/Redo and repeated no-op acceptance.
- [Planar cap reconstruction](../../../crates/ingetrazo-core/src/planar.rs) has a missing-face/area/opaque-edge-data control. Actual source cap/seam behavior is also exercised by the retained [26 Push/Pull workflows and 65 persistent app operations](../pushpull-finite-corpus/README.md), whose exact geometry/attributes and Undo/Redo pass the fresh integrated repair consumer.
- [Actual GUI geometry actions](../../../crates/ingetrazo-gui/src/geometry_actions.rs) exercise Rebuild, Intersect and Heal transactions, exact Undo/Redo and invalid-plane atomic refusal. The active workspace suite passed on the repair revision.

Seven orientation and thirteen simplification comparisons remain additional evidence. This finite behavior criterion does not require reproducing every private original cap-rebuild API or arbitrary concave/annular/randomized geometry. Those wider behavior and platform requirements remain open under the existing release gates; no whole-port completion claim follows from these two row dispositions.
