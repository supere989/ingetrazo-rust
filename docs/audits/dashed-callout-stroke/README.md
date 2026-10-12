# Continuous dashed callout stroke

The admitted curved FlatCap/Bevel dash route now emits one continuous forward/backward boundary per run, matching source QStroker endpoint-rooted normals and joins. Dash phase, flattening, generic strokes and GPU blending remain unchanged. Contour storage reserves one entry per run; bounded boundary allocation and repeated-zero-edge endcaps are preserved.

The build passed in 7.194 seconds. The actual annotation replay remains strict FAIL: 15/20 PNG, 17/20 JPEG and 18/20 PDF comparisons are exact, with 20 original readers and 25 rejecting controls. Independent review confirms that only six existing callout PNG/JPEG comparisons improved; all other hashes, PDFs and controls are unchanged. Seven PNG pixels remain across five cases. Source captures were reused.

Current reports, raw native exports, logs and receipts are authenticated in the manifest. The preceding rotated-fill packet retains the baseline. This is software Vulkan evidence; no representative performance or new hardware/Metal acceptance is claimed.
