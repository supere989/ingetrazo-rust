# Cosmetic pen candidate: strict acceptance failed

Candidate c5c179f ports Qt6.12 QCosmeticStroker solid antialias mathematics, physical 26.6 truncation, width-opacity, transformed cubic subdivision, and ordered per-segment source-over spans. It coalesces runs within each segment and preserves repeated pixels across source segments. Finite coordinates, work, temporary pixels and final spans have bounded admission. It does not implement dash or source device clipLine policy; coincident points and broad transforms remain unverified.

Fresh actual QPainter versus production native software: 36/36 thick-path phase/DPI cases exact; small zoom 8/9 exact. Whole mandatory 45-case verdict is FAIL. Rounded pixel (28,12): original RGBA (114,118,124,255), native (114,119,124,255). Literal ordered cosmetic coverages there are 43 then133; authored stroke #1e242c. Source target is QImage Format_ARGB32. No tolerance or case exclusion was applied. Earlier nine small-zoom failures remain in composer-gray-phase-density.

Capture binary SHA256 b90ff719f76e44d47f88dc6c723772c9977160b96a41124cc1393dae40f1d7e7 was built from the dirty numeric candidate immediately before c5c179f; the commit additionally admits temporary pixel storage explicitly, without changing admitted numeric operations. It is not a fresh committed-source hardware result. Source original commit 6be29fe437a7cd992d4e079f71720821b25216b9, Qt6.12/Python3.13; test backend software Vulkan. Root independently found three overlap failures in nine hardware images of the older thick checkpoint; these remain unpromoted.

Qt operator/target-format audit is required before replacing fixed-function blending. Current thin counterexample does not establish a universal byte-rounded destination multiplication rule. Shader changes are held pending that audit.

## Same-pass Vulkan constraints

Current swapchain pass has COLOR_ATTACHMENT usage only and no input attachment/self dependency. CAD draws can contain source-overlapping spans within one indexed draw. Attachment feedback-loop layout alone does not order these reads. Rasterization-order attachment access is the direct programmable blend route when enabled: input/color attachment references and pipeline/subpass flags provide ordered reads. Without it, color-write to fragment-input-read BY_REGION self dependency/barrier is needed between nonoverlapping draw groups; preserving only current palette batches is insufficient. Swapchain INPUT_ATTACHMENT support and per-image descriptors must be queried. Existing egui writes also require availability before first CAD read. No additional target/pass or CPU image is proposed.

Primary references:
- https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_rasterization_order_attachment_access.html
- https://docs.vulkan.org/samples/latest/samples/extensions/rasterization_order_attachment_access/README.html
- https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_attachment_feedback_loop_layout.html
- https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qcosmeticstroker.cpp

## Source-ordered draw grouping diagnostic

`qt_cosmetic::disjoint_ranges` greedily retains maximal prefixes with disjoint integer one-row pixel intervals. It bounds span admission, uses row/x ordered maps, never reorders any span, and flushes only on actual overlap. Three kernel tests pass, including full order preservation and pairwise disjointness. Rounded small fixture: 311 coalesced spans partition into 28 ordered disjoint draw ranges (not311). This is diagnostic metadata only; no renderer/barrier changes are activated. Float-transformed general triangles do not inherit this integer pixel-aligned guarantee. Source width opacity and actual destination operator must still be established.
