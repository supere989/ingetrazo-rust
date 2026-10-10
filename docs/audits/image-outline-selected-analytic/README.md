# Source selected-image line coverage

Actual Qt6.12 QPainter ARGB32 alpha coverage, QColor(243,115,41), width2, default SquareCap and antialiasing. All84 literal line controls match exact coverage at every occupied pixel, including both directions, clipping, near-axis, point and64 deterministic partly off-device lines. Missing corpus, coverage-byte and phase mutations reject. Fresh source helper uses actual QPainter and does not reuse the Rust kernel.

Source dispatch: Qt6.12 qpaintengine_raster.cpp non_complex_pen LinesHint calls QRasterizer::rasterizeLine, rather than the generic gray-path rectangle hypothesis retained separately. Official analytic source: https://raw.githubusercontent.com/qt/qtbase/6.12/src/gui/painting/qrasterizer.cpp (rasterizeLine/intersectPixelFP). Rust implementation carries GPL source provenance and uses bounded analytic fixed-point spans, no raster image.

The retained hypot-negative demonstrates source operation-order necessity: replacing source sqrt(sum of squares) and sqrt(old_squared/new_squared) with hypot and length ratios changed287 and159 coverage pixels in two clipped sweep cases. Exact source arithmetic resolves both; no tolerances or color adjustments.

This stage is standalone coverage, not an actual native outline UI or Metal proof. Front-plane clipping, scene overlay/cache/selection/layer behavior and actual framebuffer comparison remain required.
