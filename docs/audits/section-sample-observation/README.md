# Closed section per-sample observation

The retained section PNG is Mesa/Xvfb evidence, not NVIDIA evidence. The first private-CDI configured capture is retained separately and stopped at the image guard before renderer identification. It produced no sample evidence.

The canonical Mesa/Xvfb run reproduced the entire retained closed PNG exactly. Its owned color attachment was a four-sample RGBA8 renderbuffer. The four top-origin pixels are (400,228), (328,300), (471,300), and (400,371). Their cap samples occupy slots 0, 1, 2, and 3 respectively; all other samples have identical RGBA8 (33,43,59,255). Resolved red is 89,89,88,88. Float bits are retained. This isolates the source difference after sample color/coverage; it does not establish Metal's sample values or a production resolve fix. The 134 non-rounding iso residuals remain open.

`native_metal_section_samples OUTPUT` observes these same pixels through the real prepared World renderer. A cold-only opt-in retains MSAA data for a bounded compute read, without changing normal texture usage or resolve. The command, Prepared frame, sample buffer and compute pipeline follow the existing completion/quarantine lifetime. A single ARM metadata check passed in 7.41 seconds; actual SDK, MSL and GPU execution are pending root-reviewed publication.

Run the source script under the exact isolated Xvfb environment from `scripts/check-original-section-gl.py`, replacing its invoked script with `scripts/reference-section-samples.py` and using a fresh output path. Native observation is diagnostic only, with no changed acceptance tolerance.
