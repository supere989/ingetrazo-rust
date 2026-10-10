Source curve flattening acceptance

Production kernel f311b06 adds bounded QBezier::addToPolygon source arithmetic (threshold0.25 in physical space, source depth9), before26.6 quantization and existinggraycells. Six actualQPainter path controls now whole-image exact: ellipse, negative ellipse, rounded rectangle, horizontal/diagonal/reversed round-cap stroke outlines. The direct gray_render_cubic draft failed allsix and is retained. QOutlineMapper::curveTo converts cubics to source floating subdivisions before fixed coordinates; this explains the corrected stage.

This helper supplies actual authored Qt path elements to a native diagnostic; native production shape construction/stroker integration remains required. It is not a production whole-shape or hardware acceptance claim. Sourcecurve stroke offsets, cache/performance, transformed curves and sourceviewport clipping remain gates. Three kernel tests pass including finite/admission controls.

Sources: Qt6.12.0 src/gui/painting/qbezier.cpp and qoutlinemapper.cpp. QBezier adaptation GPL-3.0-only under repositoryLICENSE; FreeType graycells FTL under licenses/FTL.txt.
