# Native GPU host checkpoint

The `native_vulkan` example runs the production `IngeTrazoApp::update_ui` directly in winit 0.30 and submits egui meshes with ash 0.38 / egui-ash-renderer 0.7.0. There is no wgpu dependency in this build. The normal launcher remains the transitional Glow host pending acceptance. Linux/Windows Vulkan is the target; macOS requires a separate native Metal implementation, not the upstream portability/MoltenVK route.

## Verified boundary

The shared update function preserves document/session logic and existing texture handles. Native root viewport close events enter the application Save/Discard/Cancel guard; returned Close/CancelClose commands determine exit. egui-winit supplies keyboard, pointer, IME and platform clipboard handling. Redraw scheduling follows egui repaint delay and input wakeups, with an event-loop proxy for immediate background repaint requests. A single in-flight fence retires texture frees safely. Zero extent postpones swapchain recreation.

`cargo test -p ingetrazo-gui --lib`: 73 passed, 1 ignored. `cargo tree -i wgpu`: no packages. Actual disposable Xvfb startup used Mesa llvmpipe Vulkan 1.4.318, with production menus, viewport and tray visible in retained capture. Extension diagnostics describe advertisement, not enabled hardware RT. Use `scripts/check-native-vulkan-startup.py target/debug/examples/native_vulkan OUTPUT` for native resize captures and an actual WM_DELETE_WINDOW clean-close control. Do not equate screenshot existence with pixel parity.

## Remaining production gates

The checkpoint still consumes the existing CPU projected egui scene painter. Full viewport acceptance requires cached world vertex/index/texture uploads and direct depth/stencil GPU rendering; the egui pass becomes the UI overlay. Existing historical wgpu screenshots must be rerun on the actual native backend.

Resource budget initialization can use exposed VulkanContext instance, physical_device, device, graphics_queue and command_pool. The default upstream allocator and synchronous texture uploader need replacement or preflight enforcement: bounded staging chunks, explicit heap budgets and committed accounting, texture deduplication, BC7/BC5 support checks, mip streaming and retained scene state when allocation fails. Presentation semaphores are allocated per swapchain image and recreated after idle; error propagation, device loss, screenshot readback, clipboard actions and native close tests remain open.

Shared backend APIs should expose adapter diagnostics, immutable resource revision/cache handles, budget admission, upload completion, draw/compute submission, present and readback. Native Metal implements these independently. Ray tracing requires actual feature/property queries, enabled acceleration-structure/ray-pipeline features, BLAS/TLAS caches and hardware benchmarks; extension names alone are insufficient.

Quadratic CPU work and other eligible pipelines require persistent worker processes as explicitly directed. Root owns scheduler/budget modules: immutable binary geometry/task buffers and shared outputs should cross that boundary without per-frame JSON/full Scene copies or worker startup. Worker counts and task grain require throughput/latency/memory measurements. GPU submission stays direct; optional integrations remain in parity scope when optimized.

This is a reviewable host proof, not full renderer or platform parity.
