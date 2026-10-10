# Pinned native JPEG pixel decoder
Upstream zune-jpeg0.5.15 crate SHA25627bc9d5b815bc103f142aa054f561d9187d191692ec7c2d1e2b4737f8dbd7296, original MIT/Apache-2.0/Zlib licenses retained.
Source opt-in only: Rust translations of libjpeg-turbo3.2.0 jidctint.c, jdcolor.c, jdsample.c. IJG license in LICENSE-IJG and full libjpeg-turbo license inventory in LICENSE-TURBO; original algorithms copyright1991–1998 ThomasG.Lane and libjpeg-turbo contributors as listed upstream. This software is based in part on the work of the Independent JPEG Group.
Changes: new use_source_islow() selects source-compatible thirteen-bit full-resolution IDCT, sixteen-bit YCbCr conversion and IJG fancy interpolation including width<=2 nearest policy; standard decoder defaults/SIMD remain unchanged. Source decode only accepts RGBA output. EXIF/XMP/ICC/IPTC unused by this pixel-only consumer are skipped without allocation; APP14 color metadata remains interpreted.
Official source:
https://github.com/libjpeg-turbo/libjpeg-turbo/tree/3.2.0/src
https://github.com/qt/qtbase/blob/v6.12.0/src/plugins/imageformats/jpeg/qjpeghandler.cpp
Source Qt plugin embeds libjpeg-turbo3.2.0, defaultquality75 selects ISLOW/fancy.
Maintain the vendored source as a pinned dependency. Future upstream upgrades require exact original decoder corpora and default-path regressions before updating.
The opt-in correctness path is currently scalar; this is not a claim of SIMD/throughput parity. Cold decode is immutable-asset cached by future caller; portable ARM/Windows runtime proof remains required.
