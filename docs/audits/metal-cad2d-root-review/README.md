# Independent native Metal CAD review
Reviewed owner source d7f070e on public integration2f66f1e. Both actual Mac target crosschecks pass, as do three host span-plan controls including six frozen actual Qt whole-image references. These do not prove Metal GPU rendering.

The production submission path is nonblocking, owns up to three frames, polls actual command completion, retains old immutable geometry/palette/target owners while commands remain in flight, and quarantines unknown/error ownership. Warm span cache checks no longer allocate a batch-layout Vec before checking reuse. Engineering readback and bounded waits remain explicit separate entrypoints.

Native macOS CI now compiles both MSL shaders and executes strict ordered-span compute whole-image comparisons, admission rollback and normal retirement. Apple framebuffer fetch executes when supported; explicit capability refusal is recorded separately and cannot claim rendered proof. Actual GPU results are pending. Native application WSI, general triangle/cosmetic parity, full world/material/texture/RT and physical Mac acceptance remain OPEN.
