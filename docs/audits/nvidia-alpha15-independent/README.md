# Independent same-GPU source opacity acceptance

Original frozen commit `6be29fe437a7cd992d4e079f71720821b25216b9`; Python3.13.13/PySide6.12.0. Fresh actual original viewport ran on NVIDIA GeForce RTX5090/PCIe/SSE2, NVIDIA GL3.3 driver615.71.09. Each source framebuffer is captured from its own newly created proof viewport.

Native source includes f96ec35 opacity f64 threshold/source-span preprocessing plus independent bounded upload_many integration. Build checkout c8e230d, locked debug build; exact executable SHA256 `a332e0695e5a196cc3fbde5635d01ad60d5c1b563f5fc63fc4ae9d3cd7c0fd6f`. Strict NVIDIA discrete production World GPU offscreen probe: all15 exact RGBA, opacity bucket ordering, one geometry upload, shared24-byte position/RGB vertices and zero duplicate XYZ. Mandatory cases include10base+4rounding+f64 opacity threshold. The stale opaque and incorrect linear blend controls remain rejected.

Both runs use immutable dev-base image `a6597584287dfce8099218b3231240c1a939d73a573eeb260933f0d2b9b7070c`, readonly CDI gpu0, no network,128MiB GPU admission and externally bounded runs. Source Xwayland uses readonly own authorized display/socket/auth; auth contents are not recorded. Native headless selects explicit NVIDIA EGL Vulkan ICD and reports driver/API/adapter in retained log.

Earlier Mesa-source versus NVIDIA-native seven one-byte mismatches remain preserved in prior audits. Same hardware source/native matches; no byte tolerance or shader compensation is introduced.

Actual native swapchain/encoded egui composition is **OPEN**. The fixture SHA256 is `6b0ad9aa063ad0c72f3dbd92f4772eb2343b2863fbb3284525b45c427ea944d6`. A new owned client appeared, but own-client capture remained unavailable and adapter diagnostics did not appear before bounded timeout; retained failure is not a passed window proof. Capture examines only the exact newly created named client, never a desktop/root image. Linux NVIDIA headless proof does not establish Windows/macOS hardware, native Metal, general material/texture rendering, or all transparency semantics.
