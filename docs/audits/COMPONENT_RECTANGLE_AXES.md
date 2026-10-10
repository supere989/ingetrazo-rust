# Reentered drawing-frame Rectangle stage

Source `core/group.py::frame_axes` derives orthogonal unit axes but retains the raw affine encoded frame. `core/axes.py::plane_axes` projects context red into a drawing plane, with green fallback. Rectangle uses that shared helper (`tools/rectangle.py::_plane_axes`), context axes for snapped-plane candidates and axis-key locks. Explicit vector input remains world XYZ.

Rust Rectangle now consumes the Scene runtime drawing axes on activation/click/hover, computes in-plane corners/VCB/quadrant/square inference from context red/green, follows context axes for snapped-plane candidates, and resets locks to the current context blue. Viewport pre-click arrow keys pass the current context directions. Stored matrices are unchanged. Surface/topology and texture plane axes remain world helpers.

The source viewport's actual `_current_work_plane` still has world-axis camera/ground fallbacks; these have deliberately been preserved. A world-plane hover therefore wins over the default context plane while context red still determines rectangle heading. This follows operative code rather than assuming the axes module's broad comment changed all viewport fallbacks.

`probe-python-component-rectangle-axes.py` runs actual original Viewport edit reentry, axes sync, Rectangle key/click/hover/VCB for twenty controls: five identity/scaled/reflected/custom/skewed stored frames × default/red/blue/world-hover planes. `component-rectangle-axes-source.json` records raw frames, controlled world points and exact ordered source face rings. Native compares cyclic rotation with preserved winding (4e-6 Qt tolerance), world dimensions2×3, inert preview and world reset. `component-rectangle-axes-baseline-failure.log` retains earlier production3378428 failing context red heading. This is controller proof, not actual pointer/timer/rendered proof.

Current tools lib/tests PASS; GUIlib96PASS/1ignored; GUI component integration13PASS. General snapping inference, Shift axes, other planar tools, standard views, and actual reentry gestures remain followups; no broader parity row promotion.
