# Native SKP VFF import stage

The Rust reader adapts OpenSKP MIT source at `291700bc213655a00461838de71e5f206311e619`, the exact frozen Python requirements pin. Copyright Ahsan Mehmood 2024–2026; full license is retained in the reader directory. Frozen application source is `6be29fe437a7cd992d4e079f71720821b25216b9`.

`SU_File.skp` is the original OpenSKP test fixture. Additional controls author the actual supported VFF TLV grammar and retain that file header/archive metadata; they are engineering inputs, not evidence of all SketchUp eras. The runtime is Rust only. Python/OpenSKP is used exclusively as the independent test oracle.

This stage supports face/coedge connectivity, holes, source flags, plain materials, source layer recipes, top-level leaf placements and source-threshold mesh sharing. Parsing produces a bounded neutral payload; unsupported or malformed data fails before destination scene publication. Duplicate record identities, finite numeric admission, file/archive/TLV bounds and destination preservation are tested.

Legacy MFC containers, textured materials/UV, annotations, saved views, nested/tagged extraction, billboard/image definitions and empty/library-only semantics explicitly refuse. Streaming/performance and rendered GUI import acceptance remain open. P21 is partial.

Reproduction: build the formats `skp_import_acceptance_fixture` example; run `scripts/check-native-skp-vff.py OPENSKP_REPO NATIVE_BINARY controls OUTPUT` with the frozen reference interpreter. The checker requires the literal six-control manifest, exact parser pin and hashes, and compares original-loaded native IGZ geometry, metadata, layers and sharing. A modified-point negative must fail equality.

Frozen native source `79b043014d0d9d6b1477d71223ac6badd9673b7d` / binary SHA256 `8b170aeecaac443eb7b4b89602428b8f8304a01fb4b5175b3623fc32d735f10f`: real fixture plus five successful geometry/recipe/sharing controls and one expected textured-material refusal pass. Three destination/payload/integration tests and one duplicate-record admission test pass. The failed duplicate-face test fixture is retained separately; it was corrected to raw TLV coedge bodies, without weakening parser admission.
