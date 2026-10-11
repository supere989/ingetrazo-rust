# Actual original GPU mip diagnosis

`reference-native-texture-mips.py INPUT OUTPUT` calls the pinned original `Viewport._get_texture` on the authorized NVIDIA OpenGL context and reads every actual mip. The native example uploads via the production Vulkan cache decoder and `TextureImage::rgba8`, then reads every mip on strict NVIDIA hardware. Both use mirrored source rows.

All four original asset level-zero images match exactly. Even reductions preceding an odd extent also match. Odd reductions differ: the 150×151 glass asset reaches a 72-channel-level difference. GPU ledger returns to zero. The complete strict comparison is retained, with no tolerance changes.

Ramp and channel-separated impulse captures establish that a simple Vulkan linear blit is not the source odd-size reduction. Three-tap area filtering approximates the source but is not byte exact. The 150×151 first reduction also shifts the even axis; that shift does not generalize to the later 174×143 reduction of the 1392×1144 asset. This remains a diagnosis; no candidate mip algorithm is enabled. Do not substitute fitted offsets or claim this alone explains all model-frame errors.

`assets.zip`, `ramps.zip` and `kernels.zip` retain inputs, actual source raw levels, manifests and native asset levels. GPU execution was terminal before archival. The source-specific shader path and rounding rule remain the next implementation task.
