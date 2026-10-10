# Native Metal foundation

macOS uses native Metal; Linux and Windows use native Vulkan. This isolated foundation adds a direct Metal device, persistent command queue, shared/private buffer allocation, CPU writes to shared buffers, and allocation accounting shared with Vulkan. It does not implement rendering or compute submission. No wgpu or MoltenVK is introduced.

Full isolated workspace integration passes 434 tests with two ignored; retained log is `native-metal-foundation/workspace-integration-tests.log`.

Both Apple Silicon and Intel macOS all-target checks pass. Five shared allocation-ledger tests pass and the Linux GUI check passes. Logs are in `native-metal-foundation/`. These are compile and CPU test evidence, not Mac GPU execution evidence.

The APIs currently require macOS 11 or newer. Actual allocation size is reconciled after Metal allocation and before publishing the buffer. Advertised ray tracing and BC compression support are diagnostics only. Fence-retired ownership, rendering, compute, texture formats/compression, acceleration structures, application integration and real Mac hardware validation remain open. No parity row is promoted by this foundation.

References: https://docs.rs/objc2-metal/0.3.2/objc2_metal/ and https://developer.apple.com/documentation/metal/mtldevice .
