# CoreText PDF subset native bearings

Retained ARM third PDFs have identical Helvetica Regular/Oblique glyph coordinates, contour ends, point flags, advances, and PDF text placement. C bearings differ: native89/source90 and native157/source158. Bearing-only mutations reproduce all six nonzero baseline counts then produce zero changed pixels at72/150/300DPI. Actual PDF reader diagnostic uses valid same-length SFNT/checksum replacement. This is not a new production Mac binary proof.

Production borrows native bounding rect origin; QFixed truncation at100px and again at actual native em precedes subset2048-unit rounding. Advances follow the same source sequence. Outlines, generic FreeType, PDF0.3 shear and Canvas14degree shear unchanged. Compact metadata parse occurs only on cold glyph load; existing bounded glyph cache provides warm reuse. No full collection copy/process/new warm allocation.

Source mapping and hashes in source.json: CoreText boundingBox384–399, getUnscaledGlyph980–999; QFontSubset1156–1166 and generateGlyph907–918. Qt qRound implementations use half-away-from-zero (sign0.5 addition then truncation), matching Rust round; extracted scalar checks include six signedhalf values, negative bearing and1000em. Initial scalar incorrectly used outline-control minimum89 as native CGRect minimum; that failed. Correct native CGRect/source path bound is90units (4.39453125px), distinct from offcurve point89. No production formula changed to accommodate that bad test assumption.

Focused scalar passes; actual production macOS SDK/raster/PDF acceptance still needed on coordinated batch. CJK tracking and broader synthesis/raster/platform/fullP29 remain OPEN. Replay probe.py with retained ZIP/report arguments; temporary PDFs/source/testbinary automatically removed.
