# Native Canvas synthetic bold and ARGB coverage

The production native glyph bitmap uses CoreGraphics 32Big + PremultipliedFirst: bytes are ARGB. Decode grayscale from RGB indices1/2/3; prior decoder included opaque alpha and omitted blue, reducing opaque black coverage to168. Four extracted-production scalar controls pass and reject that historical value; temporary binary/source were purged after hashes were retained. This is not native macOS pixel proof.

Native synthetic bold follows Qt6.12 imageForGlyph second RIGHT shifted draw (half quantized underline thickness times transform scale). PDF left-offset policy and0.3 italic policy remain unchanged. Glyph cache includes requested bold. Source boundingBox and alphaMapBoundingBox do not expand physical bounds for synthetic bold: preserve source +3 width padding, including possible clipping at large scale. Fixture observes0.25/1/4/32 zoom edge occupancy; existing4Mi-pixel budget refusal remains explicit at32 if exceeded. It does not enlarge bounds to hide source behavior.

The new native_paper_font_tiles example observes actual production CanvasPainter, resolver, layout, raster, cached tile and encoded white composition. It requires six literal cases and three unchanged warm frames, distinct Times bold cache/pixels and invalid glyph/scale rejection. It is not GPU upload/WSI/desktop/physical-input acceptance. Native SDK/runtime replay remains REQUIRED. ARM presentation deadline remains separate and strict; Intel prior presentation passed.

Coordinated macOS commands (root-owned single cloud batch):

    cargo run -p ingetrazo-gui --example native_paper_font_tiles -- /tmp/native-font-tiles
    python scripts/check-macos-paper-font-tiles.py /tmp/original-macos-fonts /tmp/native-font-tiles /tmp/native-font-tiles-comparison.json

Use actual existing original six-case captures; checker requires pinned original source/Qt, PNG hashes, literal inventory, exact800x300 RGBA equality and blank/one-pixel negative controls. Partial refusal diagnostics survive but overall six-case failure is mandatory. Source/script syntax checked locally; Rust probe parsed by rustfmt. No local native SDK or pixel acceptance claim, no broad suite rerun.
