# Isolated source-policy color diagnostic

Pinned Qt6.12 QNSView.colorSpace defaults to NSWindow.colorSpace when no explicit surface color space is requested. Its native Metal layer receives that exact CGColorSpace. Primary source: https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platforms/cocoa/qnsview_drawing.mm . The raster backing store likewise uses that view space: https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platforms/cocoa/qcocoabackingstore.mm .

Opt-in presentation_probe --diagnostic-window-colorspace selects the actual owned NSWindow CGColorSpace once at attachment. Missing native color metadata refuses explicitly. Default attachment still selects authored sRGB, preserving previous failures. No GPU bytes, shader, palette, extra image/pass, presentation owner, callback criterion, resize target or15-second deadline is changed.

The own-window harness --window-color-space combines this path with actual GPU drawable-center readback and unchanged exact screenshot/presentation checks. CI retains normal sRGB and separate window-policy outputs. The Qt observer also records its actual effective QNSView color space. Both architecture all-target cross checks pass; this is policy alignment awaiting actual causal Intel/ARM proof, not a fix claim.
