# Actual native raster-outline candidate rejected

Source13486d5379de38751d36717d1e910c1c0654cbdf. Fresh original NVIDIA Qt full display source is docs/audits/image-outline-front-projection/original. Native exact new-owned-window Vulkan RTX5090 client captured by scripts/capture-owned-native-image-outlines.py; all24 processes closed normally. The capture report passed field records capture/lifecycle completion only, not visual equality.

Strict whole RGBA comparator rejects22/24. Top controls differ2176 border pixels, max135 channels. Hidden-layer and near-clipped cases are exact. Image interior/surface correctness is retained separately by the outline-ablated24 actualGPU proof; this does not promote whole display.

OriginalTop border is full opaque gray in one row with5-on/1-off visible pattern. The candidate assumed antialiased QImage raster operator: fractional two-row4-on/2-off. Its standalone Qt raster kernel proofs cannot establish actual QPainter-on-QOpenGLWidget engine semantics. The native cached callback consumer is implemented but is not accepted rendering parity.

No tolerance changes or palette adjustments. Operative widget paint-engine and stroke geometry are the required next correction. Native Metal outline consumer remains open.
