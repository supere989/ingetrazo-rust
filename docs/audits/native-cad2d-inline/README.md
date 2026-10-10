# Precise authored CAD2D inline Vulkan (bounded)

Source `49d3075`/`3cef03f` preserves authored straight encoded float RGBA and separate coverage. Immutable shared geometry has one cache identity; batch colors and pose are uniforms. The Vulkan callback records directly in the existing UI renderpass, without a target image, extra pass, CPU rasterization or warm geometry copy/upload. Scratch collections reuse capacity. Generic egui Color32 remains a distinct lossy interface.

Fresh actual Xvfb/llvmpipe application startup, two native resizes and OS close cover five mandatory controls. Precise interior source QColor(37,93,157,128) over white matches the independently retained actual QPainter [146,174,206,255]. The pre-existing Color32 path [145,173,206] is retained as a rejected negative control. The later checker texture and black UI overlay verify draw order and sampler/descriptor/tag restoration. Pose+color changes retain one 72-byte geometry upload. Callback-only frames avoid zero-sized UI meshes. An actual injected callback failure aborts the unsubmitted command buffer, presents a reported clear-only recovery frame, then renders normally; final captures do not claim a successful failed frame. Real pressure after successful vertex allocation denies the index allocation, restores the cap, verifies prior drawable identity/ledger rollback and retries. Pressure proof inspects actual resources; it does not independently pixel-capture the failed preparation instant.

Run with the project's PySide6 original interpreter:

```
python scripts/check-native-cad2d-inline.py docs/audits/native-cad2d-inline --output result.json
```

Production fixture `native_vulkan_cad2d_fixture` uses `INGETRAZO_CAD2D_CALLBACK_ONLY`, `INGETRAZO_CAD2D_CALLBACK_ERROR`, `INGETRAZO_CAD2D_PRESSURE`, or `INGETRAZO_CAD2D_UNIFORM_UPDATE` to enable each acceptance control. Normal startup remains unchanged.

Actual RTX hardware, full shape geometry/AA, managed generic Color32 quantization residual, and native Metal implementation/hardware acceptance remain open. The platform-neutral immutable geometry/straight encoded color/coverage contract is ready for a direct Metal consumer, but no Metal backend is implemented by these commits.
