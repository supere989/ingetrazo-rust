# Production composer transient preview acceptance

Fixed production source: `4e4889d`. Native fixture invokes the production composer and precise CAD2D factory; toolbar activation and keyboard Undo/Redo routes are fixture instrumentation. Real XTEST pointer events place a 50×35 mm rectangle, hold its preview, cancel, commit and Undo. Original uses its actual ComposerWindow toolbar and canvas.

All 19 samples on each of four preview/committed boundaries are visible in both implementations. Cancellation and Undo have zero boundary ink. Native warm preview reuses its upload and increases cache hits. Five pixel negatives reject black startup, missing preview/commit, stale cancelled geometry and zero-byte PNG. Baseline now waits for presented white paper rather than only published state.

Original Ctrl+Y Redo timed out in the standalone fixture; this is an open keyboard/fixture integration boundary, not an established product defect. Native Redo passes. Pose/color changes after committing a shape are not covered. These captures use software Vulkan for native and software Qt/OpenGL for original. All nine full shape-image antialiasing comparisons remain failed separately; no whole-image equality or hardware claim follows.
