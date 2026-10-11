# Reviewed followup to native stage failures

The retained ea575 run proves both native Mac builds/defaults and ARM original-document annotation/actual CLI startup. Its independent failures remain in `native-platform/metal-stage-repair`; no failure is relabeled a pass.

The billboard diagnostic now restores the allocation limit only after verifying the refused render leaves committed/reserved accounting unchanged, and before allocating its separate capture buffer. It still inspects the accepted frame with the rejected camera, then retries normally. The earlier diagnostic stopped during its own readback allocation under the deliberately zero limit.

Standalone section targets use a pre-allocation available-capacity ticket and reconcile actual allocated size before publication. Heap-specific estimates cannot bound standalone texture allocations. Zero-size/over-ticket allocations still refuse, now with numeric descriptor/accounting details; the generic retained ARM error did not identify which branch failed. [API/source rationale](../metal-section-target-admission/README.md).

Independent source review corrected stencil submission: exclude expanded colored tails, retain the source complete texture stream and eligible instanced raw geometry. [Literal count boundaries](../metal-section-target-admission/stencil-streams.md). The strict ten-image checker is unchanged.

Retained Intel source/native pixels isolate a separate clear conversion issue: source background is RGBA(230,232,235,255), native was (229,232,235,255). This alone explains 458,100 of 458,392 closed-scene differences. Scene clear now quantizes the f32 value in RGBA8 before passing it to Metal's double API, preserving invalid-input admission. The source-byte regression passes; native whole-image parity is still pending. This bounded source observation is not a universal OpenGL driver rounding guarantee; the [Khronos normalized-integer reference](https://wikis.khronos.org/opengl/Normalized_Integer) permits implementation rounding differences.

The final GUI Apple metadata preflight passes in **2.31 seconds** at runtime head `ce623f42`. It checks Rust types for the actual section/billboard/document examples, not SDK/link/shader/GPU execution. The next stable integration CI owns actual native acceptance. Intel's unchanged strict annotation sample policy and unavailable accelerated CGL witness remain open.
