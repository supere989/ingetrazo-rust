# Actual native Metal offscreen acceptance
Reviewed source0f3b335, CI38044914248. Both ARM and Intel native jobs compile/link/run successfully on Apple Paravirtual devices. Root independently downloaded artifacts and compared every captured64x64 RGBA byte with all six frozen actual Qt references: ARM framebuffer-fetch and span-compute each exact, Intel span-compute exact. Intel Apple-family fetch is explicitly unsupported.

Both actual GPUs compiled and executed a literal older BYTE_MUL shader and rejected it with2304 differing pixels. Shader compilation errors are not accepted as negatives. Warm cache/uniform changes avoid geometry and palette preparation; actual candidate admission failure restores prior image/accounting, and acknowledged normal retirement returns the tracked ledger to zero.

Physical Mac, native application WSI/paint, broader triangle/cosmetic shapes, world/textures/material/compression/ray tracing and full renderer parity remain OPEN. The whole workflow Linux job is not claimed green by these native-job results.
