# Exact Metal canvas composition foundation

`cad_compute_painter` prepares immutable original source span plans and Qt byte palettes, then encodes source-ordered integer SourceOver into an existing private RGBA8Unorm canvas. Tier2 read/write texture support is required before preparation. The caller must supply ShaderRead|ShaderWrite usage, a bounded target/clip, and retain Prepared through GPU completion. This module does not submit, poll, sleep, read back, or copy images.

Cold plans upload bounded source rows, sparse tiles and references once. Warm unchanged geometry/layout and palette retain buffers without plan generation or allocation. Palette changes preserve the geometry plan when batch first/count layout is unchanged. Cache publication follows successful validation, every budgeted native allocation, and pipeline creation. Earlier prepared owners retain evicted immutable buffers/palettes.

One 8x8 threadgroup owns each sparse source tile. Each pixel is read once, composed in original batch/index order with separate BYTE_MUL source/destination terms, and written once. Tiles are unique, so invocations do not overlap. Dispatched work is bounded by the sparse source coverage, rather than dispatching the entire canvas for every callback.

This is not yet activated in presentation. The required next stage supplies bounded persistent RGBA canvases, source-ordered generic UI render/compute encoder segmentation and one final BGRA drawable draw. Apple framebuffer fetch remains the shorter inline path. Generic non-span geometry remains required scope. Dual-Mac cross checks validate Rust API compilation only; actual MSL execution, integer pixels, rollback, frame ownership, resize, bytes and latency remain unverified until native probes execute.
