# Literal section stencil input streams

Pinned original `views/viewport.py` 6584–6599 draws `_faces_count`, `_tex_faces_count`, then `_draw_instanced_raw` (2948–2968). `_faces_count` is established at 5022 before translucent/back/front-cull colored tails are appended. `_tex_faces_count` is established at 5140 **after** translucent/front/back texture runs are appended. Therefore texture opacity filtering would incorrectly narrow the original mask.

Correction removes expanded colored tail geometry from the mask, excludes classic front-cull opaque spans using existing immutable opaque-run metadata, and preserves complete eligible instanced prototype streams. All textured runs remain geometry-only and unchanged, including translucent/back-painted texture tails; no shader sampling or discard is added. Inherited translucent placement is not blanket-filtered. No new allocation/upload/cache key or per-frame geometry preparation is introduced.

The unchanged ten-image strict checker remains authoritative. This followup has source review and diff-check only; no build/capture/CI. Transparent untextured cap mismatch (20,164 pixels) is retained as the motivating observed failure; native closure is pending. Source pin: 6be29fe437a7cd992d4e079f71720821b25216b9.
