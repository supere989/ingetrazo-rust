# Source-derived Qt byte-span correction candidate

Qt v6.12.0 qdrawhelper_p.h BYTE_MUL and qcompositionfunctions.cpp solid SourceOver multiply premultiplied 8-bit channels by 8-bit raster span coverage, rounding EACH intermediate channel before compositing. Source law: t=pixel*coverage; (t+(t>>8)+128)>>8. Actual public QPainter DestinationIn validates all65536(channel,coverage) pairs and262144channels. Source URLs and file hashes are retained.

The production CAD2D fragment candidate implements that literal integer rule. Actual RTX black-mask and all three colored AA span controls now match every6300testedpixel at both sizes. Ten fullcoverage heldoutcolors remain exact. Earlier58/38/43coloredpixel failures stay retained in cad2d-corrected-aa-spans.

Gradient interpolation STILL FAILS: dark4430, unequal3870, reference3620 pixels of6300 at each size. The whole strict checker intentionally exits1. Do not promote gradient/global brush opacity or full shape antialiasing. Global brush opacity modifies brush colors before span composition; the earlier15-case global-opacity comparison remains a distinct boundary. Native coverage inputs are now interpreted as quantized8-bit raster spans, justified by this source contract; arbitrary continuous alpha-gradient equivalence is not established.

This isolated shader candidate requires root review before integration. No palette fitting, tolerance widening, image copy or extra render pass. Geometry unchanged. Hardware witness is RTX5090 NVIDIA via owned-window Vulkan; native Metal remains untested.
