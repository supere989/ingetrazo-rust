# Native font and ordered Canvas merge audit

Read-only comparison of next-integration `17cfa943` and native Metal `729480f6`. The generated JSON contains every source-bearing commit in original dependency order, acceptance/evidence inventory and patch-equivalent flags. No build, cloud launch or root worktree edit was performed. A `git merge-tree --write-tree` simulation reported the nine listed conflicts. It writes Git objects only, without checkout/index changes.

Prefer integrating reviewed history and resolving the narrow unions rather than replacing files with old native-branch copies. Shared exact palette and some ordinary warm consumer changes already exist on the annotation branch; preserve the newer annotation owner semantics.

Production stages, in order:

1. Metal raw encoded bytes: `5cd7e4fb` then `7b0b196d`, with native cancellation accounting `e29a1c20`. Existing Vulkan retirement improvements must survive.
2. Native matching/resource identity: `df37af7a`, `64c4ccb5`, `68cee8da`, `ed56063a`, `64263f68`, `2f69df23`, then full native backend `76f391aa`; collection/name admission `38a7b27b`, `61dcff6b`; virtual physical resource witness `f5eea66e`.
3. Effective glyph metrics/synthesis: `a1911c0f` plus required narrowing `1cbcabd4`; subset bearings `96505419`; shaping context `7a944bd5`; native bold/ARGB `46068820`; negative-fixed positioning `ab536e52`; bounded phases `2261f288`.
4. Scoped HarfBuzz tracking: `9a3b6ec4` (includes vendored dependency/license/provenance and Cargo patch), then exact phase `38c249de`, opaque native mask `26d1de22`. Keep unmarked FreeType and annotation behavior unchanged.
5. Ordered producer: observation `3baeda0e`, shared palette `55ea2050` (already patch-equivalent), consumer `7612ff86` (newer annotation branch already supplies API), producer `785df793`, warm/cache identities `1bfb39f2` (already equivalent), `6c2843ad`, `8d0368e9`, ordinary renderer `c49ec6d0` (newer counterpart already present).
6. Actual nine-case GPU/18-PDF gates: provider/PDF/tile fixtures and scripts in full JSON, culminating `de4a544d`. Do not omit source physical/effective traits, tracking context or warm deep-admission guards. Retain ninth ARM+Intel ZIP proofs and historical failures.
7. Presenter observation `8898a61c`, `7cd4543e`; source-reviewed lifetime candidate `729480f6` stays a candidate awaiting tenth native execution, not established ARM causality.

Platform boundaries:

- Actual native SDK builds and nine whole GPU images/eighteen PDFs are exact on ARM and Intel in ninth run38093876455. Intel positive presentation passed; ARM completed GPU commands without positive callback and failed strict deadline. Tenth run38094814956 is still active at audit time.
- CoreText matching/glyph resource path is macOS-only, with physical TTC index distinct from extracted SFNT index0; instance identity and effective synthesis policy must remain in font/cache keys. Qt source pixel policy is deliberately scoped native paper behavior. Windows provider/full platform font parity remains open.
- Ordered producer is shared Canvas code; Metal exact consumer has actual image proof, while ordinary Vulkan consumer focused admission proof is separate from complete GPU font acceptance. Glow/non-raw compatibility fallback must remain usable, without claiming exact native font parity.
- No full physical-Mac or full P28/P29 promotion. Broader fonts, layout, heterogeneous item interactions and ARM WSI remain scoped open. Native GPU test is offscreen readback; presenter lifetime probe is a distinct gate.
- No process renderer, perframe font provider startup or warm geometry/palette re-admission is introduced. Keep current ownership budgets/exclusions and backend identity keys.
