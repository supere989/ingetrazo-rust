Source round-cap production acceptance

Production3773184 constructs the literal source cubic round-cap outline, flattens localQBezier geometry at0.25/physicalDensity before applying thetransform (QOutlineMapper::curveTo ordering), then26.6 graycells. Transforming controls first produced14–23 remainingpixels atcaps and is retained. Correct ordering matches six of nine actual production softwareVulkan shape images exactly: rect,polygon-clamped,line-inverted,arrow,terrain-ticks,terrain-band. Roundedrect1976,ellipse1932,clippedterrainhatch2143 pixels remain strictFAIL.

Five GUI cache/geometry/admission tests and three graykernel tests pass. Warm identical origin reuses immutable geometry; curve integer translation currently conservatively retains fullphysicalorigin and can retessellate, so optimaltranslatedcache/performance notclaimed. All spans use one existingGPUdirectCADpass, no imageupload/pass. Coldscanconversion CPUcost/Metal/hardware newcoverage remain acceptancegates. Originalsource frozen unchanged; ninecasechecker exits1 until remainingcases exact.

Actualprivatebinary SHA256d160a35aeea1e840bdf940aa86e43af8124a54778542423285e52f9f87b13363. SourceQt6.12 QStrokerRoundCap, QOutlineMapper::curveTo, QBezier::addToPolygon. No tolerance changes or fittedwidthtable.
