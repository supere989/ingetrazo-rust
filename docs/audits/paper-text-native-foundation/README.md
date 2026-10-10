# Direct shaping foundation, not a complete text painter

This isolated source stage adds immutable PaperFont/shape_run only. No application PDF export text error was removed and no fonts are resolved through subprocesses in application code. Existing paper View/vector API remains untouched. Fonts share an Arc read-only blob and one cached fixedadvance table; font creation parses once, each run owns a temporary HarfBuzz buffer. RAII drops font before backing bytes and every temporary native object; callback data belongs to the HBfont funcs destroy hook. No unsafeSend/Sync. Input font/text/glyph publication inventories are bounded. UTF8byte clusters and single script/direction runs are explicit; paragraph bidi/fallback/wrapping/embedding remain required.

Dependency: official servo harfbuzz-sys =0.8.0, defaultfeaturesOFF, bundledONLY. Bundled actualengine8.4.0 (not latestHarfBuzz). No FreeType/CoreText/DirectWrite feature enabled. ttf-parser =0.25.1 already existed in Cargo.lock and becomes a direct dependency. cc/pkg-config already existed; only one new locked package. License sourcepackageApache2/MIT, HarfBuzz OldMIT. Engineversion/platform compatibility remains bounded by evidence; broader Unicode/script behavior is open.

Seven actual source supplementary Qt glyph runs match numericallyEXACT, including shapedligatures, kerning, bold/italic and Serif/Monospace. Native derives ascent/descent/leading from the font independently; no source baseline is injected. Warm repeated shape matches, Arc bytes reused and released on drop, malformed/empty/oversized fonts and oversized text refuse. A1/64position mutation fails the strict oracle. Font source SHA is verified before testing. These are shaping/helper controls, not original QPainter wholepage or usergesture proof.

## Retained held-out failure

Source expansion adds combiningmarks, kernpair string, Serifbold ligatures and pureArabic/Hebrew runs (19source PDFs total). Combining q+diaeresis currently FAILS: native92.8125baseline versus source supplementaryQt−19.1875. This failure is retained, not excluded to claim19nativepasses. After literal source negativeYscale and fontHExtents callbacks it remains unresolved. The source observer is independently compared against a plain original painter at72/300DPI for all19pages:38complete RGBA images EXACT, so observation did not alter the operative source PDFs.

Qt6.11 primary implementation corroboration (operative observations are6.12):
https://raw.githubusercontent.com/qt/qtbase/v6.11.0/src/gui/text/qharfbuzzng.cpp
https://raw.githubusercontent.com/qt/qtbase/v6.11.0/src/gui/text/freetype/qfontengine_ft.cpp
Source has glyphExtents and contourPoint callbacks as well as advances/metrics. Contour points use FToutline coordinates directly; GPOSanchorFormat2 can differ from a defaultOpenType font with no contour callback. Exact native contour/extents support requires additional source-backed work; positions must not be adjusted by test case. The current foundation is reviewable progress, not ready to claim full font compatibility or text rendering. Source QtfontBBox/contour/paragraph semantics, exactPDF font embedding, title/default raster/UI remainOPEN.
