# Native Metal source stencil candidate

Native SDK/runtime acceptance is pending. No pixel closure claim.

Pinned source viewport.py6565–6628 uses geometry-only backface marking (Replace1, color/depth writes off, LEQUAL), then its section frame quad (Equal1, clip off, depth write, polygon offset2/2). _draw_instanced_raw2948 draws prototype color/texture vertex ranges without texture sampling. The preceding opaque/backoverride passes reset use_tex and hard_cutout before this mask; earlier visible depth retains texture cutout effects. Reflected prototype winding remains explicit.

Off-fill keeps original Depth32Float targets and pipelines. Fill uses a separately cached pipeline and a lazily admitted Depth32Float_Stencil8 target; native size queries and actual allocatedSize retain existing budget/lifetime rules. Apple documents that this combined format may allocate64bits/pixel: https://developer.apple.com/documentation/metal/mtlpixelformat/depth32float_stencil8 . Six cached source frame vertices are retained by immutable Prepared owners. Camera-only frames do not rebuild/upload them. Annotation queries consume the same depth attachment after the cap draws; combined-depth query sampling still requires native acceptance.

The named GUI example native_metal_section_fill reuses ten existing section-original-gl recipes and real production Scene/World/Qt-camera preparation. It captures native RGBA and fill-disabled frames, with three warm repeats and upload/allocation counters. The checker compares every RGBA pixel with retained originals and reports cap-mask mismatches separately. Source cut contours are included in those originals; the current Metal contour stage remains missing, so failures are preserved rather than removing/masking contour pixels. No percentage threshold, CPU polygon fill or recapture is used.

Focused Linux checks passed (metal library1.93s, named GUI example11.65s); these do not compile macOS-only encoder paths. Initial missing original submodule symlink caused refusal before compilation and is retained in /tmp/metal-section-check.log. Source and target preserved; native SDK and GPU execution must follow review in a single coordinated batch.
