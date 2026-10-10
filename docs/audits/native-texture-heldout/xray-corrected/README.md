XRay correction, bounded software proof

Actual original Qt/Mesa corpus remains in ../original; the initial failures
remain in ../baseline and phase-only results in ../phase-corrected. The same
mandatory 15-case checker now reports 14 exact sampled cases, two negative
controls, and the retained Shaded average failure (exit status 1).

Original viewport.py1652 uses sampled opaque texture fronts at opacity .6
with depth writes disabled, regardless of show_textures. Back opaque and all
transparent texture runs retain their own opacity; _xray_face_depth draws
all cached texture positions without alpha sampling. Native submission now
uses these pipeline states without rebuilding the texture vertex/image cache
when switching textured→XRay→textured. The original five-case production probe
passed all25 RGBA samples and actual unsignaled-fence, budget rollback/retry,
retirement, warm hash/decode/upload and new style-switch reuse assertions.
GUI library tests:94 passed,1 ignored.

This is llvmpipe Vulkan versus original Mesa Qt. Original NVIDIA Qt paired
reference is still required for the separately retained RTX cutout discrepancy.
Shaded average, additional style/texture controls, compression/streaming,
GPU hardware broader execution and native Metal acceptance remain open.
