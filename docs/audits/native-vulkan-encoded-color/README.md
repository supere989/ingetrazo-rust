# Encoded color adapter checkpoint

Production World renders directly into R8G8B8A8_UNORM. The original Qt OpenGL source blends encoded RGB; blending linear RGB and then converting changes translucent appearance. The Vulkan egui adapter marks the opaque CAD target and decodes exactly only when the swapchain is sRGB, avoiding an extra pass, copy or sampling view. Managed UI textures retain their existing path.

Fresh Linux software native window captures passed startup, 800x600 and 1100x700 resize, and OS close for ten section cases. Exact-zero controls and established 5% positive section tolerance pass; Top closed/reversed/duplicate match 20164 pixels exactly. Iso remains bounded, not exact. The 48MiB texture under 8MiB staging headroom retains exact red/green/blue/yellow partial-update controls; prior full-image allocation failure is retained separately. Headless 17MiB readback/depth/cache and zero final ledger pass.

Original alpha helper captures ten actual Qt cases including encoded RGB blend on black, opacity bucket ordering, equal-opacity insertion order, opaque occlusion and clipping. opaque-only-alpha-baseline.json intentionally retains missing translucent surfaces and the incorrect .995 classification. This checkpoint does not implement or claim transparent surface parity. Native Metal, hardware WSI, textured/back-painted surfaces and full rendering remain open.
