# Production composer byte-span independent rerun

Fixed root-review source efddd76 includes current capsule geometry and literal Qt BYTE_MUL fragment correction. Built in a new isolated worktree and unique target. Direct CAD2D explicitly enabled. Native software Vulkan draws actual production physical shape commands; original actual QPainter AA images were regenerated and are byte-identical to all nine previously frozen references.

All nine strict whole-image cases still FAIL: rect2084, rounded1976, ellipse1932, polygon1744, invertedline1048, arrow1115, terrainticks1608, terrainhatch3936, terrainband1311 pixels. Checker exits1; no tolerance or reference changes. Three mandatory inventory negatives reject missing/duplicate/reordered cases. Geometry/palette command comparisons and five mutation negatives are retained separately.

Pinned shaderc0.8.3 rebuilding only cad2d.frag produces byte-identical SPIR-V, with hashes and command retained. The diagnostic AA pixel-mask color correction is exact separately; production geometry/coverage remains approximate and unpromoted. No full-sheet/export or native Metal claim.
