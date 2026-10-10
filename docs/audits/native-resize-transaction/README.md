# Transactional native Vulkan resize

Source d726bb0 + d713a16, exact revision and binary hashes in proof.json. Isolated worktree, no canonical changes.

Preparation now owns candidates independently: swapchain/views/renderpass/framebuffers, egui pipeline, CAD integer pipeline/input-attachment descriptor pool/sets, per-image semaphores and presentation fences. All old GPU/present uses must acknowledge completion first. The final fallible CAD lock precedes infallible publication; only then are old submitted owners destroyed. No new per-frame hashing/copying/allocation is introduced by this cold resize operation.

[Vulkan swapchain creation specification](https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainCreateInfoKHR.html) retires a non-null oldSwapchain at the actual creation call even on failure. The implementation remembers retirement and retries with NULL rather than illegally reusing a retired handle. It retains old owners/cache metadata on failed candidate preparation, but does not acquire retired old images or promise their display survives (the implementation may free unacquired old images).

Actual RTX5090 owned-client integer-enabled controls cancelled preparation after successful real allocations at eight checkpoints. Positive logs count the actual successfully created/destroyed unpublished handles; published owners/descriptor state and tracked allocation ledger remain unchanged; each retry succeeds and renders. CAD uploads remain one across all retries. Two OS resizes and close end with tracked ledger zero. Two independently compiled owned source variants restore early old-swapchain destruction and early egui pipeline publication; both are rejected by the published-owner invariant. Their diffs and logs are retained. These are explicit engineering cancellation controls, not synthesized Vulkan errors or real driver OOM tests.

Fresh final-source nine whole600x500 shape images match retained original Qt references exactly (zero differing pixels each), five strict lifecycle controls pass (callback-only/error, real pressure, uniform changes, nearest texture/UI restoration). Workspace 523 passed, 2 ignored. Prior original reference images and old-float negatives remain under integer-cad-independent; no tolerance changed.

The generic shader/pipeline cold constructor also releases successfully created shader modules and partial pipeline handles on actual error returns, according to [vkCreateGraphicsPipelines](https://docs.vulkan.org/refpages/latest/refpages/source/vkCreateGraphicsPipelines.html). Real driver creation-error/OOM paths, device-loss recovery, constructor failures outside this swapchain candidate, whole implicit driver allocation accounting and driver-call global latency bounds remain open. No validation-layer claim (layers unavailable). Native Metal macOS remains mandatory and unproved by these Vulkan controls.

Replay: build examples listed in proof.json with unique CARGO_TARGET_DIR. owned-run.py uses exact readonly publish merged libraries/Python/XKB, readonly CDI/host X socket/auth, and only newly owned clients; no desktop-wide capture or user-window interaction. Actual normal shape/lifecycle harness is scripts/run-owned-integer-cad-hardware.py. Checker:

    python scripts/check-native-resize-transaction.py docs/audits/native-resize-transaction/positive --old-cleanup docs/audits/native-resize-transaction/negative-old-cleanup --old-pipeline docs/audits/native-resize-transaction/negative-old-pipeline

Old negative binary execution intentionally fails the fixture invariant and process unwinds before the capture helper completes; the retained helper readiness timeout is secondary to the exact owner-loss assertion in startup.log, not accepted as the negative by itself.
