# Native Shaded texture means

The Shaded-with-textures-disabled mode preserves source texture run geometry and order. A separate sampler-free fragment pipeline takes mean RGB and shade as separate float uniforms, matching pinned Viewport._texture_avg_color and the shaded run loop. No sampled GPU image, sampler, or surface image descriptor is created for a mean draw. Reference image planes retain their independent sampled path.

Cold immutable assets are hashed by the existing asset identity path and decoded by the scoped source decoder. The sixteen source-equivalent 4×4 pixels are averaged in f64 and narrowed once for the color uniform. The decoded image is discarded after computing the mean. Unchanged revision/mode frames traverse neither assets nor means and perform no decode, hash, or geometry/image upload.

Mean candidates and their active identity set are admitted before GPU publication. Candidate discard preserves the previous mean map. Publication capacity is reserved during preparation; mean publication is infallible and follows existing GPU resource publication. Existing sampled/reference publication and resource retirement error paths are not newly guaranteed to roll back every partial GPU-cache mutation; that broader transaction/lifetime gate remains required.

World.configure_texture_average accepts configurable decoder limits and a peak entry count. The entry policy counts simultaneously retained old plus newly prepared distinct means, rather than claiming an active-only or exact allocator byte budget. Defaults match the scoped decoder and admit 65536 peak entries; large/streamed source texture scope remains required. External average-file reads are bounded by the actual configured encoded cap plus one byte. Sampled image decoder policy remains unchanged.

Stats report average_decodes, average_hits and average_entries through World.texture_stats. These count cold CPU work and mean entries, not Vulkan allocations. Pixel, style switching, malformed replacement preservation and actual GPU acceptance must be retained separately; this source stage does not close real-model rendering parity. Additional Qt image formats, invalid/missing image fallback policy, native ARM inverse-alpha equivalence, and annotations remain open.


## Publication preflight

World cold replacement now prepares contour descriptors without touching the published pool. Texture candidate preflight validates cached identities, reserves the sampled-image map and active identity set, and validates all old/new buffer IDs and ledger allocations. Retirement lists must be disjoint from candidate IDs; old buffers must have no use beyond the supplied completed serial. The final Vulkan fence status is checked before descriptor publication. A private proof carries a distinct Resources-instance owner, not merely a shared device/lifetime.

After successful preflight, contour/reference/texture descriptor swaps, completed buffer destruction, cache eviction and mean publication contain no recoverable allocation, retirement or fence-query failure. The renderer still requires the caller's fence/serial to describe the same completed draw; unknown lifetime completion continues to quarantine resources rather than free them. Warm reference replacements use equivalent admission, and all reference memberships are checked before any warm pose update. Warm section caps are prepared and preflighted before reference publication.

An optional sampled cache peak-entry policy, default unlimited, can refuse a genuine prepared image candidate before publication. Candidate work counters may rise on refusal; published sampled_entries, mean entries, old geometry and pixels must remain unchanged. Actual GPU refusal/retry and normal style roundtrip proof are required separately. The earlier partial-publication boundary and retained failure evidence are not erased.
