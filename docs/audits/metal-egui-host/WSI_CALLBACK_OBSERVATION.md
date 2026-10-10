# Actual drawable presentation callback observation

Native observer CI refuted zero layer geometry: both devices reported visible, non-occluded 800×600 view/layer bounds. Actual GPU commands completed. ARM timestamps remained zero; Intel first drawable timestamp zero while second was positive. The FIFO zero-head policy prevents retiring that later positive drawable. These failures remain authoritative; layer sizing candidate is not an attributed fix.

This diagnostic registers MTLDrawable.addPresentedHandler before presentation. Callback-owned atomic fields record actual invocation and the callback argument's actual presentedTime. The escaping block is retained by each Frame together with complete resource/device/window ownership. No arbitrary thread uses AppKit; callbacks only inspect the provided drawable and atomics. No retirement/deadline change is made.

Official Apple documentation says the handler runs immediately after a drawable is presented, and presentedTime is zero when a frame is unpresented or skipped. This stage does not presume skipped notifications occur or infer presentation completion from command completion. Fresh actual ARM/Intel callback observations are required before a skip-aware policy. Positive timestamp remains the unchanged displayed-frame acceptance.

https://developer.apple.com/documentation/metal/mtldrawable/addpresentedhandler(_:) ; https://developer.apple.com/documentation/metal/mtldrawable/presentedtime .
