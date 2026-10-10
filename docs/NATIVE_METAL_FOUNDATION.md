# Native Metal foundation

macOS uses native Metal; Linux and Windows use native Vulkan. This isolated foundation adds a direct Metal device, persistent command queue, shared/private buffer allocation, CPU writes to shared buffers, and allocation accounting shared with Vulkan. It does not implement rendering or compute submission. No wgpu or MoltenVK is introduced.

Full isolated workspace integration passes 434 tests with two ignored; retained log is `native-metal-foundation/workspace-integration-tests.log`.

Both Apple Silicon and Intel macOS all-target checks pass. Five shared allocation-ledger tests pass and the Linux GUI check passes. Logs are in `native-metal-foundation/`. These are compile and CPU test evidence, not Mac GPU execution evidence.

The APIs currently require macOS 11 or newer. Actual allocation size is reconciled after Metal allocation and before publishing the buffer. Advertised ray tracing and BC compression support are diagnostics only. Fence-retired ownership, rendering, compute, texture formats/compression, acceleration structures, application integration and real Mac hardware validation remain open. No parity row is promoted by this foundation.

References: https://docs.rs/objc2-metal/0.3.2/objc2_metal/ and https://developer.apple.com/documentation/metal/mtldevice .

CI independently checks the native Metal library and device probe for both macOS architectures on Linux runners and retains each check log. These jobs perform cross-compilation only; successful results do not prove framework linking, application presentation or actual Mac GPU execution.

Separate native macOS CI jobs build and link the device-probe executable on Apple Silicon (`macos-15`) and Intel (`macos-15-intel`), retaining build, Mach-O architecture and framework linkage diagnostics. Runner labels follow https://docs.github.com/en/actions/reference/runners/github-hosted-runners . These jobs establish native binary linking when successful; they do not execute the probe or establish GPU rendering/hardware parity.

Native macOS CI now executes the linked device probe on both runner architectures and retains its exit result and device-and-allocation.log. The probe requires an actual Metal device, performs a budgeted shared-buffer allocation/CPU write/release, and fails normally if unavailable; it does not submit GPU commands. Runner execution remains pending until the corresponding logs are inspected. Successful compile/link jobs alone remain insufficient evidence for device allocation, rendering or release readiness.
