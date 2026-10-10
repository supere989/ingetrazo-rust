# Native WSI failure diagnostics (no verdict change)

Actual ARM and Intel AppleParavirtual CI reached two unresolved frames and the unchanged 15-second deadline. GPU egui whole-image and font/sampler probes passed; presentation remains failed.

The read-only observer reports native view bounds, layer frame/bounds/visibility, winit window visibility, actual Occluded/Focused events, drawableSize, transaction mode, drawable ID, command status, GPU times and actual presentedTime once per second. It leaves retirement and strict presentation acceptance unchanged. A zero timestamp is not automatically unknown GPU completion: Apple's MTLDrawable docs state that zero also occurs for skipped frames; however no skip-completion inference is admitted here.

Authoritative references: https://developer.apple.com/documentation/metal/mtldrawable/presentedtime ; https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:) ; https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/SettingUpLayerObjects/SettingUpLayerObjects.html .

Layer drawableSize specifies texture dimensions, while layer frame/bounds determine presentation geometry. A zero-sized host layer is a candidate explanation requiring actual diagnostics, not a confirmed cause. Occluded/not-visible WindowServer session and virtual drawable presentation reporting also remain hypotheses. Actual native rerun required. No deadline widening, presentation-time bypass or command-completion substitution.
