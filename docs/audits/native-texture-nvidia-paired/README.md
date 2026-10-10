Paired actual NVIDIA texture acceptance — bounded source proof

Original frozen6be29fe437a7cd992d4e079f71720821b25216b9, actual Qt6.12 OpenGL3.3
NVIDIA615.71.09, rendererNVIDIA GeForce RTX5090/PCIe/SSE2. Each source image
comes from its own newly created QOpenGLWidget grabbed with grabFramebuffer,
not a desktop capture. Strict renderer assertion was enabled. Source files
were mounted read-only; runtime dependencies were bound read-only from publish's
merged layer. Existing authorized X socket/auth was read-only; auth contents
are neither copied nor retained. No host package/config changes.

Fresh complete reproduction used scripts/run-native-texture-nvidia-paired.sh,
immutable dev-base imagea6597584287dfce8099218b3231240c1a939d73a573eeb260933f0d2b9b7070c,
CDI nvidia.com/gpu=0, read-only root/network-none/keep-id, disposable tmpfs,
60second external original process bounds and50second native process bounds.
Every process exited0. Native actual driver selects strict discrete NVIDIA
vendor0x10de, RTX5090/API1.4.351, explicit EGL Vulkan ICD, production World
shader/geometry/descriptor/image upload,4×MSAA and128MiB application budget.
The original/heldout file reports preserve every actual gl_info and IGZ hash.

Frozen source8f14f1b41ee1d0adc20de15a7f3789d5bb7e6291 uses root independently built
binary394cbd6d53a962dfdc3f0d364af3055e6992a2ebeb35301445f32356204238a5.
Corrected sourcea60d92e9d4ebffed4c5f6037f705044eaa8da7c6 uses separately compiled
binary87721e02e2d69bd98d7d310a7ab35d5abfb1cb0a57011a5feedb32af899811d2.
The later average experiment is outside the renderer module tree/unwired.

Mandatory source-five: frozen and corrected both pass25/25 exact interior
RGBA samples, pixel mutation and missing-case negatives. Both real native
probes additionally assert unsignaled draw fence refusal before admission,
real image-budget denial/old GPU fullimage+ledger preservation, replacement
retry and fence-retired sampled image, warm0hash/decode/VB/image upload.
Corrected probe additionally asserts textured→XRay(show_textures=false)→textured
reuses the same geometry and image cache.

Mandatory broader15: frozen12/15 exact; wrong mixed color-back/texture phase,
XRay and Shaded-average fail. Corrected14/15 exact, retaining Shaded-average
failure (checker exit1). This includes explicit back-color overrides, alpha
sides, opaque texture backs, holes, reflected/slanted groups and both mixed
face orders. No byte tolerance changes. Both heldout checkers retain their
pixel mutation and missing-case negatives. strict-summary.json records all
exit statuses. This is sampled interior acceptance, NOT whole-image AA proof.

The earlier Mesa-source versus NVIDIA-native texture mismatches remain in
previous audits. On paired NVIDIA, cutout[199,102,102] and center[190,190,190]
match exactly, as do all source-five samples; no shader/palette compensation
was applied. Actual original hardware exhibits the same driver difference.

Reproduction: transport frozen/corrected binaries to readonly host paths; set
FROZEN_PROBE,CORRECTED_PROBE,OUTPUT (fresh), existing DISPLAY/XAUTHORITY; run
the retained transport script. It verifies source commit and refuses existing
output, creates only named owned disposable containers, and mounts no writable
source/runtime/driver files. Copy resulting artifacts back into publish, then
run strict check-native-texture-draw.py for five and heldout.py for15. Full
heldout is expected to remain failed until Shaded average is implemented.

Shaded average, additional hidden-line/monochrome/wire texture behavior, JPEG
draw/profile/high-bit/other Qt formats, compression/mip streaming, general
performance/device loss/driver OOM, Windows hardware and native Metal/macOS
implementation/hardware gates remain OPEN. No RT/BC capability promotion.
