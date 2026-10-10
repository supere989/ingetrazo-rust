# First reviewed swarm milestone

Date: 2026-10-09, America/Los_Angeles. Production implementation base includes `7a84b59`; semantic checker provenance records `77d921e` where generated after provenance integration. Pinned original: `6be29fe437a7cd992d4e079f71720821b25216b9`. Evidence is bounded to these protocols; later commits are not inferred to pass.

- **145 Rust tests pass**, CLI builds; [test log](swarm-milestone-01-tests.txt), [build log](swarm-milestone-01-build.txt).
- Four original engineering documents preserve every recursively compared field and ZIP asset and reopen through the original Python runtime.
- Edited/fresh fixtures verify actual original runtime interpretation of geometry, transforms, persistent identity, saved views, section planes, datum, survey points/paths and sheet position/scale.
- Nine boolean shape cases match original native manifold3d within 1e-6 symmetric-difference volume tolerance (largest observed approximately 1.34e-8).
- Four original Push/Pull tool cases match exactly by the retained native shape oracle: cap-out 1.5 m³, cap-in 0.5 m³, blind pocket 0.875 m³, through-cut 0.75 m³. The original reference suite has 42 passing cases; Rust does not claim to have ported that entire corpus.
- Independent checks include holed extrusion, nonfinite/invalid atomic rejection, unrelated solids/construction wires, source attributes, cutter pick order and bounded tool undo/cancel.
- `.gltf` is textual JSON with embedded buffer; `.glb` remains binary. Decoded nested/reflected geometry checks pass.
- Textured startup panic fixed. [Screenshot](render-pergola.png) was visually inspected: model, materials, annotations, scene list and UI visible. Test runs in separate Xvfb software GL, never the user's desktop. [Reproduction script](../../scripts/render-smoke.sh).

See [canonical tracker](../PARITY_PROGRESS.md) for remaining scope. No full feature row or release gate is completed by this milestone. Shared component edit semantics, FollowMe/Fillet, remaining imports/exporters, rendering correctness, recovery, integrations, cross-platform packages and broader workflow/performance acceptance remain open. Opaque source state is preserved; unrepresentable edits are rejected before output truncation.

Baseline failures are retained. Initial runtime name/UID differences were generated default nondeterminism; the checker freezes only generated values per load and verifies persisted-ID mutations remain detectable. No document identifiers are globally stripped.

CI configuration accompanies this milestone; cloud validation is checked after publication and reported independently from local success.
