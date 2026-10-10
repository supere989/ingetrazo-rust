# Bounded optimal-image resource proof

Production `Resources::upload_rgba8_image` and `TextureImage::rgba8` are exercised by `native_vulkan_texture_upload_probe`. The strict default requests NVIDIA discrete hardware; this retained run explicitly uses `--software-fixture` (llvmpipe). It proves an actual 17 MiB optimal image upload/readback, constant 7×5 GPU mip chain, reuse of transfer command/fence and ≤8 MiB staging, actual image budget denial, actual staging denial after admitting the image using queried Vulkan requirements, old image preservation, successful retry, and zero live ledger after cleanup.

Hardware proof remains pending independent validation. This resource stage does not claim source texture UV/filter/opacity rendering, active BC compression, streaming, or Metal support. Source PNG/JPEG decode and GPU texture draw/cache integration follow separately. Image owners must retain resources until their last submission fence completes; no global idle is hidden inside image drop. Diagnostic TRANSFER_SRC usage is explicitly requested.

Build: `cargo build -p ingetrazo-gui --example native_vulkan_texture_upload_probe`. Run the example with no flag for strict hardware, or explicit `--software-fixture`.

`software.log` retains the initial reusable-allocation-only checkpoint. `software-mapped.log` exercises the persistent mapping correction: one real map across the large multi-chunk upload and subsequent mip image, zero intervening unmaps; explicit staging release before the budget fault and retry creates a second map, and final two unmaps balance two maps. Noncoherent memory flushes use the full valid mapped allocation (`VK_WHOLE_SIZE`, offset zero). No per-frame mapping is introduced.
