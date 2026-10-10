# Private annotation overlay foundation

Not activated in application dispatch. This stage preserves the existing CAD pass/pipeline defaults and adds a private annotation variant: ordered byte-blended spans discard from retained visibility flags before loading destination color. No depth readback, image upload, new color target or extra render pass is introduced.

Immutable plan identity, physical density and font DPI key the CPU span mesh; glyph masks additionally key immutable resolved face, pixel size, glyph, density and quarter-pixel phase. Warm unchanged plans reuse the same mesh/masks without shaping/rasterization. Source gray/cosmetic and curve kernels generate disjoint ordered runs; query inventory is validated before font work. Current limits bound payload cells/runs/geometry/glyph masks; they are not total allocator/native-font resident memory accounting.

Focused three producer controls pass: warm immutable reuse and density invalidation; glyph admission refusal preserves previous mesh/mask; malformed geometry and missing query inventory refuse without replacing the ready mesh. One private GPU gate-admission test passes for exact batch count, both-point/sentinel/out-of-range rejection. These are CPU admission controls, not pixel parity or actual shader execution.

Evaluator now exposes a borrowed RAII staged candidate. Staging leaves live descriptors, old buffers, identities and uploads counter unchanged; commit writes descriptors and retires preflight-proven old ownership without further fallible admission. Discard retires only candidate IDs; uncertain cleanup remains retained. Existing diagnostic prepare uses stage+commit, preserving its behavior. The next required step is coordinated CAD mesh/query publication: the new query descriptor must never become visible while a previous ready mesh still references a different query inventory. Current private Renderer prepare has not yet been converted into that combined transaction and is not activated.

Shader compiled using pinned shaderc 0.8.3 helper:
`ingetrazo-image-shader-compiler annotation_overlay.frag annotation_overlay.frag.spv`.
Compiler binary SHA256 61e575f778907c65950640430a81cd7218f3df68e1cfb6c3d3b2ea04615169fa.
SPIR-V SHA256 5a629eaf301ab11bd94150c019db41b75d91b20a85d7f3046d03cd11d26f68a6.
Fresh recompilation compares byte-identically. Actual GPU gated images, source font masks, representative MSAA depth-selection mapping, full native viewport dispatch and real-model strict images remain required and unproved by this stage.
