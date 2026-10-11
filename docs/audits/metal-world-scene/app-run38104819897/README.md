# Actual native application Metal viewport checkpoint

The opt-in Rust macOS host now passes the current borrowed Scene, camera, style and mutation revisions through its native Metal owner after input/navigation, then installs the prepared texture directly into the shared-device egui painter. Unsupported scene stages return to the existing UI route. Resource/device failures retain the previous drawable and show the exact refusal.

ARM run 38104819897 built source `f8cf94b912498e6afdf543ac632e72725cdfe060` and passed eleven actual application states. Independent review verified all 28 source hashes, 175 literal interior pixels plus the stale-position patch, and four rejecting checker mutations. After cold ring seeding, warm/camera updates retain geometry and matrix counters; one moved frame writes 64 matrix bytes. Later ring catchup is separate and retained. Teardown returns the ledger to zero. The artifact ZIP and root review are canonical.

This uses synthetic egui events, actual update_ui/viewport/texture ingress and native Metal readback. It is not whole-image original parity or physical window/input acceptance. The separate retained Intel artifact built successfully but visibly paused when the native texture size/alignment query returned 0/0; the subsequent upload assertion failed. No memory admission was weakened.
