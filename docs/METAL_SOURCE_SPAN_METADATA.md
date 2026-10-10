# Original coverage span metadata

The precise composer producer retains its original signed integer Qt coverage rows and byte coverage before conversion into logical f32 vertices. Admission compares every canonical quad position and coverage exactly against that producer mapping. Vulkan vertices, indices, batches and shader behavior remain unchanged.

The immutable metadata records zoom, pixels per point and the original f64 physical origin/phase. Metal translation admission requires the same exact zoom and density and an exactly integral physical origin minus the recorded phase. No tolerance, integer recovery from f32 vertices, or phase rounding is used. Full origins used by curved producers remain supported metadata. The cache distinguishes zoom and pixels per point even when their product is equal.

Metadata is bounded to 100000 rows and the existing million-pixel source domain. Its CPU storage is counted separately by SourceSpans::logical_bytes; this is not GPU allocation accounting. The native compute plan retains original batch order and exact source rows, with the existing bounded sparse tile/reference admission.

Linux render admission and actual composer producer tests cover altered metadata, a one-bit position mutation, phase changes, density changes, immutable warm identity and invalid-update preservation. Mac cross-compilation is engineering validation only. This stage does not activate an Intel presentation path or prove GPU pixels, ownership, resize or performance. Arbitrary non-span geometry remains required scope.
