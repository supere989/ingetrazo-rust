# Native SKP modern dimension acceptance

Production b639ca09 resolves the pinned VFF dimension connection records, cumulative candidate work admission, source transform ordering and archive-folder material join. 21a2a70b adopts the first source back-face color as document metadata rather than a style override. Both original and native use the exact pinned input files; the importer is native Rust.

Fresh strict comparison passes Untitled (43 root groups, 13 dimensions) and legacy_v2014_synthetic (10 root groups). The signature compares recursive imported groups, sharing, materials, layers and an explicit root mesh/annotation/view/section/image/guide/back-color inventory. It does not imply all SKP eras or rendered parity. Altered geometry is rejected. Four focused annotation controls pass, including cumulative overlapping candidate-byte/node admission and malformed/nonfinite geometry.

The retained b639ca09 original/native failure is the adopted back-face color mismatch; its compressed comparison and exact executable SHA remain under retained-style-failure. The corrected imported metadata initializes the terrain baseline because this newly built Scene has no preserved original terrain schema; it does not bypass later edits to that schema.

Replay: pinned reference Python scripts/check-native-skp-modern-annotations.py OPENSKP_CHECKOUT FRESH_NATIVE_BINARY OUTPUT. The checker asserts both parser and app pins and records executable/source/input/raw-signature digests. Whole-import rendered texture/style behavior, positive text labels and broader eras remain required scope.
