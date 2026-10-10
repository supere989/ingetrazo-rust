# Direct native texture draw: bounded software proof

Production `World::record` now samples cached optimal Vulkan images from direct
20-byte position/UV buffers in its existing render pass. Shader V inversion
matches the original QImage mirrored upload without copying/flipping pixels.
Source shade, rounded opacity, front/back culling, repeat, mip sampling,
alpha-cutout/Bayer behavior and depth-write phases are per-draw state.

Run from an initialized submodule checkout:

```
xvfb-run -a env QT_QPA_PLATFORM=xcb LIBGL_ALWAYS_SOFTWARE=1 /tmp/ingetrazo-parity-python/bin/python scripts/reference-native-texture-gl.py /tmp/texture-original
cargo build -p ingetrazo-gui --example native_vulkan_texture_draw_probe
target/debug/examples/native_vulkan_texture_draw_probe /tmp/texture-original /tmp/texture-native --software-fixture
python3 scripts/check-native-texture-draw.py /tmp/texture-original /tmp/texture-native
```

The retained original Qt/Mesa versus native Vulkan/llvmpipe corpus has five
literal cases and 25 exact RGBA samples. Missing-case and one-channel mutation
negative controls fail. Full image equality is not asserted.

The actual sampled-draw probe checks warm hash/decode/image/vertex uploads stay
unchanged, refuses a resource replacement before allocation on an incomplete
draw fence, then imposes real image-admission budget pressure after earlier
geometry preparation. Failed replacement preserves the prior committed ledger
and every pixel of the old GPU drawable. Retry succeeds and retires the old
sampled image after its actual draw fence completes; World teardown leaves zero
committed allocations. Engineering pressure changes admission policy, not Vulkan
allocation return values. Source original/native screenshots are retained.

This is a stage, not full texture/render parity. Independent NVIDIA source/RTX
GPU and owned-window draws, full section-hole/depth/order controls, textures with
colored back overrides, source texture-average Shaded presentation, XRay and
remaining style semantics are still open. Only PNG/JPEG cold decoding is active;
Qt additional formats, profiles/orientation/high bit depth, compressed formats,
streaming, and actual native Metal submission remain open. The existing transfer
fence failure cleanup uses unbounded device idle to protect in-flight ownership;
bounded device-loss handling remains an explicit performance/lifecycle gate.
