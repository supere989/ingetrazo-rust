# Native Metal hidden ghost candidate

Implemented direct compact24-byte position/RGB buffers, immutable revision/pose/flag cache, three source-ordered ghost draws after ordinary surfaces in the same Metal render encoder. Ghosts use LessEqual/depth-write, face bias1, opaque shaded/default-gray color, source4-pixel face stipple and6-pixel diagonal edge stipple (bottom-up GL fragment coordinates), no cull/section clip, and never enter ordinary prototypes or shadow casters. Prepared frames retain the ghost buffers through existing submitted-owner lifetime; changed preparation remains private until the whole frame succeeds. Warm camera-only frames reuse the packet/buffers.

The production native_metal world_viewport consumes the same Qt-f32/OpenGL camera matrix as Vulkan. Native Metal maps clip depth in the vertex shader. It requests4sample support explicitly: matching pipelines and native private color/depth multisample textures are budgeted before allocation; the existing single-sample UI image receives a native end-of-pass resolve. No CPU image/pass/copy is introduced. Old Prepared frames retain their own sample-compatible attachments and pipelines. Low-level World callers keep1sample by default.

Native loose wire support is deliberately source-gated by show_edges, visible nonsoft/layer-visible loose edges, width1 and no profiles/back-edges/soft-edge overlay; world positions use the same24-byte ABI. Cached ordinary lines are a separate solid mode, drawn before ghosts and excluded from the surface catalog/casters. Groups' ordinary wire/selection/profile/section/shadow/general style stages remain required unsupported stages, not silently dropped.

## Focused retained evidence

Two host tests pass: compact/layout/revision admission and hidden-only loose edge exclusion from ordinary prototypes. Both installed Apple Metal targets pass `cargo check -p ingetrazo-metal --target TARGET --all-targets -j2` with debug0/incremental0. These are compile/host checks, not Metal GPU execution. Initial unsafe descriptor setter/test-index compiler failures were corrected; the named GUI Darwin crosscheck was blocked in libz-sys before GUI compilation because this Linux container lacks a Darwin C compiler/SDK. Its failure log is retained.

## Actual Apple acceptance still pending

`.github/workflows/metal-hidden-ghost.yml` is manual only; no push was made. It materializes the existing pinned original12 from `docs/audits/hidden-ghost-workflow/captures.zip` (manifest hashes checked), builds only `native_metal_hidden_ghost`, freezes source/binary/device metadata, executes the actual native app world_viewport route with5-second GPU readbacks and120-second outer deadline, and compares all12 whole RGBA images strictly. It additionally requires warm geometry/ghost uploads0, real shared-budget refusal retaining old pixels/charges and retry, saved-view flag recall, Unhide returning no ghosts, actual visible loose-wire pixels, a separate valid embedded texture Unhide/ordinary sampled draw with warm reuse, and final charged/reserved bytes0. The deliberately nonexistent texture path remains an explicit post-Unhide refusal in the frozen12; it is not called successful ordinary rendering. The extra valid texture control does not change frozen inputs or imply a new source whole-frame proof.

Replay on actual macOS:

```
python3 scripts/check-metal-hidden-ghost.py docs/audits/hidden-ghost-workflow INPUT --unpack
cargo build --locked -p ingetrazo-gui --example native_metal_hidden_ghost -j2
target/debug/examples/native_metal_hidden_ghost INPUT FRESH_OUTPUT
python3 scripts/check-metal-hidden-ghost.py docs/audits/hidden-ghost-workflow FRESH_OUTPUT
```

No actual Apple shader compilation/GPU/pixel/lifecycle result exists for this candidate yet. Metal raster line/sample/resolve precision must pass the frozen NVIDIA comparison, not a tolerance change. Existing Metal material/classic hardware captures predate this viewport camera/MSAA change and are not fresh regression proof. Specific queued-unknown-completion ghost retention remains unexecuted; ownership is embedded in the existing Prepared/Submission quarantine mechanism. Full normal world features, Metal picking/selection/context ghost presentation, caster integration, full shadow paths and whole-platform acceptance remain open. Shared budget accounts native allocations; this is not a VM/host-total-memory bound.

Primary native resolve contract: [Apple MTLStoreAction.multisampleResolve](https://developer.apple.com/documentation/metal/mtlstoreaction/multisampleresolve).
