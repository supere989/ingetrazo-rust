Widget-only LCD candidate

Source: Qt qtbase v6.12.0 src/gui/text/freetype/qfontengine_ft.cpp, original license header retained. Source OpenGL engine uses A32 horizontal RGB mask, default FT LCD filter, and CONSTANT_COLOR/ONE_MINUS_SRC_COLOR solid brush blending on alpha-less desktop surface. The prior actual neutral widget diagnostic establishes per-channel coverage; it does not yet prove this candidate's masks or blend.

The optional vendored FreeType source-lcd feature enables its existing filtered LCD rasterizer; default grayscale calls retain FT_RENDER_MODE_NORMAL. Initial bundled build had Harmony-only LCD and refused the required filter; retained failed control. No fallback to an approximate mask. Candidate admission/cache keys distinguish LCD from gray; RGB equal-coverage runs are coalesced with one ordering range per glyph. Shader remains in the existing annotation renderpass.

Focused provider/cache control passes; actual widget GPU comparison remains required. Windows/native ARM provider policy and non-RGB subpixel orders remain open. Source-only original samples0 during overlay paint must not be conflated with World MSAA depth/color.

Shader compilation: shaderc 0.8.3 pinned Cargo.lock; ingetrazo-image-shader-compiler INPUT.frag OUTPUT.frag.spv, both absolute paths under this worktree.
