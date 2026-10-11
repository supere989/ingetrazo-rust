# Intel cold World diagnostic prereview

Prerequisite a04e7adf adds observation only. This followup keeps native candidates on production defaults exactly as the failing 9302 branch, with the override confined to seeds and compute controls. Apple4 and non-x86_64 devices refuse before preparing any World. Root review removed the unified-memory exclusion because memory topology does not identify CPU architecture; the published probe must admit x86_64 integrated/shared-memory devices. Only the retained closed recipe is constructed.

Each independently seeded cell retains one seed and first plus three warm 800×600 RGBA8 frames. Every frame must be exactly 1,920,000 bytes before comparisons. Contours must be enabled. Same NativeMetal throughout; the seed submission completes and its owners drop before the candidate cold World is created. The reused-native cell preserves the failing acquisition sequence. No pixel result aborts later observations.

Readback storage is explicitly Shared and reads wait for Completed. Apple documents completion before cross-processor access for Shared storage; synchronizeResource applies to Managed storage copies:
https://developer.apple.com/documentation/metal/mtlstoragemode/shared
https://developer.apple.com/documentation/metal/mtlblitcommandencoder/synchronize(resource:)
No missing synchronization defect is established; no synchronization or production behavior is changed.

One focused x86_64 Apple Rust metadata check passed in 22.26 seconds using the exclusive existing target, jobs2, debug0 and incremental0:

```
cargo check --locked -p ingetrazo-gui --target x86_64-apple-darwin --config 'target.x86_64-apple-darwin.z.rustc-link-lib=["z"]' --config 'target.x86_64-apple-darwin.freetype.rustc-link-lib=["freetype"]' --config 'target.x86_64-apple-darwin.harfbuzz.rustc-link-lib=["harfbuzz"]' --example native_metal_world_cold -j2
```

These build-script link declarations permit metadata checking without local Apple C SDK; this is not linking/MSL/GPU acceptance. No GPU run or publication performed. Dedicated Intel-only push workflow is staged for root review. Exact branch exclusions prevent broad parity/compute triggers. Native SDK build and runtime artifacts will authenticate source and binary SHA256; the outer runtime timeout is90 seconds. Missing reports fail inventory, differing/blank pixels stay visible without a parity claim.

Root prereview also replaced Debug-string assertions with comparisons against actual Metal enum constants: objc2-metal0.3.2 derives newtype Debug (`MTLCommandBufferStatus(4)` and `MTLStorageMode(0)`), not the assumed names. The original numeric/debug observations remain retained beside explicit completion/shared booleans. No GPU run was spent discovering this checker mismatch.
