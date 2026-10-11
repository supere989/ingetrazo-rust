# Apple depth-resolve witness prerequisite

The retained NVIDIA GL witness is device/format scoped. This batch changes no production or strict-reference policy. `native_metal_document_annotations` writes `sample-diagnostics.json` before any World/CAD admission, including actual Metal default positions, programmable capability, registry ID and existing strict-policy result.

On each actual Apple runner, before the document probe:

```sh
clang -std=c11 -Wno-deprecated-declarations scripts/native-apple-gl-depth-resolve.c -framework OpenGL -o "$RUNNER_TEMP/apple-gl-depth-resolve"
"$RUNNER_TEMP/apple-gl-depth-resolve" > "$EVIDENCE/apple-gl-depth-resolve.json" 2> "$EVIDENCE/apple-gl-depth-resolve.log"
python3 scripts/check-apple-gl-depth-resolve.py "$EVIDENCE/apple-gl-depth-resolve.json" > "$EVIDENCE/apple-gl-depth-resolve-check.json"
```

The accelerated CGL core witness mirrors the retained recipe: 8×8 combined D24S8, requested 2/4 samples, actual sample positions, sample-mask writes of rotated distinct depths, DEPTH NEAREST blit, all64 resolved pixels per shift. Shader version150 expresses the same fullscreen triangle/depth law on Apple's core profile. The checker requires a stable uniquely identified selector across every rotation; it never assumes sample0. Keep failed stdout/stderr and run the document probe independently so witness failure cannot hide Metal observations.

Missing actual Apple execution is explicit. CGL renderer identity does not yet establish correspondence with Metal registryID; the witness reports `same_metal_device_verified:false`. Even a passed inventory cannot authorize production policy until that device correlation and exact native resolved-depth law are reviewed. Native C compile/runtime must be proven on Apple, not inferred from Linux cross metadata. No CPU fallback or altered source pixels.

Focused validation: x86_64-apple-darwin `cargo check -p ingetrazo-metal` passed (2.33s). No broad suite or source recapture. Original Python remains unchanged.

Named GUI cross-check failed before GUI compilation: libz-sys invoked Linux `cc` with Apple `-arch` flags. Full log retained; this is setup failure, not Apple runtime evidence. No retry or broader build.
