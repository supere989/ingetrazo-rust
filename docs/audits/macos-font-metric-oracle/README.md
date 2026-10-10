# Native macOS source glyph observations

The original painter probe now records the operative drawText rectangle, flags, world transform, effective font size and QFontMetricsF metrics. Supplementary QTextLayout runs record native raw-font metrics, per-glyph advances and outline/bounding rectangles. Font inventory records physical OS/2 and head traits separately from requested/effective Qt traits.

This is instrumentation only. The original paint_text_mm call and pixels are unchanged; the strict six-case physical-face checker is unchanged. No Linux output is presented as macOS evidence. Python syntax compilation passed; actual ARM/Intel execution is pending coordinated CI. These fields distinguish source synthesized traits from stored face flags without accepting arbitrary requested-style mismatches.

Retained ARM first-native source 6c4d38cd still fails Times resource acquisition and PingFangSC-Regular PostScript-to-physical-collection lookup. Native glyph raster, layout and PDF pixels remain required comparisons; identity alone does not prove them.

## Actual PDF comparison stage

The same frozen original paint_text_mm recipe now also paints through actual QPdfWriter at300DPI on A4. paper_coretext_pdf_fixture uses production paper_text_painter::Painter and the direct writer with the matching source physical origin, then verifies unchanged warm layout counters and repeated exact PDF bytes. check-macos-paper-font-pages.py requires the literal six-case corpus and reads both full pages using the same actual Qt PDF reader; run at72,150 and300DPI with zero changed-pixel allowance. No glyph-placement values are copied from original into the native painter. Native refusals remain failures and cannot silently remove a case.

This stage is prepared but not yet native-SDK executed. It is not Canvas GPU capture: actual whole-item Canvas remains a separate required gate. No cloud launched by this lane.

Coordinated commands:
```
cargo run -j2 -p ingetrazo-formats --example paper_coretext_pdf_fixture -- native-font-pages
python scripts/check-macos-paper-font-pages.py original-fonts native-font-pages 72
python scripts/check-macos-paper-font-pages.py original-fonts native-font-pages 150
python scripts/check-macos-paper-font-pages.py original-fonts native-font-pages 300
```
