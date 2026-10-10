# MetalKit presenter lifecycle control

This isolated example distinguishes the custom layer-hosted presenter from the system MTKView hosting/timed-delegate lifecycle while retaining the same winit event loop, window, NativeMetal device/queue, clear color and actual callback proof. The system framework vends the drawable and render pass. The control is diagnostic only and is never called by production.

Build on native macOS with `cargo build --locked -j2 -p ingetrazo-metal --example metalkit_presenter_control`. Run the resulting example with one JSON output path. Two positive callback frames plus completed commands and zero pending owners within15s are required. Failure remains failure; unproved owners stay quarantined. Keep the full existing Canvas/PDF and production lifecycle gates alongside this new control.

Apple documents [MTKView timed updates and drawable ownership](https://developer.apple.com/documentation/metalkit/mtkview) and [delegate drawing](https://developer.apple.com/documentation/metalkit/mtkviewdelegate). This design varies native framework hosting/scheduling as a unit, so even a positive comparison would require further diagnosis before changing production.

Focused Apple-target metadata compilation passes. Actual native SDK link/runtime has not run. No cloud workflow was dispatched.
