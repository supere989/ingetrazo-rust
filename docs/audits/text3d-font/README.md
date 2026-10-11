Native Text3D font geometry — focused acceptance

Runtime: 5b6ef03c → 02e941c9. Parent Params/group/fingerprint module is root-owned; the copied scaffold was not committed in this lane and is recorded solely for execution provenance.

10 actual original cases compare exact ordered f32 coordinates, faces/holes, edges/soft flags and fingerprints. Qt100pt resolves133px at96dpi. Native uses the same unscaled FreeType load, integer outline scaling, quadratic-to-cubic operation order and Qt6.12 bounded0.5 flattening. No production Qt or subprocess. Original source is pinned6be29fe4. Source acquisition used Qt offscreen, not a rendered viewport claim.

Replay: `python3 scripts/check-text3d-font.py [optional-fresh-native.json]`. Named consumer: `cargo build -p ingetrazo-formats --example text3d_font_protocol -j2`, then `text3d_font_protocol < original.json`. Extract original.json from the authenticated ZIP into owned temporary storage. Parent module must be integrated before building.

Two focused tests pass: bounded curve/flat geometry plus aggregate/prefix/coordinate admission. Prefix shaping is intentionally source-compatible; total prefix bytes capped8MiB before any font preparation. Aggregate block+letter flattened points capped262144; each glyph contours capped1024, raw points262144; font bytes use existing16MiB provider cap. These are logical cold-work bounds, not a universal host allocator peak claim. A compiled1.0-threshold alternative fails8cases; 30 coordinate/soft/empty mutations reject.

Scope remains open: platform-native CoreText outline differences, Windows font provider, variable/synthetic styles, mixed-family fallback, multiline/contextual/complex-script breadth, actual application3DText rendering/UI/history. Missing styles/fallback/variable fonts refuse before publishing. Existing paper/annotation raster/shaping APIs unchanged. No generic font or cross-platform parity claim.

Parent independently authenticated the archives and authorized cleanup. Archive-covered final/prior binaries, terminal captures and source copies were released; cleanup.json records exact paths/bytes/inventory digests. Reusable target/worktrees and shared615 CDI bundle remain. No broad suite or GPU acquisition was run.
