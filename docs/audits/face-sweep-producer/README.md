Shared source-compatible FaceSweep packet

`ingetrazo_render::face_sweep::{Input,Key,Kind,Packet}` borrows Scene and validates acquired face history identity + Scene.version before constructing bounded affected geometry. `Input::key(publication,eye)` includes exact immutable descriptor and eye-dependent suppression membership. Backend caches must additionally retain authoritative publication identity. No camera hashing or Scene clone.

Overlay packets contain the moved cap (holes retained), painted walls with `uvw` removed/`planar=true`, and Qt-float moved endpoints. Lines contain moved rings and non-softened risers; no repeated base ring. Prism packets contain only affected incident faces and moved edges; interior/paint/layer flags survive, and Kind::Prism requires ordinary world style/back/material rendering, not overlay two-sided paint. Suppressed face/edge IDs are sorted. Overlay preview collection intentionally ignores ordinary style/layer/back/opacity policies, matching `_draw_preview_faces`; prism collection uses the ordinary borrowed-mesh policy.

Logical CPU admission is 64MiB with checked repeated paint/texture and prism incidence accounting plus conservative connectivity allowance. This does not claim a hard host allocator overhead bound. Native GPU allocations remain separately ledgered.

Three focused tests pass, including immutable model, malformed identity/revision/nonfinite, repeated 9MiB paint refusal, zero/copy/camera membership and prism interior flags. Named protocol compares all four retained original lightweight polygon recipes exactly. Synthetic eyes reproduce recorded suppression only to isolate polygon construction; this is neither fresh original camera nor GPU pixel proof. Original recipe bytes are referenced, not recopied.

Run `face_sweep_protocol SOURCE_JSON OUTPUT_JSON`; Vulkan/Metal consumer and actual app/render/cache/refusal proof remain required.
