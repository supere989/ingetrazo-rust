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

## Partial diagnostics and mandatory negatives

The checker retains one result for each of the six required names even when native resolution/export refuses. Available pages are still read and compared. Missing/refused cases always keep the overall result false. Each actual source page also passes blank and exactly one decoded-pixel alteration through the identical comparison function; both must refuse. Small actual QImage unit controls passed and are retained; actual macOS PDF/control execution remains pending coordinated native output.

The separate check-macos-paper-font-metrics.py records all six required source/native raw metric and glyph-identity comparisons. It does not compare unkerned raw advances to shaped advances, nor pretend supplementary QTextLayout positions are operative paint_text_mm placement; both remain in diagnostics. It keeps the physical-face and strict whole-page gates separate. Actual native input with the new observations is pending; Python syntax only checked locally.

## Retained exact position counterexample

The second ARM actual source/native output yields four exact supplementary raw-metric/position cases; Times refuses, and CJK's second glyph is6426 versus6400 in26.6 units. The raw per-glyph advance is100px in both; the source shaped layout adds26/64px. Arabic stores glyph arrays in opposite logical/visual order, yet exact glyph-position pairs match. The former raw-order failure remains retained; no renderer claim derives from its array ordering. A one-unit perturbation to the actual Helvetica native advance rejects.

Qt qharfbuzzng.cpp v6.12.0 lines635–648 sets ppem and ptem from the font request. Native paper shaper currently sets ppem but no ptem; AAT tracking is a candidate cause for the CJK mismatch. Do not fix via fixture-specific26/64 adjustment. Source observer now records effective point size and actual device logicalDPI in both QImage and QPdfWriter observations, to determine the operative context before introducing a shaping/cache policy. This diagnostic is not yet native-executed.

## Third ARM operative CJK PDF diagnosis

Frozen third artifact source/native glyph outline coordinates, contour ends and flags all match. Three native hmtx bearings differ from source. Valid bearing-only SFNT mutation leaves62/0/309 changed pixels at72/150/300; additionally replacing actual second-glyph text placement100 with source100.406250 produces zero differences at all3DPIs. Spacing alone leaves5/5/41. These are diagnostic mutations, not a production shaping fix. Notably150DPI cannot observe the remaining spacing discrepancy after bearing correction;72/300 and exact glyph positions remain necessary. Original failures remain intact. Temporary diagnostic PDFs were removed automatically. The generic native metric fix is already on coordinated batch; native HB ptem/trak context remains unverified pending new source point-size observations.
