# Independent actual NVIDIA backpaint acceptance

Fresh original frozen Qt viewport22cases on actual NVIDIA RTX5090/GL3.3 driver615.71.09 match actual native Vulkan offscreen production World **22/22 exactRGBA** and actual native Vulkan/encoded-egui/swapchain owned-client captures **22/22 exactRGBA**. Source and native ordering, ordinary/front-cull/opaque-back counts, shared24B vertices and no duplicated XYZ are checked literally. All own clients close cleanly; no desktop/root images.

Combined963993c source is c5da026 on f96 plus accepted bounded upload_many. Headless uses explicit EGL ICD; actual own-window presentation uses GLX ICD. Immutable dev-base image and readonly CDIgpu0/network-none mounts as preceding15-case proof; source/NVIDIA metadata and input SHA are retained. Probe binaries are independently freshly locked built in validator target; hashes in provenance.

Checker now requires exact22 orderedcase names for source/native/oldbaseline, prevents omitted/duplicated corpus acceptance and compares gateRGBA to actualsource instead of Mesa247 literal. The old owner negative corpus had18cases and is rejected by this mandatory guard. The retained old pre-backpaint binary was freshly executed against all22 same actualNVIDIA inputs to preserve a complete failurebaseline. Explicit missing/duplicate/closedgate controls also fail.

This proves bounded untextured planar front/back opacity/gate/culling/order. Slanted/hole/group geometry controls, texture/back-material broad scope, hardware other than this LinuxNVIDIA device, nativeMetal and complete renderer behavior remain open. Historical opposite-driver quantization failures remain unchanged.
