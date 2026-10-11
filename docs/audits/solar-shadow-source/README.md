# Original solar shadow raster oracle

Eight actual pinned original Viewport cases execute on native NVIDIA RTX 5090 OpenGL: solid caster and ground, opacity 0.69 and 0.70, texture alpha cutout, Hidden Line cutout, missing texture fallback, section cut, and a component caster entirely outside the camera. Each retains its original IGZ, frozen camera/style/sun, light and camera matrices, full on/off/restored frames, and the actual 2048 packed-depth image. Only the solar direction is controlled; source caster and receiver GPU passes are unmodified.

All eight maps are built exactly once across off/on, with exact restored frames. The original texture query reports NEAREST and CLAMP_TO_EDGE. Actual GL_SAMPLES observed inside paintGL is zero (draw framebuffer1), despite the requested QSurfaceFormat samples4; comparisons must use the actual target sampling. These are reference artifacts, not native Vulkan or Metal acceptance.

The initial run refused Mesa Zink over NVIDIA. Its failure is retained. Explicit NVIDIA GLX vendor/offload selection then produced native NVIDIA OpenGL. The owned private KWin/Xwayland seat was stopped after capture. The first native capture was repeated once to query sampling inside paintGL; retained final artifacts include that corrected observation. No native image was fitted or tolerance widened.

`captures.zip` contains the full corpus and transport provenance; `capture-manifest.json` verifies every entry. Root checked the five source files against their pinned original git blobs and all retained image hashes. Reproduce the two scripts under the existing disposable NVIDIA devbox transport with a fresh owned input-director seat.
