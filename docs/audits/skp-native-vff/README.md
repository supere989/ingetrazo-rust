# Native SKP VFF import stage

The Rust reader adapts OpenSKP MIT source at `291700bc213655a00461838de71e5f206311e619`, the exact frozen Python requirements pin. Copyright Ahsan Mehmood 2024–2026; full license is retained in the reader directory. Frozen application source is `6be29fe437a7cd992d4e079f71720821b25216b9`.

`SU_File.skp` is the original OpenSKP test fixture. Additional controls author the actual supported VFF TLV grammar and retain that file header/archive metadata; they are engineering inputs, not evidence of all SketchUp eras. The runtime is Rust only. Python/OpenSKP is used exclusively as the independent test oracle.

This stage supports face/coedge connectivity, holes, source flags, plain materials, source layer recipes, top-level leaf placements and source-threshold mesh sharing. Parsing produces a bounded neutral payload; unsupported or malformed data fails before destination scene publication. Duplicate record identities, finite numeric admission, file/archive/TLV bounds and destination preservation are tested.

Legacy MFC containers, textured materials/UV, annotations, saved views, nested/tagged extraction, billboard/image definitions and empty/library-only semantics explicitly refuse. Streaming/performance and rendered GUI import acceptance remain open. P21 is partial.

Reproduction: build the formats `skp_import_acceptance_fixture` example; run `scripts/check-native-skp-vff.py OPENSKP_REPO NATIVE_BINARY controls OUTPUT` with the frozen reference interpreter. The checker requires the literal six-control manifest, exact parser pin and hashes, and compares original-loaded native IGZ geometry, metadata, layers and sharing. A modified-point negative must fail equality.

Frozen native source `79b043014d0d9d6b1477d71223ac6badd9673b7d` / binary SHA256 `8b170aeecaac443eb7b4b89602428b8f8304a01fb4b5175b3623fc32d735f10f`: real fixture plus five successful geometry/recipe/sharing controls and one expected textured-material refusal pass. Three destination/payload/integration tests and one duplicate-record admission test pass. The failed duplicate-face test fixture is retained separately; it was corrected to raw TLV coedge bodies, without weakening parser admission.

## Nested placement stage

The placement planner follows reachable census, memoized subtree face totals, source sharing thresholds, nested own-tag extraction and source priority. Shared parents retain shared prototype children with local placement transforms. Singleton top-level promotion avoids swallowing already-shared children. The original source folds identical built prototypes (including alias material joins) up to eight rounds; the native implementation retains this sharing and placement order. Source QMatrix4x4 compositions retain float components. Actual controls include nested nonprincipal fractional transforms and reflections/nonuniform scales.

The nested-v1 corpus requires the prior six cases plus ten literal added names. The checker traverses actual source/native children and verifies local/world geometry, layer groups, original axes, all mesh sharing clusters, material recipes and counts. Remaining full-feature gaps above still apply; no rendered import or legacy-era closure.

Frozen nested source `ee142abeac35bc31cb246b483f710abf5103017c`, binary SHA256 `2af225fed5345136394b0960c6e30b1a5807fa4cb1601b89e5ea59416228395b`: all 17 strict rows pass, modified-point equality negative passes, five focused tests pass. Exact expanded JSON is gzip-retained with both raw and stored SHA256 digests; the earlier alias-sharing failure remains in its own directory. These are record-controlled inputs; real legacy-file and GUI workflow proof remain required.
