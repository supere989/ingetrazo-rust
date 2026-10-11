# COLLADA material resolution: independent acceptance

Runtime `d4e936bf → 78425642` follows the pinned original optional material/effect/image law. Missing optional bindings preserve geometry, literal color and shader-kind precedence match, forgiving local percent paths and missing-image tints match, image owners are shared per import, and unreferenced color-only bytes are pruned. Independent read-only review found no blocking defect in this bounded contract.

The frozen binary SHA256 is `b8349255ac0ecf061fa0a8ad57b4d60c9f50d4799870c117e94086bfe9203aea`. Independent prebuilt replay passes all 21 newly captured material cases and four meaningful changed-positive controls, plus all 12 small, seven reference and four UV retained source cases. Every output is reopened with the actual pinned original IGZ reader. No original recapture or Rust build occurred in independent acceptance.

`source-records.zip` preserves the complete frozen 21-case source tree, including empty directories and `source.json`; `source-manifest.json` authenticates every file. `proofrecords.zip` and `manifest.json` retain first checker ZIP-assumption failure, the first 21-material-match/orphan-asset failure, corrected and independently repeated native attempts, logs, all 23 legacy replay inputs/resources/outputs, frozen original expectations and provenance. Both archives were reopened and every member checked against its manifest. `receipt.json` authenticates archives and manifests.

Extract source-records.zip into a new source directory and replay the already built named binary:

```sh
QT_QPA_PLATFORM=offscreen PYTHONHASHSEED=0 /tmp/ingetrazo-parity-python/bin/python scripts/check-dae-material-resolution.py replay /tmp/dae-source --output /tmp/dae-native-new-attempt --binary /tmp/ingetrazo-skp-native-target/debug/examples/dae_import_acceptance_fixture
QT_QPA_PLATFORM=offscreen PYTHONHASHSEED=0 /tmp/ingetrazo-parity-python/bin/python scripts/replay-dae-legacy.py /tmp/ingetrazo-skp-native-target/debug/examples/dae_import_acceptance_fixture /tmp/dae-legacy-new-attempt
```

The legacy `paint.png` was overwritten across historical captures. Textured cases restore exact bytes from their corresponding retained IGZ and assert the frozen expected image hash. Missing-UV cases have color-only expected records; their inputs are reconstructed only from the literal frozen QImage recipes, with each resulting byte hash recorded. They do not have a historical image hash to authenticate, and this proof does not claim otherwise. Stable reference/UV assets are also verified by expected image hashes.

Source tint equality pins `PYTHONHASHSEED=0`; random original processes have process-specific colors. Duplicate effect SID precedence is a preexisting separate gap. Existing open-face winding nondeterminism boundary remains unchanged; closed-cube oriented rings are strict. This batch does not assert complete arbitrary COLLADA, FACEME/hierarchy or UI import history parity. Prior 13 DAE tests including 18 hash vectors are retained, and were not repeated after the final narrow pruning change.
