# Physical sheet symbols and title painters

Runtime `d9a15ec4` is one consolidated net commit on `4b101764`. Twelve actual pinned original ComposerWindow image/PDF sessions were captured once: grid, banded, minimal, rounded and chamfered double title outlines, wrapped two-column rows/dynamic fields, north0/37 degrees, ordinary/fractional scale bars, overlap and hidden title. The exact source archive is authenticated by `source-manifest.json`. Original source code remains unchanged.

The actual Rust App exports through shared bounded Draw producers, cached native Vulkan/Metal paper resources and atomic writers. Source finite-box text fit/word-wrap, i18n captions and six-significant-digit numbers are retained. Source local cubic flattening precedes affine rotation; PDF local coordinates retain their rotated matrix. Rotated glyphs use native font raster transforms, not transformed bitmaps. Unsupported generic raster rotations and remaining painters explicitly refuse. Existing eleven-image/codec/modal/tiling corpus passed once after the affected change.

**Partial strict result:** actual NVIDIA RTX5090 and llvmpipe both yield11/12 exact PNG images and12/12 exact PDF pages. The chamfered double title retains exactly2 pixels/maxchannel1; the strict checker exits1. The source/native heavy stroke directed-edge inventory is exact after literal rooted QLineF arithmetic; that does not explain the remaining clipping/coverage/composition discrepancy. No pixel normalization or tolerance is applied. Nine independent corruption controls reject missing/empty/truncated/duplicate rosters, wrong types/flags, a single pixel on separately exact title-grid, and an empty PDF.

Final affected cached build6.99s. Hardware transport12.191s, with immutable privateCDI and actual selectedRTX5090. Renderer cleanup drains its ledger; export preserves model state and malformed design refusal preserves existing destinations. Source/native/failure records, first callback/constructor errors, setup failures and superseded candidate history are retained in `records.zip`; `manifest.json` authenticates every member. Candidate binary bytes were not all preserved, so their logs/output are historical observations; current three binary hashes and final source bindings are in final-receipt.json.

Replay (no source recapture):

```sh
sheet_symbols_workflows SOURCE OUT --software
python scripts/check-sheet-symbols.py SOURCE OUT OUT/check.json
```

Omit `--software` for strictNVIDIA hardware admission. Expected checker exit is1 until the retained two-pixel failure is repaired. The shared Linux job should place this strict step after existing accepted checks. No fullP29 promotion, no Mac font/pixel claim and no Windows execution claim. Mac default font identity is independently owned by root. Remaining visible image/legend/dimension/profile/label/view-title painters remain separate, explicitly refused.

Source algebra references: [Qt6.12 qt_format_text](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qpainter.cpp), [QTextLine](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/text/qtextlayout.cpp), [QOutlineMapper](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qoutlinemapper.cpp), [QStroker](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/painting/qstroker.cpp), [QLineF](https://github.com/qt/qtbase/blob/v6.12.0/src/corelib/tools/qline.h), [QFontEngineFT](https://github.com/qt/qtbase/blob/v6.12.0/src/gui/text/freetype/qfontengine_ft.cpp).
