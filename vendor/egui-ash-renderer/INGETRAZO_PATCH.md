# Local pinned allocator/texture corrections

Origin: egui-ash-renderer 0.7.0 crates.io source, MIT license retained. No egui or ash version upgrade.

The default allocator now accepts a MemoryObserver through Renderer::with_memory_observer. Actual Vk memory requirements and selected heap are reserved before allocation, committed after binding, canceled on failure, and released after actual Vulkan free. Buffer/image/bind failure paths clean up resources. Texture construction, descriptor creation and first index allocation clean up earlier allocations on failure.

ImageDelta TextureOptions now control sampler minification/magnification and wrap mode, including options changes on partial updates. Partial updates retain prior texture contents by transitioning from the actual shader-read layout, rather than UNDEFINED. Transfer extent/payload checks precede staging allocation. Existing synchronous upload method remains: bounded/chunked upload is a subsequent gate.

The fork is required because upstream allocator types are private and do not expose admission/accounting callbacks. The unchanged parts remain upstream licensed source; shader SPIR-V is the pinned package artifact.
