# P07 guide and Unicode inference: bounded acceptance

The source pin is `6be29fe437a7cd992d4e079f71720821b25216b9`; new source captures contain only the previously missing contracts. Runtime `11c8f85a`, consumer `37b11a4b`, and mandatory test qualification `c81d14bd` are the complete implementation chain.

All 48 guide decisions across XY/XZ/YZ/oblique fixed view matrices pass, including exact float32 clipped endpoints, physical crossings versus parallel/skew pairs, analytic occlusion, model-guide and guide-point acquisition, and literal nine/fourteen logical-pixel boundaries. All 286 numeric rows cover the 68 Unicode15.1 decimal blocks plus rejected punctuation/numeric forms. Eight original versus actual App workflows (56 states), one actual egui guide-acquisition + Arabic Text/Enter/Undo/Redo control, and eight meaningful mutations pass. The retained 29 snap cases and 1,168 measurement rows/15 physical-unit App workflows also pass without recapture. One targeted native-double projection guard passes.

Infinite guides now use the original homogeneous viewport clipping law, guide points participate as endpoints, and source-authorable crossings reuse the normalized original topology solver. Logical-pixel evaluation uses an optional source float32 matrix, separately supplied by both GUI hover and press callers. Genuinely native-double points/directions and default double projection protocols keep the original matrix; rendering/picking and authored model positions are untouched. Numeric submissions normalize original-supported Unicode decimal digits and whitespace once at cold App dispatch and in the shared parser; ordinary ASCII is borrowed without allocation.

The first seven guide failures, generic linear-projection skew regression, consumer/observer/test compilation mistakes, and their corrected attempts remain archived. The two bounded source production-callback observations confirmed the skew difference was projection arithmetic, not a different callback policy. No tolerance or source recipe was weakened.

`source-records.zip` and `source-manifest.json` authenticate the flat `source.json`, `source-app.json`, and bounded `production-callback.json` inputs. `records.zip` and `manifest.json` retain all attempts, source logs, targeted tests, legacy replay and provenance. Every archive member was reopened and byte-verified. The prebuilt owner binary SHA256 is `94d47b432b2af914072647a7aeb6b49a704f396862d7f1cd58d2ea55865e913f`.

After authenticating/extracting source-records.zip into SOURCE, use one named consumer/checker:

```sh
EXAMPLES/p07_inference_workflows SOURCE/source.json SOURCE/source-app.json OUTPUT/native.json
python3 scripts/check-p07-inference.py SOURCE/source.json SOURCE/source-app.json OUTPUT/native.json OUTPUT/check.json
```

The shared build adds only `-p ingetrazo-gui --example p07_inference_workflows`. Optional affected legacy replay reuses existing binaries without any build/source capture:

```sh
python3 scripts/replay-p07-retained.py EXAMPLES /tmp/p07-retained-new-attempt
```

The unchanged complete criterion map is `criterion-review.json`. P07 is not promoted: actual perspective guide producer/camera/variable-w sessions and original keyboard-buffer filtering remain concrete unproved contracts. Existing complete drawing-plane/axes and unit preference evidence is retained; vague broader-plane requirements are not added. Fixed-matrix semantic proof does not claim source whole-frame glyphs, backend rendering, raw entity identity, or OS pointer parity.
