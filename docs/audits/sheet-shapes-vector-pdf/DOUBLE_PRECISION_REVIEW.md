# Source logical-device vector transform correction

The direct PDF writer now preserves physical path coordinates as doubles, keeps each shape path local, and expresses the source300DPI logical-device frame as PDF transforms. The source default margin still rounds42device pixels to10.08points. No raster image, extra rendering process or fallback painter is introduced. Source shape fill/clip paths retain odd-even rules; independent direct API callers can choose nonzero.

All nine whole pages match the actual frozen original atlas exactly in independent QtPdf/PDFium at72,144and300DPI (27whole-page comparisons). Two actual native PDF mutations are rejected: a one-millimetre shape shift and an eight-page atlas. The source initial margin and144DPI failures remain retained in their original directories. Intermediate local-only transforms fixed curves but exposed polygon/arrow differences; preserving the source logical-device frame resolves them without fitted geometry or tolerance.

All64formats tests pass. Additional source sheet painters, full QColor vector precision, UI export dialogs, PNG/JPEG and complete mixed-paper/failed-frame atlas acceptance remain open. These nine literal fixtures do not establish the whole P29 feature.
