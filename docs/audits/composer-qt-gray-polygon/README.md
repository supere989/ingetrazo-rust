Convex polygon gray coverage acceptance

Production source d282bd0 uses source-derived bevel contours, normalizing convex orientation before outer/inverse inner traces. Eight actual Qt QPainterPathStroker controls (3/4/7/24 sides, both orientations) match whole-image bytes. The earlier reversed-orientation failures remain retained. Four cache/admission/geometry unit tests pass.

Actual production software Vulkan nine-case comparator remains FAILED: rectangle and polygon-clamped are exact; seven remaining shapes fail unchanged strict pixels. Terrain-band improves but still differs at 1,048 pixels. No hardware, curve, full-sheet or complete AA claim. Existing immutable warm cache and 8 MiB span admission remain; native GPU warm metrics were not newly measured.

Reproduce: pinned Python scripts/check-composer-bevel-outline.py WORKTREE OUTDIR; then INGETRAZO_SHAPE_DIRECT=1 software Vulkan scripts/check-native-sheet-shape-pixels.py target/debug/examples/sheet_shape_render_fixture docs/audits/composer-byte-span-production/original OUTDIR. The nine-case command intentionally exits 1.
