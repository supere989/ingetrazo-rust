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

## Budget/resource stage

Backend-neutral `gpu_memory::MemoryBudget` reserves exact allocation bytes, commits only after API allocation/binding, and retains retired allocations until actual free after the relevant fence. `texture_mip_bytes` accounts RGBA8 and complete BC7/BC5 blocks, layers, depth, odd dimensions and mip chains with overflow checks. This arithmetic is not a compressed texture implementation.

`native_vulkan::budget::VulkanBudget` queries live VK_EXT_memory_budget properties when advertised, reports application accounting and imposes heap admission. Defaults use 80% of heap capacity and 90% of live driver budget; `INGETRAZO_GPU_HEAP_LIMIT_BYTES` explicitly overrides each application's heap limit. Driver usage is an estimate; already-accounted application allocations are not counted twice.

Pinned vendored egui-ash-renderer 0.7.0 exposes a default allocator observer so UI texture images, staging allocations and mesh buffers use actual Vk memory requirements and heap mapping. Admission/bind failures roll back, and safe actual frees release charges. Pending rejected texture deltas retry without destroying the document; successful deltas are removed individually. The remaining synchronous full-image uploader is still a chunking/performance gate.

Pure queue selection preserves original indices, handles split graphics/present queues without requiring compute on graphics, and identifies a dedicated compute candidate. Actual suitable adapters rank discrete/integrated/virtual/other/software, with `INGETRAZO_VULKAN_DEVICE` index or unambiguous name override. Dedicated compute submission is not implemented merely by identifying it.

Actual `native_vulkan_budget_probe` admits a 256-byte image and forces staging admission denial, verifying zero committed/reserved leakage before continuing production UI; startup, two native resizes and clean close pass afterward. A 600000-byte admission limit causes an actual allocation rejection while the host stays alive and clean OS close remains functional. Full GUI library: 80 passed, one ignored. Independent nearest-sampler baseline is retained; the fork now routes TextureOptions to sampler state and partial updates preserve prior texels, independent corrected nearest/partial-update screenshots pass 16 samples and two negative controls (validator commits 5704164 and 48e1864).


## Direct opaque world stage

`INGETRAZO_VULKAN_WORLD=1` enables a bounded production viewport path: cached world-space opaque vertex/index buffers, Vulkan depth testing, and an offscreen sampled image submitted before the egui UI pass in the same command buffer. Camera motion changes the matrix without re-uploading geometry. Scene version, view version, monotonic application model revision and style invalidate the cache; the shared model revision must retain the composer ink-cache invalidation body when integrated. The ordinary launcher remains Glow and the opt-in path is not full viewport parity.

`native_vulkan::resources::Resources` admits actual buffer requirements, directly maps suitable unified device-local memory, otherwise transfers in at most 8 MiB staging chunks. Retired buffers remain charged until an actual signaled fence and matching completed draw serial permit destruction. The native resource probe uploads and verifies all 17 MiB, uses three chunks with 8 MiB peak staging, rejects collection behind an unsignaled Vulkan fence, then frees after actual queue submission and fence completion. This evidence uses llvmpipe unified memory; discrete GPU readback/performance is unproved.

The crossing-face fixture uses the production app and viewport: GPU fragment depth correctly leaves red on the left and blue on the right, whereas the retained centroid painter control paints both blue. `scripts/check-native-vulkan-depth.py` checks the positive image, retained failure and swapped-side negative control. Startup, two native resizes and clean OS close also pass; the unchanged geometry reports one upload across resizes. GUI library tests: 81 passed, one ignored; render library: 18 passed.

Depth/stencil storage is allocated, but stencil section masks are not yet implemented in this path. Textures, transparent geometry, edges, grids, annotations, native section fill, prototype GPU instancing, worker-assisted tessellation and hardware throughput/VRAM profiling remain open. The surface builder avoids CPU projection/HLR but still triangulates synchronously on cache misses. UI image staging remains synchronous and full-image. GLSL and checked-in SPIR-V are reproducible with `scripts/compile-native-world-shaders.py` using shaderc 0.8.3; shaderc is a development tool, not a runtime dependency. Native Metal remains a separate implementation and hardware acceptance requirement.


## Bounded UI texture transfer

The vendored uploader now reuses a staging allocation capped at 8 MiB, borrows contiguous source row slices, and waits for a dedicated transfer fence rather than queue_wait_idle. An actual 4096×3072 RGBA image (48 MiB) uploads with only 8 MiB staging headroom, then accepts a partial lower-right update. Native screenshot samples are exactly red/green/blue/yellow across chunk boundaries and the partial region. The prior full-image uploader is compiled as a retained negative control and fails the same budget. Actual allocation rollback and native startup/two resizes/clean close are rerun. This is synchronous chunked transfer, not asynchronous mip streaming or compressed texture support; worker-assisted decode and upload responsiveness remain gates.
