# Annotation activation and solar controls integration

Integrated runtime `3d71ea3e33176f531d495bf1a1def10578b36ddf` passes **801 tests, zero failures, two ignored across 35 targets**, with every enabled example compile checked. The single warm combined run took 22.056seconds. The named merged solar executable then matched all 198 pinned original states exactly; its build took 2.926seconds. These are observed warm-run times, not a cold-build or general speedup claim.

Default Vulkan annotation stencil activation retains independently checked 3 actual viewport images, 8 translucent triangle images, 4 valid timing images and two negative images. Source review corrected optional timing query ownership before startup publication and context destruction; a fresh actual app allocation-refusal/retry lifecycle confirms retirement before zero-ledger cleanup. Captures retain their exact frozen source hashes. [Independent review](../annotation-stencil-production/root-review.json).

The solar clock and document panel now have operative date/daylight/time/zone controls.198 actual source math states and 8 original panel states compare exactly; two focused GUI checks include document-preserving egui repaints. The direct shadow-map consumer remains isolated work, so P20 stays Partial. [Solar evidence](../solar-clock/README.md).

All seven CI jobs for preceding published `0fcadf72` have completed successfully. Current publication CI is separate. Metal material and sparse-console batches remain isolated until their discovered defects and required checks are resolved. The unchanged optional console acceptance was not repeated. Full model pixel parity, all 37 row criteria and all five release gates remain open.

Raw logs and the merged native solar output are retained byte-for-byte in `logs.zip`; `logs-manifest.json` contains every digest. Focused owner checks precede one combined stable-batch run; independent old GPU proofs were checked from their archives instead of recaptured.
