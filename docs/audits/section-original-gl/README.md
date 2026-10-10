Original section stencil boundary

Run scripts/check-original-section-gl.py inside the devbox with the pinned Python venv. Requires Xvfb, Mesa GL and Qt xcb dependencies (libxcb-cursor0, libxcb-icccm4, libxcb-keysyms1, libxcb-shape0). The original closed control fills, but the same shape retaining its classified interior slab does not. Excluding that slab in Rust changes original rendering behavior and must be recorded as an intentional correction. Transparency suppresses fill; duplicated opaque shell still fills. Disposable display/config/cache isolate the original user installation.

The reversed-slab control reverses Face.loop, the authoritative vertex objects. Face.vertices returns a temporary position list; reversing that list does not alter the face. Correct reversal restores the same cap coverage as the closed control (20,164 pixels).
