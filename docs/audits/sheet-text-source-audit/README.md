# Source font/layout dependency audit (native text OPEN)

Fourteen actual unchanged original views.composer.paint_text_mm/_draw_text_mm PDF helpers were executed with a forwarding QPainter observer. It records the actual source drawText rectangle, flags, font, painter transform and metrics; super.drawText renders the PDF. No original painter is replaced. Source pin6be29fe437a7cd992d4e079f71720821b25216b9, Qt6.12.0, Python3.13.13. This is direct operative painter evidence, not production ComposerWindow UI proof or native text acceptance.

Cases cover Latin fi/ffi kerning, accents, bold/italic/underline, wrap/newline, center/right alignment, Serif/Monospace, Arabic/Hebrew and missing CJK glyphs. Actual source generic Sans Serif resolves DejaVu Sans Book pixel100. Font paths/SHA256 are recorded; actual QRawFont head bytes are asserted equal to the resolved file head table. Font files are not redistributed. CJK missing glyphs remain the source baseline, not a fabricated native fallback pass.

Supplementary QTextLayout glyph runs exercise actual Qt shaping with the source font/rectangle, but are explicitly distinguished from QPainter-internal actual placement. They reveal ligatures and1/64pixel metrics (ascent92.8125/descent23.578125). Raw Unicode cmap IDs are not the shaped glyph inventory.

Engineering-only audit-paper-font-shaping.py calls existing system HarfBuzz10.2 through ctypes, preserving actual source expectations. Default scaled font advances fail6/7 selected Latin/style runs. Source-like per-glyph hmtx advance truncation at6400/upem yields all7exact glyphIDs+positions, including kern and ligature behavior. This is a dependency experiment against supplementary Qt runs, not Rust application or wholepage pixel parity. The experiment is not imported by production code and adds no runtime subprocess.

Primary implementation references reviewed:
- Qt qharfbuzzng.cpp font-function adapter: https://raw.githubusercontent.com/qt/qtbase/v6.11.0/src/gui/text/qharfbuzzng.cpp (6.11 corroboration; operative observations above are6.12).
- HarfBuzz customfont callbacks: https://harfbuzz.github.io/fonts-and-faces-custom-functions.html .
- Current HarfRust documented nofontsize/alwaysUnitsPerEm boundary: https://github.com/harfbuzz/harfrust .
- Candidate official servo harfbuzz-sys0.8 bundled dependency: https://github.com/servo/rust-harfbuzz/blob/master/harfbuzz-sys/Cargo.toml . No dependency or native text implementation added in this audit commit.

Remaining implementation needs direct cached font resolution/identity, fixedadvance shaping, bidi/script segmentation, Qt wrapping/alignment/metrics, source PDF font embedding and strict wholepage controls/negative malformedfont admission. A bare glyph-outline approximation is not presumed equivalent to source embedded glyph rendering. Text/title/default raster/fullP29 remainOPEN.
