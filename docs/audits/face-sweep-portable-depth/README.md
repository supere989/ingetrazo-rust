# Portable source-camera clip depth

Runtime `9fdf1788` retains the original Qt-f32 MVP on the portable policy. A draw-only high bit converts homogeneous clip Z to `(z+w)*0.5` in the vertex shader **after** MVP multiplication. Native EXT depth control and ordinary Vulkan projection leave that bit clear. Low bits still own section clipping/cutout. Surface, edge, texture, image-plane and contour ingress carry the policy; unsectioned sweep solid/texture/line draws preserve it while clearing section clipping. Solar receivers inherit it; shadow-map shaders retain their existing independent depth policy.

The retained RTX5090/615.71.09 experiment disables native depth negotiation. All 24 complete production App/World/label source frames match exactly. Four new native-EXT reversed-door frames also match. The checker authenticates literal case/phase inventory, pinned original frame hashes, runtime/binary provenance and 84 transparent/white/one-byte negatives. Actual tool Cancel and Save occur afterward, and each shutdown reports zero committed GPU bytes. This is method-dispatched application rendering, not a physical-pointer acceptance claim.

The initial portable implementation omitted the high bit on unsectioned sweep textures: two actual frames differed by 7149/7106 pixels, max194. Those images remain in the archive. The omission was corrected before the final frozen run. Historical CPU pre-rounded-Z-row failures539/1079 remain in the prior `face-sweep-camera` audit. No source recapture, mask or tolerance change occurred.

Forced feature-off on RTX5090 proves this shader fallback, not driver coverage on every physically extension-less device. The six-case source corpus has no section cap/contour/image-plane or solar frame; those policy ingress changes are code-reviewed here, not newly pixel-proven. Native PushPull f64 handler-dot versus Qt-f32 raw distance remains separate open scope. No general renderer/full PushPull promotion follows.

Replay:

```sh
/tmp/ingetrazo-parity-python/bin/python scripts/check-face-sweep-portable-depth.py docs/audits/face-sweep-portable-depth docs/audits/face-sweep-label-source
```

Focused named application build passed. Two depth-policy unit checks passed; no broad suite was run. Shader SPIR-V was rebuilt with the existing pinned shaderc0.8.3 helper. Source and bytecode hashes are retained in provenance.json. All processes are terminal; raw outputs/target remain for independent intake until released.
