# Retained Metal egui texture snapshot admission

The managed texture update now converts borrowed `ColorImage` pixels or the pinned font iterator directly into one admitted destination Vec. A sparse change copies the old snapshot once then encodes the patch directly; it no longer clones all source Color32 entries or constructs an intermediate RGBA patch. GPU publication remains transactional and immutable. Old submitted PreparedPaint owners retain the original image, sampler and CPU snapshot.

Every CPU snapshot reserves its requested bytes before allocating, reconciles actual Vec capacity, and decrements only when its final owner drops. The 256 MiB guard therefore includes old frames and staged replacement images, not only the current map. Incoming egui delta storage is outside this snapshot counter; pending host deltas still need a separate admission policy. Actual GPU images retain Metal allocatedSize accounting.

The native `egui_draw_probe` now requires initial retained CPU bytes20, replacement current-cache bytes20 but retained bytes36 while the original frame is held. Existing GPU old-image/partial/destination-preservation controls remain unchanged. Pure admission tests reject over-limit before allocation and prove final-owner accounting. Both macOS targets compile examples and tests; actual native execution is pending.

Published stats distinguish current cache bytes, all retained snapshot capacity, converted source pixels, copied snapshot bytes and uploaded image bytes. Counters increment only for successful transactions. Stable frames retain zero updates; sparse updates still upload a full image replacement. GPU-to-GPU copy plus sparse patch/async immutable ownership is a future optimization, not implemented here.

Mipmap generation/filtering remains explicitly unsupported. BC/ASTC capability advertising is not active compressed decode/upload parity. Source image format, world sampling and physical-Mac performance acceptance remain open.
