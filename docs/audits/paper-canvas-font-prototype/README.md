# Experimental native canvas font painter — not parity acceptance

The first actual original Qt 6.12 TextItem/DeviceCoordinateCache comparison fails all six declared text regions: 611/323, 620/323, and 603/323 differing pixels. The frozen report retains the first binary SHA. This historical binary predates subsequent resource-budget and atomic item publication hardening; it is not represented as the exact executable of this commit.

The prototype consumes existing typed PDF layout/shaping and shares bounded font resources; bitmap textures are cached by glyph/device transform and text items publish paint operations atomically. Oversize/invalid transform/index refusal preserves shared shaping/outline state (one real-font regression passed). This is an experimental stage, not ready for canonical adoption. Canvas layout/background/underline/fallback and platform fidelity remain open.

Primary Qt 6.12 source audit: qfontengine_ft.cpp loadGlyphFor sets HintNone for scaled nonrotation glyph transforms. The prototype currently retains the pixel100 hinted policy while transforming; a separate correction will be tested without changing declared pixel regions or tolerance. Original runtime remains pinned at 6be29fe437a7cd992d4e079f71720821b25216b9.
