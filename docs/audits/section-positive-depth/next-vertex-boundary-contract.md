# Proposed compiled vertex boundary observation (prereview only)

No runtime code, build, capture or GPU dispatch is authorized by this record.

## API constraint and scope

Apple MSL specification, 2026-06-04, sections5.1.1,5.2.1.1,5.11: a vertex function that writes device buffers must return void, and void vertex functions are used with rasterization disabled. Therefore an output-buffer store cannot legally be inserted into the current rasterizing Out-returning world_vertex. There is no supported transform-feedback tap used by this implementation. Do not describe a separate void function as the unchanged production binary.

Primary: https://developer.apple.com/metal/Metal-Shading-Language-Specification.pdf
Rasterization switch: https://developer.apple.com/documentation/metal/mtlrenderpipelinedescriptor/israsterizationenabled

OpenGL transform feedback captures selected compiled vertex outputs when no geometry shader is present. Its varying selection applies at the next link. This can retain literal original basic.vert/basic.frag bytes but does not promise the linker leaves the rasterizing shader's instruction selection unchanged.
Primary: https://registry.khronos.org/OpenGL-Refpages/gl4/html/glTransformFeedbackVaryings.xhtml

Production Metal library compilation passes options=None. No explicit no-fast-math/invariance policy is present. Apple documents fastMathEnabled default=true; this permits floating-point optimization and does not establish the actual instruction sequence. Observer replay must use the same options=None and must not add precise/fma/order controls or change production options.
Primary: https://developer.apple.com/documentation/metal/mtlcompileoptions/fastmathenabled

## Smallest defensible paired contract

1. Existing named closed-iso stage consumer retains untouched ordinary frame9fab and all normal stage records. Only an explicit cold diagnostic request creates extra observation resources.
2. Read actual admitted immutable Shared native mesh vertex and index buffers, model matrix, camera and section bytes after preparation. Bound60vertices/60indices/one identity model. Serialize exact float/u32 bits, colors, vertex stride and index order, not reconstructed scene data. Match all20 ordered triangles against retained source CPU records before attributing contenders. This closes actual upload identity rather than repeating integer recipe checks.
3. Create one separate rasterization-disabled Metal vertex replay pipeline, using a mechanically validated copy of the actual world_vertex arithmetic through world, section_distance, camera multiplication and final z-remap. Same immutable buffers and uniform bytes, same device, options=None; no compute kernel, no fragment, no attachments/depth/bias/section clipping. The only changed tail stores world, pre-remap clip, post-remap clip and clip distance to a unique Shared record indexed by admitted vertex_id. Draw60unique vertices with drawPrimitives(Point), so no output write alias. One fixed record array<=4KiB, explicitly admitted and retained with command/pipeline/inputs until Completed; timeout quarantines owner. This is actual compiled vertex-stage arithmetic in a separate diagnostic binary, not proof of the rasterizing function's exact registers/FMA.
4. Source harness links an observer program from exact original shader bytes with transform-feedback varyings gl_Position, gl_ClipDistance and v_world. Verify linked varying names/types/component counts; fail rather than patch the original shader if a built-in cannot be captured. Replay the actual ordinary opaque VBO/VAO stream with its actual uniforms/generic identity model attributes and rasterizer discard; capture only60vertices. Retain GL identity/compiler version, raw source upload bits and linked program/shader hashes. Full original ordinary frame must still match the immutable closed-iso PNG. No section/raster/depth prefix or framebuffer recapture is needed to read this buffer.
5. Compare exact pervertex world/section clip-distance/source GL clip xyz/w versus native replay pre-remap xyz/w, then compute the documented native z-remap from captured source z/w only as a labeled CPU control. All records finite; actual inputs/model/camera/plane must match before shader arithmetic inference.

## Interpretation gate

If inputs differ, stop at the first upload/triangle/uniform mismatch. If compiled replay positions differ with inputs identical, expose which vertex/component differs and actual bit patterns; do not attribute the production raster mismatch solely to replay, because the output-store/link changes can affect instruction association. If outputs match, the remaining boundary is actual rasterizing VS codegen/user clipping/raster setup/D24-versus-D32 comparison; generated clipped vertices are not directly captured by these preclip APIs. This probe cannot claim to resolve that boundary or justify bias changes.

Approval question for parent review: is this explicitly qualified vertex-replay observation sufficiently discriminating to execute, or should we prefer a compiled production pipeline GPU capture/disassembly route instead? No extra ARM run should be spent under a false same-binary claim.
