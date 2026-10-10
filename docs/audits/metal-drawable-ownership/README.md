# Caller drawable ownership transfer candidate

The caller retains an immutable drawable ID before `presentDrawable` and `commit`, then releases its drawable reference after final CPU submission. The presented handler receives the drawable from Metal and records callback time; no unretained drawable query occurs afterward.

Apple advises releasing caller drawable references promptly after final CPU work to avoid exhausting the limited drawable pool: https://developer.apple.com/library/archive/documentation/3DDrawing/Conceptual/MTLBestPracticesGuide/Drawables.html . Scheduling remains `MTLCommandBuffer.presentDrawable` before commit.

Command, paint, readback, callback, window, layer, device and ledger ownership remain retained on unknown completion. GPU completion alone is not displayed proof. The existing positive callback timestamps, exact resize, six displays and 15-second deadline remain unchanged. Both Apple architecture all-target cross checks pass; actual native presentation replay is required. Prior skipped/unknown callback failures remain retained on this branch.
