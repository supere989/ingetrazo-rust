# Actual GPU draw / host hardening followup — execution pending

`cargo run --locked -p ingetrazo-metal --example egui_draw_probe -- --output=DIR`
on native macOS compiles the production egui MSL pipeline, encodes and submits
actual nearest 2x2 RGBA texture plus FontImage black glyph and scissor meshes,
then BLITs the actual private GPU target into a shared readback buffer. Both whole
64x64 opaque images (initial and sparse partial) must match independent literal
RGB expectations. Font coverage here is1 only; no font AA/Color32/CAD parity claim.
The shared application never calls this diagnostic readback path.

Warm ten GPU images have unchanged geometry/texture upload counts. Actual
second-buffer admission failure and image admission failure preserve old sampled
image/cache/ledger; malformed partial is refused. An actual Metal shared-event
GPU wait holds a submitted frame: 50ms diagnostic timeout preserves every owner
and poisons admission, signal then actual completion permits explicit diagnostic
release. A separate pending Drop retains all native owners/charged bytes for
process lifetime even after event release, never frees by timeout guess. Completed
Drop queries actual status and frees without a blocking wait.

`scripts/check-metal-egui-draw.py DIR` independently regenerates both expected
opaque images and requires full byte equality/complete producer protocol. Five
synthetic negatives reject empty, white missing draw, black startup, flipped
origin, stale partial. `--self-test` exercises negatives only, not GPU execution.
Producer refuses known existing artifacts, avoiding stale positive result reuse.
Linux ARM/Intel crosschecks establish compile only; runtime MSL and every GPU
assertion still require actual Mac runner execution. Physical Mac WSI remains OPEN.

Host review corrections: capacity3/zeroextent gate precedes mesh/texture
preparation, ordinary raw input stays queued and pending texture deltas preserved;
close requests still enter app guards. about_to_wait always installs future wait
control flow even after poll errors, preventing expired-deadline spin. Prepared
paint admits actual f32 shader arithmetic for its bounded physical extent domain
before buffer allocation and validates actual extent before encoding. Native
CAMetalLayer owner now retains Arc<winit::Window>, including native view/delegate
ownership through indefinite GPU/presentation quarantine; no dangling view/window
lifetime assumption after shared app State drops.

Window-server execution, ownership/presentation retirement, dirty native close,
resize/DPI, texture filtering/repeat/real glyph AA, full world/CAD callbacks,
materials/texture/compression/RT and physical ARM/Intel Mac performance remain
required. No default renderer promotion or full application acceptance yet.
