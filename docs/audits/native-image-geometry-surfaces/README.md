# Actual NVIDIA image geometry surface controls

Production source c2ebb57f; fixture extension be7f8751. Actual original frozen 6be29fe437a7cd992d4e079f71720821b25216b9, Qt NVIDIA RTX 5090, and production headless Vulkan RTX 5090. The source disables only QPainter image outlines. Original GPU upload, UV, projection and shading remain unchanged.

All 24 literal full 800×600 RGBA frames match exactly. Eight additional cases cover oblique orthographic and perspective cameras, asymmetric frames, negative V, offscreen clipping, near clipping and arbitrary colored alpha texels. Strict checker requires the complete named corpus, actual device metadata, zero warm uploads/hash/decode, and rejects the retained inverse-V output, three device mutations and a one-channel pixel mutation.

The original report retains its earlier descriptive sixteen-case scope string; it contains all 24 actual case records. No report data were rewritten. Geometry helper source extension is be7f8751. Build log and SHA256 inventory are retained.

Replay: pinned parity Python scripts/check-native-image-surfaces.py ORIGINAL NATIVE docs/audits/native-image-planes/native-rtx --geometry.

This establishes GPU image surfaces for this corpus. Missing original dashed/selected QPainter outlines, mixed transform workflows, unsupported image formats, broader texture/style performance and native Metal rendering remain open.
