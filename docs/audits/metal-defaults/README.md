# Default native Metal sky and profiles

The Scene adapter now admits the default sky/profile flags. Sky is generated as two gradient quads plus an interior horizon line in the existing World encoder, before surfaces, with one 96-byte constant packet. It adds no sky image/vertex buffer upload or render pass. The original default RGB constants precede f64 gradient interpolation; custom colors retain the existing shared f32 style precision.

Profiles retain compact endpoint/anchor f32 arrays and f64 plane normals once per declared prototype, with one world/inverse matrix pair per placement. Camera selection transforms the eye once per placement; identical accepted camera/publication reuses native endpoints without rebuild/upload. Cold profile payload and selected endpoints each have checked 64MiB admission; GPU buffer ownership uses the existing shared ledger and Prepared lifetime. Candidate owners/cache counters publish only after a new World Prepared is accepted. Ordinary hard edges precede profiles, then ghosts/preview overlays. No throughput or full-device claim is made.

Loose soft-edge planes use the original first-three-vertex cross; group prototypes use source Newell normal/centroid and reject non-manifold radial counts. Source AST controls retain ten actual Qt camera horizon/gradient cases, five folded loose-plane eye selections, and the collinear-prefix group Face.normal/centroid case. Unit controls also check shared prototype ownership, translated endpoint order, suppression/hidden edges, invalid eye, and vertical sky fallback. All 43 focused World tests and both Apple Metal all-target metadata checks pass. Logs, including the unsupported local GUI Apple C toolchain refusal, are compressed and authenticated in manifest.json. Shader compilation, real pixel behavior, default launcher acceptance and GPU cache/ledger execution remain for root Apple CI.

Run source controls (pinned original unchanged):

```
QT_QPA_PLATFORM=offscreen /tmp/ingetrazo-parity-python/bin/python scripts/reference-metal-defaults.py OUTPUT.json
```

Focused native execution on macOS:

```
cargo run --locked -p ingetrazo-metal --example native_world_defaults -- OUTPUT
```

This native example uses the actual World Scene adapter/default style/four-sample target. It covers empty startup, repeated frame identity and zero extra profile/base uploads, camera-only profile refresh, profile toggle/soft-edge edit, unsupported-grid refusal preserving the accepted image, and final zero ledger. It retains five 64x64 RGBA images and a report. Its scene matrix is a controlled diagnostic camera, so it does not replace actual launcher/Qt whole-frame acceptance. The default launcher must separately submit a native World frame; changed refusal/fallback reasons are logged once, without per-frame log spam.

Section, shadows, geographic/terrain, image/annotation, soft/back-edge, grid and unsupported style/selection routes retain their explicit gates. This batch does not claim those features or general transformed-profile pixel parity.
