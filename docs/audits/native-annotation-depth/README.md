# Native annotation depth prerequisite
This stage retains World depth (STORE and sampled DEPTH-only view) and evaluates bounded annotation queries directly on GPU. It does not activate production annotation painting or assert original GL multisample resolve policy.

CPU prepares Qt float projection followed by Python double division/pixel truncation and narrows normalized coordinates for inverse reconstruction. Source double forward-distance tolerance is encoded with a directed-up float boundary, preserving comparison against finite GPU float distance without shaderFloat64. Four focused tests include all18 captured original depth cases and a nearest-rounding negative.

GPU owns immutable query/output buffers, descriptors and pipeline. Changed queries upload once; unchanged queries reuse buffers. Source inverse math uses precise/no-contraction arithmetic. Diagnostic reads only a few output flags. Preparation admits actual storage/workgroup limits, resource context and completed fence before descriptor reuse; replacement uses publication preflight. Unknown completion retains owners. Target replacement invalidates depth binding. Original GL resolve/sample-position mapping remains required; diagnostic sample indices are explicit.

Software llvmpipe actual four-sample controls: front/behind/on-surface/background, unchanged query reuse, invalid sample refusal, stale-target refusal and final ledger0. Resource/evaluator execution proof, not original live viewport or NVIDIA parity.

Build cargo --locked -j2 -p ingetrazo-gui --example native_vulkan_annotation_depth_probe with reduced-debug owned target. Run with external timeout30s and explicit --software-fixture; default adapter is strict NVIDIA discrete.

Shaders use pinned shaderc0.8.3, entry main, default options. Compiler selects Compute for .comp, Vertex for .vert, Fragment otherwise. Source and SPIR-V retained; compile command ingetrazo-image-shader-compiler INPUT.comp OUTPUT.comp.spv. Single/MS variants differ only in texture declaration/fetch.
