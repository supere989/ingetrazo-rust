# Native Metal presentation foundation — opt-in, unexecuted

The `ingetrazo-metal` presentation module attaches a native CAMetalLayer to an
AppKit NSView from a winit Window. A single persistent native device and queue
submit directly to the drawable in one Metal render pass. No wgpu, MoltenVK,
process dispatch, CPU image upload, extra image, or blocking GPU completion wait
is involved. `presentation_probe` currently clears the drawable only: this is
not an egui application renderer, world renderer, or parity claim.

## Ownership and bounds

Three retained in-flight owners maximum. Polling requires actual command
Completed **and** positive drawable presentedTime before retirement. Errors
poison admission; Drop never waits and retains the entire device/queue/layer/
command/drawable owner when completion is unresolved. Extent <=16384, finite
positive density, valid normalized clearRGBA. Minimized zero extent skips.
Drawable extent/backing density are updated on resize and scale events.
`allowsNextDrawableTimeout=true` permits up to one second acquisition blocking;
this documented foundation limitation must be measured and replaced by paced
acquisition for the final fast host. Core Animation allocates its drawable pool;
that system-owned memory has not been reconciled with the application ledger.

## Replay / acceptance still required

`cargo run --locked -p ingetrazo-metal --example presentation_probe` on actual
macOS: identify native device, capture only the owned window, check clear color,
two resize/density cycles, completion/presentation counters, clean close,
minimize/restore and denied drawable/capacity controls. ARM and Intel cross-check
are compile evidence only. No actual CAMetalLayer execution has happened in this
isolated checkpoint; AppleParavirtual offscreen CI proof cannot substitute for
physical Mac WSI.

## Application integration plan

1. Shared `IngeTrazoApp::update_ui`, egui-winit input/clipboard/IME/platform output,
   public document close guards and session store; same ComposerCompute pool.
2. Direct native Metal egui mesh+font/managed-texture cache with actual source
   encoded-alpha contract. Apply partial updates transactionally, sampler options,
   clip/scissor, stable IDs and frame-owned resource retirements; unsupported
   callbacks refuse explicitly until native implementations exist.
3. Integrate exact CAD framebuffer fetch on admitted Apple families and source-
   ordered compute on Intel/AMD as actual paint paths, not an offscreen wrapper.
   Current Metal CAD compute is bounded constant pixel spans only.
4. Shared cached world/material/texture/depth/stencil/edges, admission pressure,
   immutable scene revision ownership and optional RT/compression. These remain
   full renderer scope; this clear probe does not complete any of them.
5. Production repaint pacing and asynchronous completion ownership, actual ARM
   and Intel physical Mac measurements/captures, failure controls, and public
   opt-in launcher only after genuine full application output passes.

Sources reviewed: Apple CAMetalLayer nextDrawable / allowsNextDrawableTimeout
(up to one second), and CAMetalLayer keeping drawable references / MTLDrawable
presentedTime. No inferred command-only presentation completion.
