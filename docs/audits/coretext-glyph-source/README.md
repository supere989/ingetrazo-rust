# Native CoreText glyph backend: SDK review stage

Source33d9154d, isolated from review/metal-raw-canvas14efd891. Original source pin6be29fe437a7cd992d4e079f71720821b25216b9 and Qt6.12.0 remain authority. Apple hvgl documentation: https://developer.apple.com/fonts/TrueType-Reference-Manual/RM06/Chap6hvgl.html . Native CoreText interprets hierarchical variation outlines; application does not implement a partial hvgl parser, raise the16MiB cap, copy the56MiB collection, or invoke a renderer process.

## Source mapping

Qt qcoretextfontdatabase populateFamily/getFontDescription/fallbacksForFamily registers descriptors by family/native-normalized weight/style, uses native language cascade and registers private cascade descriptors. qfontdatabase.cpp bestStyle uses weight distance divided10 and style penalty0x1000, first minimum; equivalent registered slots replace their handle. This stage ports this mechanism; Mac execution must verify actual descriptor order/identity and broader fallback/script ordering. No names from the six-case fixture are embedded in matching policy; generic Sans Serif maps to platform Helvetica as existing provider did.

Qt qfontengine_coretext initializeHeightMetrics/capHeight/recalcAdvances use CoreText100pixel QFixed metrics. imageForGlyph/alphaMapBoundingBox use padded metrics, CoreGraphics text matrix/position and grayscale coverage. Native code has bounded raw/converted-metric admission before integer casts. Exact CTFontDrawGlyphs transform, RGB byte ordering, smoothing/gamma/appearance and source synthetic traits remain actual pixel gates; implementation is not proof.

CoreText glyph paths feed existing source-derived cubic-to-quadratic PDF subset conversion; direct vector resources and ToUnicode remain existing APIs. This does not substitute outline paths for the production Text painter. CoreText native instance advances override bounded HarfBuzz metric callbacks. Metadata/shaping extraction deliberately excludes outline/collection data. Variable-dependent HarfBuzz feature/positioning tables and contour callbacks still require broader source controls.

## Ownership and identity

One Rc retained CF font per CachedFont; Create/Copy results scoped.17 bounded metadata/shaping tables share one Arc SFNT with shaper/subsetter. Existing16font/32MiBfont-byte,64request FIFO,4096glyph/256Ki-point and8subset/2MiBsubset caches remain. Native variation axes (at most64), effective symbolic traits and font matrix participate in identity, alongside physical path/index/provider epoch. Extracted SFNT index0 differs deliberately from physical TTC index. Probe cold request eviction requires same physical index, same normalized key and same held owner. Outstanding external owners retain resources; aggregate caller retention/OS internal caches are not claimed bounded by application FIFO.

Raster admission limits transform, dimensions and4MiB coverage pixels; transient RGBA and coverage owners are explicit. Color fonts refuse; they remain required scope. Cache resource creation/publication reserves before eviction. Cache refresh changes namespace and drops only cache ownership.

## Focused local checks

Inside ingetrazo-publish, reduced debug0/incremental0/-j2 target:

- cargo check -j2 -p ingetrazo-formats --lib: PASS.
- cargo test -j2 -p ingetrazo-formats --lib paper_font_sfnt::tests:2PASS. Valid collection identity ignores declared53,903,096byte hvgl table, reading less128bytes; oversized/malformed name table refusals.
- cargo test -j2 -p ingetrazo-formats --lib device_glyph_refusal_restores_shared_font_geometry:1PASS existing Linux path.
- Full native Rust branches including paper_coretext_provider_fixture compile in retained host metadata harness; only temporary cfg selector/framework declaration substitution, original source restored before commit. This catches Rust integration types, not Apple SDK ABI.
- rustc --edition2024 --target aarch64-apple-darwin --emit=metadata with retained ffi-check.rs:PASS. Stub SFNT adapter means it proves native FFI module Rust typechecking only, not full target/runtime.

No broad suite or cloud run started by this worker. Actual root-coordinated ARM/Intel SDK/runtime CI is mandatory before adoption. Native fixture emits chosen face, physical/extracted indices, glyph metrics/coverage and PDF subset/warm/refusal observations. Existing check-macos-paper-font-matches.py is unchanged: its requested-physical-style assumption must be reconciled against actual original synthesis independently before any change. Retained old Times and CJK failures remain authoritative. Full Canvas/PDF font parity, platform raster/layout/fallback/script policy and full paper workflows remain OPEN.
