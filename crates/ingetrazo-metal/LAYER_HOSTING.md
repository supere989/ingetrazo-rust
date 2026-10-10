# AppKit custom layer attachment order

Apple NSView.wantsLayer documentation distinguishes AppKit layer-backed drawing from a supplied layer-hosting view. Install the supplied CAMetalLayer with setLayer before setWantsLayer(true). The previous native attachment performed these calls in reverse order.

Primary source: https://developer.apple.com/documentation/appkit/nsview/wantslayer . Reviewed 2026-10-10; search-indexed official documentation explicitly identifies the property-setting order as crucial. Runtime diagnostics now retain prior wantsLayer state and require the actual NSView.layer identity to match the supplied CAMetalLayer after attachment.

This corrects a documented source defect. It is not yet evidence that attachment order caused the ARM presentation failure. Actual 1058e3a ARM run38048814938 artifact11668289072 retained correct visible800x600 layer geometry, three GPU-completed commands with positive GPU timings, zero terminal callbacks, zero displayed frames, and failure at the unchanged15-second deadline. Intel passed in the same run. No terminal callback or display criteria are relaxed by this correction. Unknown owners remain quarantined. Native ARM execution must determine the result before any host adoption.
