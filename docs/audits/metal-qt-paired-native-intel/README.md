# Actual same-runner Intel Qt / Metal mismatch

Run38059244391 source6f095c537dfd8fa3aa2af0dfef8e9d2f1ac78edf, job114233939821 terminalFAIL, artifact11673310252 archiveSHAd683df82568d8757cbf3cab90535e1f86dfcdff1387595e00b985a57af6e8025. Complete actual artifact retained.

Actual Qt6.12 QWidget/QPainter NSWindow36 capture fourRGB20,51,89 matches rawQImage4096pixels. Actual Metal ownNSWindow45 capture fourRGB21,51,89, despite actual positivecallback/exactresize presentation PASS. Independent strict pair records one-byte red failure without tolerance or palettefit. This distinguishes actual two render/presentation paths on the same runner; it does not establish the cause. Raw offscreen Metal clear20,51,89 and both full PNGs/profiles retained.

ApplicationSave/Discard reach phase1 with realclose/modal but no acknowledgedpositive modal submission; Cancel remainsphase0. All show visibletrue beforeexit. PhysicalMac/fullhost acceptance remains open.

Apple documents CAMetalLayer.colorspace default nil as not color matched. Existing source does not set this field. An explicit authored sRGB output contract is a source-backed next candidate to test, not a proved correction or fitted byte adjustment.
https://developer.apple.com/documentation/quartzcore/cametallayer/colorspace
