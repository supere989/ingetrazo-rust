# Vulkan resource ownership admission

Frozen source 3f4061fe builds and executes the named ownership probe. Two independent real llvmpipe Vulkan contexts exercise valid small buffer and RGBA image transfers, four early foreign-owner refusals, unchanged allocation/transfer counters and charged bytes after refusals, valid byte preservation, and final zero ledgers. The 30-second external limit bounds the engineering probe; production transfer waits remain unchanged.

Source inspection found image allocation before context admission, missing read_chunks context identity, and missing TextureImage lifetime admission. Shared admission now checks Arc lifetime and VkDevice identity before those allocations or handle use. Existing upload paths reuse the same check. The private raw image upload also admits its context.

No old implementation was executed with foreign Vulkan handles: that would violate the API contract and risk driver behavior. The retained initial build failure was missing the original-source worktree symlink; fixing the isolated symlink allowed the focused build. No broad suite or repeated GPU screenshots were run. Software execution does not establish hardware rendering parity. Raw VkImage readback still relies on the caller's raw-image ownership contract; it now admits the context but cannot infer ownership from a raw handle.
