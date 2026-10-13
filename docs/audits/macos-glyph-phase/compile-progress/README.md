# Glyph-phase diagnostic compile checkpoint

At `e72a077a`, ARM/Intel Rust metadata checks passed in 4.45/4.37 seconds. The existing text-canvas compiled test binary then passed all nine matching tests in 0.37 seconds (352 filtered). The first two Cargo filters matched zero tests; their logs are retained and supply no test acceptance. The actual module filter is `composer::text_canvas::`; the final run reused the first 11.14-second build directly.

This verifies compilation and shared device-position, clipping, ordered-composition and cache controls. It does not execute CoreText on macOS, observe phase masks on a Mac, establish image parity or prove repaired Metal host capture. The native refusal diagnostic is included in the compiled library. The archive binds five actual logs/raw origins and eight executed Git files; frozen Python and reference captures remain unchanged.
