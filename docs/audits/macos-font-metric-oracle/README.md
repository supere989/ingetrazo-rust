# Native macOS source glyph observations

The original painter probe now records the operative drawText rectangle, flags, world transform, effective font size and QFontMetricsF metrics. Supplementary QTextLayout runs record native raw-font metrics, per-glyph advances and outline/bounding rectangles. Font inventory records physical OS/2 and head traits separately from requested/effective Qt traits.

This is instrumentation only. The original paint_text_mm call and pixels are unchanged; the strict six-case physical-face checker is unchanged. No Linux output is presented as macOS evidence. Python syntax compilation passed; actual ARM/Intel execution is pending coordinated CI. These fields distinguish source synthesized traits from stored face flags without accepting arbitrary requested-style mismatches.

Retained ARM first-native source 6c4d38cd still fails Times resource acquisition and PingFangSC-Regular PostScript-to-physical-collection lookup. Native glyph raster, layout and PDF pixels remain required comparisons; identity alone does not prove them.
