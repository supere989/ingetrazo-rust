# Render and workflow integration

Base published review `aaea1a31`; reviewed integrated runtime `f2dbb0cd`. Original Python remains pinned to `6be29fe437a7cd992d4e079f71720821b25216b9`; runtime changes are in Rust. Main remains `c73fe993`.

One combined workspace run passes **852 tests, zero failures, two existing ignores**, doctests and every example compilation in **30.44 seconds** including local-package invalidation. Dependencies are retained. The optional interpreter path was checked once because Scene storage changed: **15 tests pass in 49.08 seconds**, including compilation. Logs and source IDs are in acceptance.json. No broad suite is repeated for documentation. Preceding published review passes all seven CI jobs (both Metal compile/link platforms, both ordered Metal runtime jobs, Linux tests).

Reviewed intake:

- Rich GLB `90302c06` → integrated `85bfe6ad`: nine independently decoded source/native surfaces, materials and assets; eight triangle collections exact. One planar hole uses different diagonals with exact rational boundary/coplanarity/area proof; seven corruptions reject.
- Blender document/follow `eefe451d` → `6e8f6860`: five cleaner cases, eight real original/native states, original reopening of native IGZ, two fractional cameras/keys and six corruptions. Root actual Blender completes eleven renders in19.14s. Both retained source cube images match exactly; follow uses persistent workers, latest stopped camera, three draft files and cached full-document projection. Independent review corrected per-frame slider history, missing parked geometry, Cancel ownership and pending-control submission.
- BIM `47d7e952`, `51865be0`, `dc884108` → `89798028`, `23e3f4ac`, `04995239`: genuine pointer entry and CSV/IFC Save workflow, eleven phases. Root independently replayed IfcOpenShell EXPRESS, exact root rings/quantities and three parsed-file mutations. Native mixed metadata Undo remains an explicit difference from source no-History tagging.
- Cutout `cfb1f29b`, `bd9c9a86`, `9c7d7676`, `965ed6e6` → `c83b3ce3`, `ab4108af`, `efedcd41`, `f2dbb0cd`: all16 native whole frames/maps exact, including three heldouts; four actual GPU application controls pass. Root verified every archived digest and independently recomputed comparisons, checked three integrated source/SPIR-V digest pairs, and retained the deliberate legacy-discard negative. No extra render pass, full-scene copy or per-frame capability query was introduced.

Portable depth remains a strict failure: all24 frames and7/8 maps match; section packed depth differs at38,486 pixels, maximum channel error255. The current feature-off discard module reproduces286 shadow pixels. The independent replay returns exit1 for that retained portable map failure; it is not converted to a pass. Broader production scenes, actual new light UI, Windows/Mac consumer details, physical GPU/Metal acceptance, ray tracing, streaming and representative memory/performance requirements remain open.

Canonical stage evidence is linked from the tracker. Temporary Blender source/native captures were purged only after byte verification against their retained files/archives; duplicate-purge.json records every removed file. Build dependency caches are kept.

No full feature row is promoted: **10 Implemented / 27 Partial / 0 Verified**, all five gates OPEN. Counts are not effort percentages.
