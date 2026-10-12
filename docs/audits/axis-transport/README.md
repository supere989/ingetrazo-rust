# Axes transport diagnostic

This reads the actual original GL axes VBO and uploaded uniforms at the frozen default-perspective camera pose. The delegating draw hook preserves arguments, return values and GL array-buffer binding. Model/history/camera observations must remain unchanged. It captures no screenshots.

The native optional mode calibrates the actual host viewport, waits eight ordinary frames, acknowledges the previous frame fence and reads the accepted uploaded axes buffer. Its matrix is the issued column-major push-constant value, not a GPU uniform query. Normal runs omit transfer-source usage and all readback.

Root-only commands (SOURCE is authenticated extracted original six-case capture):

```sh
QT_QPA_PLATFORM=xcb /tmp/ingetrazo-parity-python/bin/python scripts/reference-axis-transport.py SOURCE SOURCE_AXIS_OUTPUT
ai_bridge_capture_workflows SOURCE NATIVE_AXIS_OUTPUT --axis-transport-only
python scripts/compare-axis-transport.py SOURCE_AXIS_OUTPUT NATIVE_AXIS_OUTPUT
```

Comparison is diagnostic only: positions, per-axis colors, matrix bits, clip flags and extent. A differing report does not authorize geometry fitting or establish pixel parity. Existing six-case screenshot behavior and frozen inputs remain unchanged.

The follow-on records actual GL state at each delegated axes draw (depth function/write, blend equation/factors, line width/smoothing, multisample/coverage and scissor). Native pipeline state is explicitly labeled the pipeline creation recipe; it is not a driver query. The source and native matrix transport already matched in the retained initial run; this diagnostic tests raster-state hypotheses without changing rendering. Diagnostic admission is cached once at World creation.
