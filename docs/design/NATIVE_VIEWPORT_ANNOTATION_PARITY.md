# Native viewport annotation parity requirement

Source: pinned Python 6be29fe437a7cd992d4e079f71720821b25216b9, views/viewport.py:7084-7267. Root audit 2026-10-10.

Production Rust viewport.rs native GPU branch skips build_scene_render_data/render_scene_to_egui. Its subsequent native callbacks currently supply image borders and selected snap overlays, not dimensions or leader text. Merely invoking existing fallback annotations is insufficient: pipeline.rs uses fixed colors, tick geometry and label boxes that differ from source.

Required dimension behavior: entity visibility; selection/hover orange versus document dimension-style ink; live line endpoints; projected endpoint admission; extension pen1.0 and dimension pen1.5; 16 sampled subsegments with both endpoints visible; endpoint occlusion; document arrow/tick/none endings. Arrows have length10 and half-width2.2 device pixels; ticks use perpendicular4-pixel offsets. Midpoint labels require visibility, source unit/precision/display_text semantics, default native font at document point size in bold, white230-alpha halo at(+5,-4) and ink at(+4,-5).

Required leader behavior: entity visibility; selected/hover ink; occluded leader pen1.2; visible anchor disk radius2.5; source multiline splitting, horizontalAdvance-based block placement and native fontMetrics height; white230-alpha halo displaced(+1,+1). Empty text and perspective eye boundaries remain in scope.

Implementation must use direct native CAD/text submission with retained immutable geometry/font resources and correct viewport clipping/order. Do not rebuild the complete CPU scene merely to recover overlays. Cache invalidation must cover geometry/context, camera, document styles, visibility, selection and hover. Occlusion must reproduce source sampling/depth semantics, with representative measured costs and optimized persistent workers where required. Vulkan and native Metal acceptance are separate.

Acceptance: original and actual production native viewport captures of imported chapel dimensions plus authored arrow/tick/none, hidden layers, selected/hover, partial occlusion, multiline leaders, perspective and edited Undo/Redo states. Retain whole-image comparisons and reject omitted-overlay/style/occlusion mutations. A World-only fixture cannot establish application dispatch acceptance. All annotation/platform gates remain open until proven.
