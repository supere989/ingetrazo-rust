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

For requested-target attachment inspection, append `--requested-draw-only` to the original producer command. This executes the pinned `render_image` GPU `_export_size=(1024,768)` / `paintGL` sequence, deliberately omits `toImage` and all QPainter/PNG work, then restores the retained live FBO, FBO size, depth owner and framebuffer bindings. The declared frozen default request is `{}` at actual logical 800×600. This is attachment inspection only, not yet a per-sample coverage proof.

The consuming sample discriminator uses `--requested-draw-only --sample-counts` on the original producer and `--axis-sample-counts` on the existing native host consumer. Three declared frozen residual sites are (575,1), (749,441), (1,639) in output top-origin pixels. Both attachment pixel coordinates are (x,767-y), matching the retained bottom-origin scene-target transport. Each of three axes has four actual sample masks and depth-enabled/disabled precise queries: 36 rows / 72 queries. Source queries delegate the same program/VAO/VBO/uniforms; native queries reuse the same accepted axes buffer, shader, push constants and four-sample pass. Both disable all color/depth writes. Native requires the actual precise-query device feature and submits on the existing owned requested-target/fence path, then restores the live target. No resolve, QImage, composite or PNG is produced. These are counts and depth-passing decisions, never claimed raw stored color/depth or strict pixel acceptance.
