# Opt-in direct Metal egui application host — native execution pending

Source adds a direct MSL mesh painter, persistent CAMetalLayer/device/queue, and
shared IngeTrazoApp::update_ui host via egui-winit. Launch macOS CLI with
INGETRAZO_NATIVE_METAL=1. Default launcher remains unchanged. No wgpu, MoltenVK,
child rendering process, image composition intermediate, or GPU completion wait
is introduced. The existing shared ComposerCompute controller stays in the app.

Immutable texture replacements preserve old submitted owners; partial changes
copy the retained CPU source and upload a new image once, never mutate an image
being sampled. Sampler min/mag/wrap supported, mipmap requests explicitly refuse.
256MiB current CPU texture cache cap, <=4096 texture sides, bounded million
vertices/three million indices. Actual Metal buffer/image allocation sizes are
charged before publication. Geometry hashes reuse identical VB/IB; frame owners
retain draw textures, sampler, pipeline and immutable mesh until actual render and
presentation completion. This cold texture path uses shared/managed Metal images,
not the optimized resource uploader; no shortest-path performance claim.

Shared input/IME/clipboard and root viewport commands use egui-winit. Native close
requests go through the existing application Save/Discard/Cancel guards.
Painting failures retain application/document state and last presented output,
with a native window title/status diagnostic. Presentation errors quarantine GPU
owners. Recovery/error-modal visibility is not yet acceptance-proven.

The generic Color32 path uses the same encoded-alpha interpretation as the
Vulkan generic painter, including unchanged font coverage. Lost Color32 precision
cannot be recovered; this is NOT the exact authored CAD path. Existing composer
state had unconditional Vulkan types on Mac; platform gating now permits the
same factory-absent generic mesh behavior. Metal precise CAD callbacks, native
GPU world/depth/stencil/material/texture rendering, ray tracing/compression, full
multiple viewports and all lifecycle/visual parity remain OPEN. Unsupported paint
callbacks and missing user textures refuse, no other graphics backend is invoked.

## Evidence boundary

ARM and Intel complete CLI cross-check PASS and Metal tests cross-check PASS.
MSL is compiled at runtime; Linux cross-check cannot establish MSL validity,
texture sampling, actual window painting or physical hardware acceptance.

`cargo run --locked -p ingetrazo-metal --example egui_pipeline_probe` on Mac
replays runtime MSL/pipeline creation, warm upload counters, actual second-buffer
admission rollback preserving old cache/ledger, partial update metadata and
missing texture/callback refusals/final ledger0. It does NOT submit/sample a GPU
paint. The actual app still requires owned-window captures on ARM and Intel,
fonts, nearest/linear/repeat, sparse partial pixel controls, resize/DPI, dirty
close and pressure/unknown completion cases. AppleParavirtual is explicitly a
virtual-device boundary; physical Mac is required independently.

Remaining efficiency gates: CPU mesh packing/hash each repaint, immutable changed
texture full replacement, system drawable accounting, up-to-one-second documented
nextDrawable acquisition, final production pacing and presentation retirement.
